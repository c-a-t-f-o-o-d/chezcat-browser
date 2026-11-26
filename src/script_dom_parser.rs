//! Bounded inert HTML DOMParser output, separate from the active document tree.
//! https://html.spec.whatwg.org/multipage/dynamic-markup-insertion.html#dom-parsing-and-serialization
use super::*;
#[cfg(test)]
#[path="script_domparser_template_publication_tests.rs"]
mod template_publication_tests;

pub(in crate::scripting) const CONSTRUCTOR: &str = "DOMParser";
pub(in crate::scripting) const PARSE: &str = "\0DOMParser.parseFromString";

fn element_named(nodes: &[Node], index: usize, names: &[&str]) -> bool {
    let node = &nodes[index];
    node.namespace == crate::html::Namespace::Html
        && matches!(&node.kind, NodeKind::Element { tag, .. } if names.contains(&tag.as_str()))
}

fn document_elements(nodes: &[Node]) -> Result<(usize, usize, usize), String> {
    let child = |parent: usize, names: &[&str]| {
        nodes[parent].children.iter().copied().find(|&n| element_named(nodes,n,names))
    };
    let html=child(0,&["html"]).ok_or("Missing parsed HTML document element.")?;
    let head=child(html,&["head"]).ok_or("Missing parsed HTML head.")?;
    let body=child(html,&["body","frameset"]).ok_or("Missing parsed HTML body or frameset.")?;
    Ok((html,head,body))
}

fn parsed_template(node: &Node) -> bool {
    node.namespace == crate::html::Namespace::Html
        && matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "template")
}

fn parsed_document_entries(nodes: &[Node]) -> Result<usize, String> {
    let templates = nodes.iter().filter(|node| parsed_template(node)).count();
    nodes.len().checked_add(1).and_then(|count| count.checked_add(templates))
        .and_then(|count| count.checked_add(if templates == 0 { 0 } else { 2 }))
        .ok_or("Parsed template identity count overflow.".into())
}

