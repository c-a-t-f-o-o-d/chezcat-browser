//! Bounded inert HTML DOMParser output, separate from the active document tree.
//! https://html.spec.whatwg.org/multipage/dynamic-markup-insertion.html#dom-parsing-and-serialization
use super::*;

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
        let count = parsed
            .nodes
            .len()
            .saturating_add(1);
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
        &mut self,
        mut parsed: Document,
        url: String,
        roots: (usize, usize, usize),
        doctype_position: Option<usize>,
        count: usize,
    ) -> Result<Value, String> {
        let arena = &self.state.inert_documents;
        let document = FIRST_ENTRY + arena.entries.len() as u32;
        let implementation = document + 1;
        let first = document + 2;
        let handle = |local: usize| {
            if local == 0 {
                document
            } else {
                first + local as u32 - 1
            }
        };
        let doctype=doctype_position.map(handle);
        let (html, head, body) = roots;
        let generated_base = arena.generated.len();
        let mut entries = Vec::new();
        let mut generated = Vec::new();
        let mut parents = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| "Cannot reserve inert document identities.")?;
        generated
            .try_reserve_exact(parsed.nodes.len() - 1)
            .map_err(|_| "Cannot reserve inert document source nodes.")?;
        parents
            .try_reserve_exact(parsed.nodes.len())
            .map_err(|_| "Cannot reserve inert document parent links.")?;
        parents.resize(parsed.nodes.len(), None);
        let mut payload = url.capacity().saturating_add(parsed.title.capacity());
        for (local, node) in parsed.nodes.iter().enumerate() {
            self.state.work(node.children.len().saturating_add(1))?;
            for &child in &node.children {
                parents[child] = Some(handle(local));
            }
            if let NodeKind::Element { tag, attrs, .. } = &node.kind {
                if tag == "template" {
                    return Err(
                        "Template contents in DOMParser documents are outside the admitted subset."
                            .into(),
                    );
                }
                self.state.work(attrs.len())?;
            }
        }
        let children_for = |children: &[usize], extra: usize| -> Result<Vec<u32>, String> {
            let mut result = Vec::new();
            result
                .try_reserve_exact(children.len() + extra)
                .map_err(|_| "Cannot reserve inert document child links.")?;
            result.extend(children.iter().copied().map(handle));
            Ok(result)
        };
        let document_children=children_for(&parsed.nodes[0].children,0)?;
        let node = |kind, parent, children| {
            Entry::Node(InertNode {
                kind,
                owner: document,
                parent,
                children,
            })
        };
        payload = payload.saturating_add(document_children.capacity() * size_of::<u32>());
        entries.push(node(
            Kind::Document(DocumentNodes {
                xml_interface:false,kind:parsed.kind,mode:parsed.mode,content_type:"text/html",
                implementation,
                doctype,
                url,base_url_override:None,
                html: Some(handle(html)),
                head: Some(handle(head)),
                body: Some(handle(body)),
                title: std::mem::take(&mut parsed.title),
            }),
            None,
            document_children,
        ));
        entries.push(Entry::Implementation);
        for (offset, mut source) in parsed.nodes.drain(1..).enumerate() {
            let local = offset + 1;
            let children = children_for(&source.children, 0)?;
            let metadata = match &source.kind {
                NodeKind::Element { tag, href, .. } => tag
                    .capacity()
                    .saturating_add(href.as_ref().map_or(0, String::capacity)),
                _ => 0,
            };
            payload = payload
                .saturating_add(node_bytes(&source))
                .saturating_add(metadata)
                .saturating_add(size_of::<Node>() + 2 * size_of::<usize>())
                .saturating_add(children.capacity() * size_of::<u32>());
            source.children = Vec::new();
            generated.push(GeneratedNode {
                node: Rc::new(source),
                unsafe_nonce: parsed.nonce_unsafe_nodes.contains(&local),
            });
            entries.push(node(
                Kind::Parsed {
                    source: ParsedSource::Generated(generated_base + offset),
                    initial_template: None,
                },
                parents[local],
                children,
            ));
        }
        self.state.copies(payload)?;
        self.publish_inert_batch(document,entries,generated,payload,Vec::new(),true)
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
