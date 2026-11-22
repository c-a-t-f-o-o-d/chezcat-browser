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

fn append_element(nodes: &mut Vec<Node>, tag: &str) -> Result<usize, String> {
    nodes
        .try_reserve(1)
        .map_err(|_| "Cannot reserve an HTML document element.")?;
    let index = nodes.len();
    nodes.push(Node {
        kind: NodeKind::Element {expanded_name:Vec::new(),
            tag: tag.into(),
            id: None,
            href: None,
            attrs: Vec::new(),
        },
        namespace: crate::html::Namespace::Html,
        children: Vec::new(),
    });
    Ok(index)
}

fn blank_text(node: &Node) -> bool {
    matches!(&node.kind, NodeKind::Text(text) if text.bytes().all(|b| matches!(b,b' '|b'\t'|b'\n'|b'\r'|0x0c)))
}

fn document_elements(nodes: &mut Vec<Node>) -> Result<(usize, usize, usize), String> {
    let mut roots = std::mem::take(&mut nodes[0].children);
    let explicit_html = roots
        .iter()
        .copied()
        .find(|&index| element_named(nodes, index, &["html"]));
    let html;
    let siblings;
    if let Some(index) = explicit_html {
        html = index;
        if roots.iter().any(|&id| {
            id != html && !blank_text(&nodes[id]) && !matches!(nodes[id].kind, NodeKind::Comment(_) | NodeKind::DocumentType(_))
        }) {
            return Err("HTML recovery outside the document element is unsupported.".into());
        }
        roots.retain(|&id| !blank_text(&nodes[id]));
        siblings = std::mem::take(&mut nodes[html].children);
    } else {
        html = append_element(nodes, "html")?;
        let first = roots
            .iter()
            .position(|&id| {
                !blank_text(&nodes[id]) && !matches!(nodes[id].kind, NodeKind::Comment(_) | NodeKind::DocumentType(_))
            })
            .unwrap_or(roots.len());
        siblings = roots.split_off(first);
        roots.retain(|&id| matches!(nodes[id].kind, NodeKind::Comment(_) | NodeKind::DocumentType(_)));
        roots.push(html);
    }
    let explicit_head = siblings
        .iter()
        .copied()
        .find(|&id| element_named(nodes, id, &["head"]));
    let explicit_body = siblings
        .iter()
        .copied()
        .find(|&id| element_named(nodes, id, &["body", "frameset"]));
    let head = match explicit_head {
        Some(id) => id,
        None => append_element(nodes, "head")?,
    };
    let body = match explicit_body {
        Some(id) => id,
        None => append_element(nodes, "body")?,
    };
    let mut html_children = vec![head];
    let mut in_body = false;
    for id in siblings {
        if id == head {
            if in_body {
                return Err("Misplaced HTML head is unsupported.".into());
            }
        } else if id == body {
            if in_body {
                return Err("Misplaced HTML body is unsupported.".into());
            }
            in_body = true;
        } else if element_named(nodes, id, &["html", "head", "body", "frameset"]) {
            return Err("Duplicate HTML document elements are unsupported.".into());
        } else if !in_body && matches!(nodes[id].kind, NodeKind::Comment(_) | NodeKind::DocumentType(_)) {
            html_children.push(id);
        } else if !in_body && blank_text(&nodes[id]) {
            // Whitespace before the ordinary body token has no visible node here.
        } else if !in_body
            && explicit_head.is_none()
            && element_named(
                nodes,
                id,
                &[
                    "base", "link", "meta", "noscript", "style", "script", "title",
                ],
            )
        {
            nodes[head].children.push(id);
        } else {
            in_body = true;
            nodes[body].children.push(id);
        }
    }
    html_children.push(body);
    nodes[0].children = roots;
    nodes[html].children = html_children;
    Ok((html, head, body))
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
        let (mut parsed, doctype_position) =
            crate::html::parse_inert_html(markup, available - 1, &|| {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return Err("HTML document parsing time limit reached.".into());
                }
                Ok(())
            })?;
        let (html, head, body) = document_elements(&mut parsed.nodes)?;
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
                kind:parsed.kind,mode:parsed.mode,content_type:"text/html",
                implementation,
                doctype,
                url,
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
                    .len()
                    .saturating_add(href.as_ref().map_or(0, String::len)),
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
        let admitted = self.reserve_inert_entries(entries.len(), payload, generated.len())?;
        debug_assert_eq!(admitted, document);
        self.state.inert_documents.generated.extend(generated);
        self.state.inert_documents.entries.extend(entries);
        Ok(Value::Host(document))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