impl Bridge<'_> {
    pub(in crate::scripting) fn create_dom_parser(&mut self) -> Result<Value, String> {
        let parser = self.reserve_inert_entries(1, 0, 0)?;
        self.state.inert_documents.entries.push(Entry::Parser);
        Ok(Value::Host(parser))
    }

    pub(in crate::scripting) fn parse_inert_document(
        &mut self,
        args: &[Value],
    ) -> Result<Value, String> {
        let Some(Value::String(markup)) = args.first() else {
            return Err("DOMParser requires converted HTML source.".into());
        };
        if markup.len() > self.state.limits.host.string_bytes {
            return Err("HTML source exceeds the inert document string limit.".into());
        }
        self.state.work(markup.len())?;
        self.state.copies(markup.len())?;
        let available = self.inert_remaining_entries();
        if available < 5 {
            return Err("Inert DOM identity limit reached.".into());
        }
        let deadline = self.state.execution_deadline;
        let (parsed, doctype_position) =
            crate::html::parse_inert_html(markup, available - 1, &|| {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return Err("HTML document parsing time limit reached.".into());
                }
                Ok(())
            })?;
        let (html, head, body) = document_elements(&parsed.nodes)?;
        let count = parsed_document_entries(&parsed.nodes)?;
        if count > available {
            return Err("Inert DOM identity limit reached.".into());
        }
        let url = if self.state.location.is_some() {
            let Value::String(url) = self.location_property("href")? else {
                unreachable!()
            };
            url
        } else {
            "about:blank".into()
        };
        self.publish_parsed_document(parsed, url, (html, head, body), doctype_position, count)
    }

    fn publish_parsed_document(
        &mut self, mut parsed: Document, url: String, roots: (usize, usize, usize),
        doctype_position: Option<usize>, count: usize,
    ) -> Result<Value, String> {
        if count != parsed_document_entries(&parsed.nodes)? || count > self.inert_remaining_entries() {
            return Err("Parsed document identity reservation is inconsistent.".into());
        }
        let arena = &self.state.inert_documents;
        let document = FIRST_ENTRY + arena.entries.len() as u32;
        let implementation = document + 1;
        let first = document + 2;
        let handle = |local: usize| if local == 0 { document } else { first + local as u32 - 1 };
        let template_count = parsed.nodes.iter().filter(|node| parsed_template(node)).count();
        let associated = if template_count == 0 { None } else {
            Some(document.checked_add(u32::try_from(parsed.nodes.len().checked_add(1)
                .ok_or("Parsed template owner count overflow.")?).map_err(|_|"Parsed template owner count overflow.")?)
                .ok_or("Parsed template owner handle overflow.")?)
        };
        let generated_base = arena.generated.len();
        let (html, head, body) = roots;
        // All owner/edge decisions remain local until the shared batch admits
        // both the document and every template content identity together.
        self.state.work(parsed.nodes.len())?;
        self.state.copies(parsed.nodes.len().checked_mul(
            size_of::<(Option<u32>, u32, Option<u32>)>() + size_of::<usize>())
            .ok_or("Parsed template traversal storage overflow.")?)?;
        let mut topology = Vec::new();
        topology.try_reserve_exact(parsed.nodes.len()).map_err(|_|"Cannot reserve parsed owner plan.")?;
        topology.resize(parsed.nodes.len(), (None, document, None));
        let mut templates = Vec::new();
        templates.try_reserve_exact(template_count).map_err(|_|"Cannot reserve parsed template plan.")?;
        for (local, node) in parsed.nodes.iter().enumerate() {
            if parsed_template(node) {
                let fragment = associated.unwrap().checked_add(2)
                    .and_then(|first| first.checked_add(templates.len() as u32))
                    .ok_or("Parsed template fragment handle overflow.")?;
                topology[local].2 = Some(fragment);
                templates.push(local);
            }
        }
        let mut pending = Vec::new();
        pending.try_reserve_exact(parsed.nodes.len()).map_err(|_|"Cannot reserve parsed owner traversal.")?;
        pending.push(0);
        while let Some(local) = pending.pop() {
            let source = &parsed.nodes[local];
            self.state.work(source.children.len().saturating_add(1))?;
            let (parent, owner) = if let Some(fragment) = topology[local].2 {
                (fragment, associated.unwrap())
            } else { (handle(local), topology[local].1) };
            for &child in &source.children {
                topology[child].0 = Some(parent);
                topology[child].1 = owner;
                pending.push(child);
            }
            if let NodeKind::Element { attrs, .. } = &source.kind { self.state.work(attrs.len())?; }
        }
        let children_for = |children: &[usize]| -> Result<Vec<u32>, String> {
            let mut result = Vec::new();
            result.try_reserve_exact(children.len()).map_err(|_|"Cannot reserve parsed child links.")?;
            result.extend(children.iter().copied().map(handle));
            Ok(result)
        };
        let mut entries = Vec::new();
        entries.try_reserve_exact(count).map_err(|_|"Cannot reserve parsed document identities.")?;
        let mut generated = Vec::new();
        generated.try_reserve_exact(parsed.nodes.len()-1).map_err(|_|"Cannot reserve parsed source nodes.")?;
        let document_children = children_for(&parsed.nodes[0].children)?;
        let mut payload = url.capacity().checked_add(parsed.title.capacity())
            .and_then(|bytes|bytes.checked_add(document_children.capacity()*size_of::<u32>()))
            .ok_or("Parsed document payload overflow.")?;
        entries.push(Entry::Node(InertNode {
            kind: Kind::Document(DocumentNodes {
                xml_interface:false, kind:parsed.kind, mode:parsed.mode, content_type:"text/html",
                implementation, doctype:doctype_position.map(handle), url, base_url_override:None,
                html:Some(handle(html)), head:Some(handle(head)), body:Some(handle(body)),
                title:std::mem::take(&mut parsed.title),
            }), owner:document, parent:None, children:document_children,
        }));
        entries.push(Entry::Implementation);
        let mut fragments = Vec::new();
        fragments.try_reserve_exact(template_count).map_err(|_|"Cannot reserve parsed template fragments.")?;
        let mut links = Vec::new();
        links.try_reserve_exact(template_count).map_err(|_|"Cannot reserve parsed template content index.")?;
        for local in templates {
            let children = children_for(&parsed.nodes[local].children)?;
            payload = payload.checked_add(children.capacity()*size_of::<u32>())
                .ok_or("Parsed template child payload overflow.")?;
            links.push((handle(local),topology[local].2.unwrap()));
            fragments.push(Entry::Node(InertNode {
                kind:Kind::Fragment { template:Some(handle(local)) }, owner:associated.unwrap(),
                parent:None, children,
            }));
        }
        for (offset, mut source) in parsed.nodes.drain(1..).enumerate() {
            let local = offset + 1;
            let children = if topology[local].2.is_some() { Vec::new() } else { children_for(&source.children)? };
            let metadata = match &source.kind {
                NodeKind::Element { tag, href, .. } => tag.capacity()
                    .checked_add(href.as_ref().map_or(0,String::capacity)).ok_or("Parsed element metadata overflow.")?,
                _ => 0,
            };
            payload = payload.checked_add(node_bytes(&source)).and_then(|bytes|bytes.checked_add(metadata))
                .and_then(|bytes|bytes.checked_add(size_of::<Node>()+2*size_of::<usize>()))
                .and_then(|bytes|bytes.checked_add(children.capacity()*size_of::<u32>()))
                .ok_or("Parsed source payload overflow.")?;
            source.children = Vec::new();
            generated.push(GeneratedNode { node:Rc::new(source), unsafe_nonce:parsed.nonce_unsafe_nodes.contains(&local) });
            entries.push(Entry::Node(InertNode {
                kind:Kind::Parsed { source:ParsedSource::Generated(generated_base+offset), initial_template:None },
                owner:topology[local].1, parent:topology[local].0, children,
            }));
        }
        if let Some(owner) = associated {
            entries.push(Entry::Node(InertNode {
                kind:Kind::TemplateDocument { implementation:owner+1, kind:parsed.kind, html:None, doctype:None },
                owner, parent:None, children:Vec::new(),
            }));
            entries.push(Entry::Implementation);
            entries.extend(fragments);
        }
        debug_assert_eq!(entries.len(),count);
        self.state.copies(payload)?;
        self.publish_inert_batch_with_document(document,entries,generated,payload,links,true,
            associated.map(|owner|(document,owner)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity_fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
    fn identity_source()->(Document,Option<usize>,(usize,usize,usize)){
        let(mut parsed,dtype)=crate::html::parse_inert_html("<!doctype html><html><head><title>owned</title></head><body><a href=\"#target\">data</a><!--marker--></body></html>",100,&||Ok(())).unwrap();let roots=document_elements(&parsed.nodes).unwrap();for node in &mut parsed.nodes{if let NodeKind::Element{tag,href,..}=&mut node.kind{if tag=="a"{tag.reserve(128);href.as_mut().unwrap().reserve(256);}}}(parsed,dtype,roots)
    }
    #[test]
    fn domparser_node_identity_exact_owned_capacities_joint_backing_and_projected_parent_records(){
        let(mut doc,mut forms,mut page)=identity_fixture();let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let(parsed,dtype,roots)=identity_source();let count=parsed.nodes.len()+1;let before=(b.state.retained,b.template_import_backing(),b.state.inert_documents.generated.len());let Value::Host(owner)=b.publish_parsed_document(parsed,"https://parsed-storage.test/source".into(),roots,dtype,count).unwrap()else{panic!("real parsed document")};
        let mut payload=0usize;let mut pending=vec![owner];let mut native=false;while let Some(handle)=pending.pop(){let node=b.state.inert_documents.node(handle).unwrap();assert_eq!(node.owner,owner);payload+=node.children.capacity()*size_of::<u32>();if let Kind::Document(record)=&node.kind{payload+=record.url.capacity()+record.title.capacity();assert_eq!(record.implementation,owner+1);assert!(record.html.is_some()&&record.doctype.is_some());}for &child in &node.children{assert_eq!(b.state.inert_documents.node(child).unwrap().parent,Some(handle));pending.push(child);}native|=node_index(handle).is_some();}
        let mut widened=false;for source in b.state.inert_documents.generated.iter().skip(before.2){payload+=node_bytes(&source.node)+size_of::<Node>()+2*size_of::<usize>();if let NodeKind::Element{tag,href,..}=&source.node.kind{payload+=tag.capacity()+href.as_ref().map_or(0,String::capacity);if tag=="a"{assert!(tag.capacity()>tag.len()&&href.as_ref().unwrap().capacity()>href.as_ref().unwrap().len());widened=true;}}}assert!(native&&widened);assert_eq!(b.state.retained-before.0,payload+b.template_import_backing()-before.1);
    }
    #[test]
    fn domparser_node_identity_storage_and_late_copy_refusal_publish_no_batch_and_recover(){
        let(mut doc,mut forms,mut page)=identity_fixture();let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let snapshot=|b:&Bridge<'_>|(b.state.retained,b.template_import_backing(),b.doc.nodes.len(),b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.state.inert_documents.adopted.len(),b.state.dom_commits);let before=snapshot(&b);let limit=b.state.retained_limit;let copies=b.state.copy_work;
        for storage in [true,false]{let(parsed,dtype,roots)=identity_source();let count=parsed.nodes.len()+1;if storage{b.state.retained_limit=b.state.retained;}else{let payload=parsed.nodes.iter().map(|node|node_bytes(node)).sum::<usize>();b.state.copy_work=payload;}assert!(b.publish_parsed_document(parsed,"https://parsed-storage.test/refused".into(),roots,dtype,count).is_err());assert_eq!(snapshot(&b),before);b.state.retained_limit=limit;b.state.copy_work=copies;}
        let(parsed,dtype,roots)=identity_source();let count=parsed.nodes.len()+1;let Value::Host(owner)=b.publish_parsed_document(parsed,"https://parsed-storage.test/recovered".into(),roots,dtype,count).unwrap()else{panic!("recovered batch")};let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};let body=record.body.unwrap();let root=b.inert_children(body).unwrap()[0];assert!(node_index(root).is_some());assert_eq!(b.state.inert_documents.node(root).unwrap().owner,owner);
    }

    fn start(source: &str) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse(
            "<!doctype html><html><title>Parser fixture</title><body><p id=original>Guard</p></body></html>",
        );
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        page.set_location(&crate::navigation::Location::home());
        let outcome = page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                label: "Original inert DOMParser fixture".into(),
                source: source.into(),
            }],
        );
        (doc, forms, page, outcome)
    }

    #[test]
    fn dom_parser_builds_a_genuine_inert_html_document_with_its_own_nodes() {
        let (mut doc, mut forms, mut page, outcome) = start(
            r#"
var parser=new DOMParser(), events=0, pending;
if(!(parser instanceof DOMParser) || Object.getPrototypeOf(parser)!==DOMParser.prototype ||
   parser.parseFromString.length!==2 || Object.prototype.toString.call(parser)!=='[object DOMParser]')
    throw 'interface';
var parsed=parser.parseFromString('<!--pre--><!DOCTYPE html><html id="root"><head><title>Local &amp; inert</title></head>'+
  '<body><section id="child">Example<!--note--><span>!</span></section>'+
  '<script src="/inert-script.js">window.events=1;</script><img src="/inert-image.png" onload="events=2"></body></html>', 'text/html');
if(!(parsed instanceof Document) || parsed===document || parsed.defaultView!==null ||
   parsed.ownerDocument!==null || parsed.contentType!=='text/html' || parsed.compatMode!=='CSS1Compat' ||
   parsed.characterSet!=='UTF-8' || parsed.title!=='Local & inert' ||
   parsed.doctype.name!=='html' || parsed.doctype.parentNode!==parsed || parsed.firstChild.nodeType!==8 ||
   parsed.documentElement.id!=='root' || parsed.documentElement.parentNode!==parsed ||
   parsed.head.ownerDocument!==parsed || parsed.body.parentNode!==parsed.documentElement)
    throw 'document identity';
var section=parsed.body.firstChild;
if(section.id!=='child'||section.ownerDocument!==parsed||section.parentNode!==parsed.body ||
   section.textContent!=='Example!'||section.firstChild.nextSibling.nodeType!==8 ||
   section.lastChild.textContent!=='!'||events!==0)throw 'parsed tree';
var imported=document.importNode(section,true);
if(imported===section||imported.ownerDocument!==document||imported.parentNode!==null||
   imported.textContent!==section.textContent)throw 'checked import';
if(events!==0||document.getElementById('original').textContent!=='Guard')throw 'original document';
var implied=parser.parseFromString('<title>Implied title</title><p id="child">Other document</p>', 'text/html');
if(implied.doctype!==null||implied.compatMode!=='BackCompat'||implied.body.firstChild.id!=='child'||
   implied.documentElement.firstChild!==implied.head||implied.head.firstChild.textContent!=='Implied title'||
   implied.body.firstChild.ownerDocument!==implied||implied.URL!==document.URL)
    throw 'implied shell';
document.title='dom-parser-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "dom-parser-ok");
        let nodes = doc.nodes.len();
        let forbidden = page.console_eval(
            &mut doc,
            &mut forms,
            "document.importNode(section.nextSibling,true);",
        );
        assert_eq!(forbidden.errors, 1);
        assert!(
            page.report()
                .contains("Import supports ordinary HTML containers")
        );
        assert_eq!(doc.nodes.len(), nodes);
        assert!(page.state.dynamic.pending.is_empty());
        assert!(page.state.images.entries.is_empty());
    }

    #[test]
    fn dom_parser_type_and_capacity_failures_preserve_the_existing_tree() {
        let (mut doc, mut forms, mut page, outcome) = start(
            r#"
var parser=new DOMParser(), rejected=0, coercion='';
try{DOMParser();}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
try{parser.parseFromString();}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
try{parser.parseFromString.call({}, {toString:function(){coercion+='BAD';}}, 'text/html');}
catch(e){if(e.name==='TypeError')rejected++;else throw e;}
try{parser.parseFromString({toString:function(){coercion+='a';return '<p>Hi';}},
 {toString:function(){coercion+='b';return 'Text/HTML';}});}
catch(e){if(e.name==='TypeError')rejected++;else throw e;}
if(rejected!==4||coercion!=='ab')throw 'brand and conversions';
var kept=parser.parseFromString('<p>kept</p>','text/html');
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let len = page.state.inert_documents.entries.len();
        let retained = page.state.retained;
        let allowance = page.state.limits.host.extra_nodes;
        page.state.limits.host.extra_nodes = len + 4;
        let result = page.console_eval(
            &mut doc,
            &mut forms,
            "parser.parseFromString('<p>over capacity</p>','text/html');",
        );
        assert_eq!(result.errors, 1);
        assert_eq!(page.state.inert_documents.entries.len(), len);
        assert_eq!(page.state.retained, retained);
        page.state.limits.host.extra_nodes = allowance;
        let result = page.console_eval(&mut doc, &mut forms, "if(kept.body.firstChild.textContent!=='kept'||parser.parseFromString('<p>new</p>','text/html').body.firstChild.textContent!=='new')throw 'retry';");
        assert_eq!(result.errors, 0, "{}", page.report());
    }
}
