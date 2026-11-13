//! Real, bounded initial blank contexts. No child navigation, DOM writes or
//! painting is claimed. Only authored-hidden, created, src-absent frames enter.
//! https://html.spec.whatwg.org/multipage/iframe-embed-object.html
use super::*;

const FRAME_BASE: u32 = 512;
const FRAME_STRIDE: u32 = 16;
const MAX_BLANK_FRAMES: usize = 8;

pub(super) struct BlankFrame {
    owner: usize,
    connected: bool,
    document: Document,
    origin: crate::navigation::Location,
    serialized_origin: String,
    base: crate::navigation::Location,
    policy: crate::csp::PolicySet,
}

fn window(index: usize) -> u32 { FRAME_BASE + index as u32 * FRAME_STRIDE }
fn frame_handle(handle: u32) -> Option<(usize, u32)> {
    (FRAME_BASE..FRAME_BASE + MAX_BLANK_FRAMES as u32 * FRAME_STRIDE).contains(&handle)
        .then(|| (((handle - FRAME_BASE) / FRAME_STRIDE) as usize, (handle - FRAME_BASE) % FRAME_STRIDE))
}
fn document_node(index: usize, node: usize) -> u32 {
    if node == 0 { window(index) + 1 } else { window(index) + 3 + node as u32 }
}

impl Bridge<'_> {
    pub(super) fn frame_instance_window(&self, handle:u32) -> Option<u32> {
        let (index,offset)=frame_handle(handle)?;
        let frame=self.state.blank_frames.get(index)?;
        (matches!(offset,0..=2) || matches!(offset,4..=6) && (offset-3) < frame.document.nodes.len() as u32)
            .then(||window(index))
    }

    pub(super) fn frame_position_path(&mut self, handle: u32) -> Option<Result<super::script_position::PositionPath, String>> {
        let (index, offset) = frame_handle(handle)?;
        Some((|| {
            self.frame_window_access(Some(WINDOW), window(index))?;
            if !matches!(offset, 1 | 4..=6) { return Err("Invalid blank Node position handle.".into()); }
            let frame = self.state.blank_frames.get(index).ok_or("Unknown blank Node context.")?;
            let mut node = if offset == 1 { 0 } else { (offset - 3) as usize };
            if node >= frame.document.nodes.len() { return Err("Invalid blank Node position index.".into()); }
            let mut work = 1usize;
            let mut steps = Vec::new();
            for _ in 0..frame.document.nodes.len() {
                work = work.saturating_add(frame.document.nodes.len()).saturating_add(frame.document.nodes.iter().map(|node| node.children.len()).sum::<usize>());
                let parent = frame.document.nodes.iter().position(|parent| parent.children.contains(&node));
                let Some(parent) = parent else {
                    if node != 0 { return Err("Disconnected blank Node metadata is unsupported.".into()); }
                    steps.reverse();
                    self.state.work(work)?;
                    return Ok(super::script_position::PositionPath { root: window(index)+1, steps, attribute: None });
                };
                let ordinal = frame.document.nodes[parent].children.iter().position(|&child| child == node).unwrap();
                steps.try_reserve(1).map_err(|_| "Unable to allocate blank Node position path.")?;
                steps.push(ordinal);
                node = parent;
            }
            Err("Cyclic blank Node position metadata.".into())
        })())
    }

    pub(super) fn frame_context(&self, handle: u32) -> Option<crate::javascript::WindowContext> {
        let (index, offset) = frame_handle(handle)?;
        (offset == 0 && index < self.state.blank_frames.len()).then(|| crate::javascript::WindowContext {
            document: window(index) + 1, location: window(index) + 2,
        })
    }

    pub(super) fn frame_window_access(&self, caller: Option<u32>, target: u32) -> Result<(), String> {
        let origin = |handle| {
            if handle == WINDOW { self.state.location.as_ref().map(|view| &view.source) }
            else { frame_handle(handle).filter(|(_, offset)| *offset == 0)
                .and_then(|(index, _)| self.state.blank_frames.get(index)).map(|frame| &frame.origin) }
        };
        let caller = caller.and_then(origin).and_then(|location| location.url.as_ref());
        let target = origin(target).and_then(|location| location.url.as_ref());
        if matches!((caller, target), (Some(a), Some(b)) if a.same_origin(b)) { Ok(()) }
        else { Err("Foreign Window access requires a verified same-origin web context.".into()) }
    }

    pub(super) fn frame_eval(&self, handle: u32) -> Option<Result<bool, String>> {
        let (index, offset) = frame_handle(handle)?;
        Some(if offset != 0 { Err("String compilation requires a Window context.".into()) }
        else { self.state.blank_frames.get(index).ok_or_else(|| "Unknown blank Window.".into())
            .map(|frame| frame.policy.allows_eval().is_ok()) })
    }

    pub(super) fn frame_native_property(&self, handle: u32, name: &str) -> bool {
        self.frame_context(handle).is_some() && (self.owns_global_property(name)
            || matches!(name, "parent" | "top" | "frameElement" | "closed" | "length" | "origin"
                | "setTimeout" | "clearTimeout" | "fetch" | "XMLHttpRequest" | "frames"))
    }

    pub(super) fn frame_content(&self, node: usize, property: &str) -> Option<Value> {
        if tag(self.doc, node) != "iframe" || !matches!(property, "contentWindow" | "contentDocument") { return None; }
        Some(self.state.blank_frames.iter().enumerate().rev()
            .find(|(_, frame)| frame.owner == node && frame.connected)
            .filter(|(_, frame)| property != "contentDocument" || matches!((self.state.location.as_ref().and_then(|view| view.source.url.as_ref()), frame.origin.url.as_ref()), (Some(a),Some(b)) if a.same_origin(b)))
            .map_or(Value::Null, |(index, _)| Value::Host(window(index) + u32::from(property == "contentDocument"))))
    }

    pub(super) fn validate_blank_frame(&mut self, node: usize) -> Result<(), String> {
        if !self.state.dom.created.contains(&node) || !self.doc.nodes[node].children.is_empty()
            || self.doc.attr(node, "src").is_some() || self.doc.attr(node, "srcdoc").is_some()
            || self.doc.attr(node, "sandbox").is_some()
        { return Err("Only created, src-absent initial blank iframes without srcdoc, sandbox or children are implemented.".into()); }
        if self.doc.attr(node, "hidden").is_none()
            && !matches!(self.get_style(node, "display")?, Value::String(display) if display.eq_ignore_ascii_case("none"))
        { return Err("Blank iframe painting is unavailable; an authored hidden attribute or inline display:none is required.".into()); }
        Ok(())
    }

    pub(super) fn prepare_blank_frames(&mut self, nodes: &[usize], connected: bool, parent: usize) -> Result<Vec<BlankFrame>, String> {
        let owners: Vec<_> = nodes.iter().copied().filter(|&node| tag(self.doc, node) == "iframe").collect();
        if !connected || owners.is_empty() { return Ok(Vec::new()); }
        self.state.dynamic.policy.allows_initial_blank_frame()?;
        if owners.len() != 1 || nodes.len() != 1 {
            return Err("Only direct insertion of one initial blank iframe is implemented.".into());
        }
        if self.state.blank_frames.len().saturating_add(owners.len()) > MAX_BLANK_FRAMES {
            return Err("Blank frame context limit reached (8 retained contexts).".into());
        }
        if self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes).saturating_add(owners.len() * 4) > self.state.node_limit {
            return Err("DOM appended-node limit reached (256 shared with child documents).".into());
        }
        let origin = self.state.location.as_ref().map(|view| &view.source)
            .filter(|location| location.url.is_some()).ok_or("Blank frames currently require a committed web origin; local-file contexts remain unavailable.")?;
        let base = self.state.dynamic.base.as_ref().unwrap_or(origin);
        let bytes = owners.len().saturating_mul(self.state.dynamic.policy.clone_work_bytes()
            .saturating_add(origin.display().len().saturating_add(base.display().len()).saturating_mul(4))
            .saturating_add(std::mem::size_of::<BlankFrame>() + 4 * std::mem::size_of::<Node>() + 64));
        // No parent link, current-frame mapping or context identity changes
        // before every validation and aggregate quota check has succeeded.
        for &owner in &owners {
            if self.state.listeners.iter().any(|listener| listener.event == "load" && listener.target == node_handle(owner)) {
                return Err("Synchronous blank-frame load listener dispatch is not implemented.".into());
            }
            let mut ancestor = Some(parent);
            while let Some(node) = ancestor {
                self.state.work(1)?;
                if self.state.listeners.iter().any(|listener| listener.event == "load"
                    && listener.target == node_handle(node)
                    && listener.options.capture)
                { return Err("Synchronous blank-frame load listener dispatch is not implemented.".into()); }
                ancestor = self.state.tree.parents[node];
            }
        }
        if self.state.listeners.iter().any(|listener| listener.event == "load" && listener.options.capture
            && matches!(listener.target, WINDOW | DOCUMENT)) {
            return Err("Synchronous blank-frame load capture dispatch is not implemented.".into());
        }
        self.state.work(owners.len() * 4)?;
        self.state.replace_bytes(0, bytes)?;
        let origin = self.state.location.as_ref().unwrap().source.clone();
        let base = self.state.dynamic.base.clone().unwrap_or_else(|| origin.clone());
        let url = origin.url.as_ref().unwrap();
        let serialized_origin = format!("{}://{}", if url.secure { "https" } else { "http" }, url.host_header());
        Ok(owners.into_iter().map(|owner| {
            let mut document = crate::html::parse("<html><head></head><body></body></html>");
            document.scripting_enabled = self.doc.scripting_enabled;
            BlankFrame { owner, connected: true, document, origin: origin.clone(), serialized_origin: serialized_origin.clone(), base: base.clone(),
                policy: self.state.dynamic.policy.clone() }
        }).collect())
    }

    pub(super) fn commit_blank_frames(&mut self, descendants: &[usize], frames: Vec<BlankFrame>) {
        for frame in &mut self.state.blank_frames {
            if descendants.contains(&frame.owner) { frame.connected = false; }
        }
        self.state.blank_frame_nodes += frames.iter().map(|frame| frame.document.nodes.len()).sum::<usize>();
        self.state.blank_frames.extend(frames);
        // With no observable load listeners, the initial blank load has no
        // callbacks. Never fabricate a resource request or a deferred load.
    }

    pub(super) fn frame_attribute_guard(&self, node: usize, name: &str) -> Result<(), String> {
        if tag(self.doc,node)=="iframe" && name=="srcdoc" {
            let connected=self.state.dom.connected.get(node)==Some(&true);
            let created=self.state.dom.created.contains(&node);
            let published=self.state.blank_frames.iter().any(|frame|frame.owner==node);
            if connected || !created || published {
                return Err(format!("srcdoc navigation is not implemented. Receiver node={node}; connected={connected}; created={created}; retained-context={published}."));
            }
        }
        if tag(self.doc, node) == "iframe" && self.state.dom.connected.get(node) == Some(&true) {
            return Err(format!("Changing connected blank-frame attribute {name:?} is not implemented."));
        }
        Ok(())
    }

    pub(super) fn iframe_srcdoc_property(&self,node:usize)->bool {
        self.doc.nodes.get(node).is_some_and(|item|item.namespace==crate::html::Namespace::Html && tag(self.doc,node)=="iframe")
    }

    pub(super) fn iframe_srcdoc_get(&mut self,node:usize)->Result<Value,String> {
        if self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign {
            return Err("srcdoc reflection requires known HTML namespace provenance.".into());
        }
        if !self.iframe_srcdoc_property(node) {return Ok(Value::Undefined);}
        let NodeKind::Element {attrs,..}=&self.doc.nodes[node].kind else {unreachable!()};
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|(name,_)|name.len()).sum())?;
        let text=self.doc.attr(node,"srcdoc").unwrap_or("");
        if text.len()>self.state.limits.host.string_bytes {return Err("srcdoc exceeds the host string bound.".into());}
        self.state.copies(text.len())?;
        Ok(Value::String(text.to_owned()))
    }

    pub(super) fn frame_get(&mut self, handle: u32, property: &str) -> Option<Result<Value, String>> {
        let (index, offset) = frame_handle(handle)?;
        let result = (|| {
            self.frame_window_access(Some(WINDOW), window(index))?;
            let frame = self.state.blank_frames.get(index).ok_or("Unknown blank-frame handle.")?;
            let value = match offset {
                0 => match property {
                    "window" | "self" | "globalThis" => Value::Host(handle),
                    "document" => Value::Host(window(index) + 1),
                    "location" => Value::Host(window(index) + 2),
                    "parent" | "top" => Value::Host(WINDOW),
                    "frameElement" => if frame.connected { Value::Host(node_handle(frame.owner)) } else { Value::Null },
                    "closed" => Value::Bool(!frame.connected),
                    "length" => Value::Number(0.0),
                    "origin" => Value::String(frame.serialized_origin.clone()),
                    _ => return Err(format!("Blank child Window property {property:?} is not implemented.")),
                },
                2 => match property {
                    "href" => Value::String("about:blank".into()),
                    "protocol" => Value::String("about:".into()),
                    "pathname" => Value::String("blank".into()),
                    "origin" => Value::String("null".into()),
                    "host" | "hostname" | "port" | "search" | "hash" => Value::String(String::new()),
                    _ => return Err(format!("Blank child Location property {property:?} is not implemented.")),
                },
                1 | 4..=6 => {
                    let node = if offset == 1 { 0 } else { (offset - 3) as usize };
                    let item = frame.document.nodes.get(node).ok_or("Invalid blank-document node.")?;
                    let parent = frame.document.nodes.iter().position(|item| item.children.contains(&node));
                    let native = |node| Value::Host(document_node(index, node));
                    let element = |name: &str| frame.document.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name)).map_or(Value::Null, native);
                    match property {
                        name if super::dom_tree::node_constant(name).is_some() =>
                            Value::Number(f64::from(super::dom_tree::node_constant(name).unwrap())),
                        "defaultView" if node == 0 => {
                            if !frame.connected { return Err("Retained child Document browsing-context teardown is not implemented.".into()); }
                            Value::Host(window(index))
                        },
                        "URL" | "documentURI" if node == 0 => Value::String("about:blank".into()),
                        "baseURI" => Value::String(frame.base.display()),
                        "location" if node == 0 => Value::Host(window(index) + 2),
                        "body" if node == 0 => element("body"),
                        "head" if node == 0 => element("head"),
                        "documentElement" if node == 0 => element("html"),
                        "readyState" if node == 0 => Value::String("complete".into()),
                        "contentType" if node == 0 => Value::String(frame.document.content_type.clone()),
                        "title" if node == 0 => Value::String(frame.document.title.clone()),
                        "nodeName" => Value::String(if node == 0 { "#document".into() } else { tag(&frame.document, node).to_ascii_uppercase() }),
                        "tagName" if node != 0 => Value::String(tag(&frame.document, node).to_ascii_uppercase()),
                        "tabIndex" if node != 0 => Value::Number(-1.0),
                        "nodeType" => Value::Number(if node == 0 { 9.0 } else { 1.0 }),
                        "ownerDocument" => if node == 0 { Value::Null } else { native(0) },
                        "parentNode" => parent.map_or(Value::Null, native),
                        "parentElement" => parent.filter(|&node| node != 0).map_or(Value::Null, native),
                        "firstChild" => item.children.first().copied().map_or(Value::Null, native),
                        "lastChild" => item.children.last().copied().map_or(Value::Null, native),
                        "nextSibling" | "previousSibling" => parent.and_then(|parent| {
                            let siblings = &frame.document.nodes[parent].children;
                            let at = siblings.iter().position(|&sibling| sibling == node)?;
                            if property == "nextSibling" { siblings.get(at + 1) } else { at.checked_sub(1).and_then(|at| siblings.get(at)) }
                        }).copied().map_or(Value::Null, native),
                        "isConnected" => Value::Bool(true),
                        "textContent" => if node == 0 { Value::Null } else { Value::String(String::new()) },
                        _ => return Err(format!("Read-only blank child Document/node property {property:?} is not implemented.")),
                    }
                },
                _ => return Err("Invalid blank-frame handle.".into()),
            };
            self.state.copies(if let Value::String(text) = &value { text.len() } else { 0 })?;
            Ok(value)
        })();
        Some(result)
    }

    pub(super) fn is_frame_handle(&self, handle: u32) -> bool { frame_handle(handle).is_some() }

    pub(super) fn frame_default_namespace(&mut self, handle: u32) -> Option<Result<Option<&'static str>, String>> {
        let (index, offset) = frame_handle(handle)?;
        Some((|| {
            self.frame_window_access(Some(WINDOW), window(index))?;
            if !matches!(offset, 1 | 4..=6) { return Err("Invalid blank child Node receiver.".into()); }
            let frame = self.state.blank_frames.get(index).ok_or("Unknown blank-frame handle.")?;
            let node = if offset == 1 { 0 } else { (offset - 3) as usize };
            // Blank documents are immutable and only expose Document/Elements.
            // Their existing metadata validation remains mandatory.
            let item = frame.document.nodes.get(node).ok_or("Invalid blank-document node.")?;
            let mut work = 0;
            let mut namespace = if node == 0 { None } else { Some(item.namespace) };
            if node == 0 {
                for &child in &frame.document.nodes[0].children {
                    work += 1;
                    let item = frame.document.nodes.get(child).ok_or("Invalid child namespace node.")?;
                    if matches!(item.kind, NodeKind::Element { .. }) { namespace = Some(item.namespace); break; }
                }
            }
            self.state.work(work + 1)?;
            match namespace {
                None => Ok(None),
                Some(crate::html::Namespace::Html) => Ok(Some("http://www.w3.org/1999/xhtml")),
                Some(_) => Err("Blank child namespace context is unsupported.".into()),
            }
        })())
    }

    pub(super) fn frame_node_constant_readonly(&self, handle: u32) -> bool {
        let Some((index, offset)) = frame_handle(handle) else { return false; };
        if !matches!(offset, 1 | 4..=6) { return false; }
        let Some(frame) = self.state.blank_frames.get(index) else { return false; };
        let node = if offset == 1 { 0 } else { (offset - 3) as usize };
        frame.document.nodes.get(node).is_some()
            && self.frame_window_access(Some(WINDOW), window(index)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(source: &str, policy: &crate::csp::PolicySet) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse("<html><head></head><body><p>Original frame fixture</p></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let origin = crate::navigation::Location { url: Some(crate::url::parse("https://example.test/parent/page").unwrap().0),
            path: None, fragment: String::new(), root: None };
        let base = crate::navigation::Location { url: Some(crate::url::parse("https://example.test/assets/").unwrap().0),
            ..origin.clone() };
        page.set_location(&origin);
        page.set_resource_base(&base);
        let result = page.start_with_policy(&mut doc, &mut forms, &[Script { node: 0, label: "Original blank-frame fixture".into(), source: source.into() }], policy);
        (doc, forms, page, result)
    }

    const CREATE: &str = "var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow;";

    #[test]
    fn srcdoc_reflection_cannot_insert_a_fake_document_or_navigate_a_retained_context() {
        for source in [
            "var f=document.createElement('iframe');f.hidden=true;f.srcdoc='';document.body.appendChild(f);",
            "var f=document.createElement('iframe');f.hidden=true;f.srcdoc='<script>parent.changed=1</script>';document.body.appendChild(f);",
        ] {
            let (doc,_,page,outcome)=start(source,&crate::csp::PolicySet::default());
            assert_eq!(outcome.errors,1,"{}",page.report());
            assert!(page.state.blank_frames.is_empty());
            let f=page.state.dom.created.iter().copied().find(|&node|tag(&doc,node)=="iframe").unwrap();
            assert!(!page.state.dom.connected[f]);
        }
        for write in [
            "f.srcdoc='changed';",
            "f.parentNode.removeChild(f);f.srcdoc='changed';",
            "f.setAttribute('srcdoc','changed');",
        ] {
            let (mut doc,mut forms,mut page,outcome)=start(CREATE,&crate::csp::PolicySet::default());
            assert_eq!(outcome.errors,0,"{}",page.report());
            let outcome=page.console_eval(&mut doc,&mut forms,write);
            assert_eq!(outcome.errors,1,"{}",page.report());
            assert_eq!(page.state.blank_frames.len(),1);
            assert_eq!(doc.attr(page.state.blank_frames[0].owner,"srcdoc"),None);
        }
        let (mut doc,mut forms,mut page,outcome)=start("var f=document.createElement('iframe');f.hidden=true;",&crate::csp::PolicySet::default());
        assert_eq!(outcome.errors,0,"{}",page.report());
        let outcome=page.console_eval(&mut doc,&mut forms,"var calls=0;f.srcdoc={toString:function(){calls++;document.body.appendChild(f);return 'changed';}};");
        assert_eq!(outcome.errors,1,"{}",page.report());
        assert_eq!(page.state.blank_frames.len(),1);
        assert_eq!(doc.attr(page.state.blank_frames[0].owner,"srcdoc"),None);
        let outcome=page.console_eval(&mut doc,&mut forms,"if(calls!==1||f.contentWindow===null)throw 'callback effects';");
        // Coercion connected the frame once; the rejected setter kept that real
        // callback effect and did not create a second context.
        assert_eq!(outcome.errors,0,"{}",page.report());
    }

    #[test]
    fn native_instanceof_blank_objects_keep_owner_realm_and_origin_validation() {
        let source=format!("{CREATE}{}",r#"
var childObject=w.Object,childArray=w.Array,foreignDocument=w.document;
if(!(foreignDocument instanceof childObject)||foreignDocument instanceof Object||foreignDocument instanceof childArray||document instanceof childObject)throw 'owner realm';
function Alias(){};Alias.prototype=childObject.prototype;if(!(foreignDocument instanceof Alias))throw 'foreign prototype identity';
var bound=childObject.bind(null);if(!(foreignDocument instanceof bound))throw 'bound target';
var check=w.Function('value','return value instanceof Object;');
if(check(document)!==false||check(foreignDocument)!==true)throw 'parked main/current child roots';
document.title='native-child-instanceof-ok';
"#);
        let (mut doc,mut forms,mut page,outcome)=start(&source,&crate::csp::PolicySet::default());
        assert_eq!(outcome.errors,0,"{}",page.report());
        assert_eq!(doc.title,"native-child-instanceof-ok");
        {
            let bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
            for handle in [window(0)+3,window(0)+7,window(1)+1,node_handle(bridge.doc.nodes.len()+99)] {
                assert!(bridge.instance_ancestry(handle).is_none());
            }
        }
        page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,fragment:String::new(),root:None});
        let result=page.console_eval(&mut doc,&mut forms,"foreignDocument instanceof childObject;");
        assert_eq!(result.errors,1,"{}",page.report());
        assert!(page.report().contains("same-origin"));
    }

    #[test]
    fn node_position_blank_documents_have_real_ancestry_and_disconnected_roots() {
        let source = format!("{CREATE}{}", r#"
var d=w.document,html=d.documentElement,head=d.head,body=d.body;
if(d.compareDocumentPosition(html)!==20||html.compareDocumentPosition(d)!==10||head.compareDocumentPosition(body)!==4||body.compareDocumentPosition(head)!==2)throw 'blank physical tree';
if(d.compareDocumentPosition(d)!==0||document.compareDocumentPosition.call(d,body)!==20)throw 'identity/borrowing';
var position=document.compareDocumentPosition(d),reverse=d.compareDocumentPosition(document);
if((position!==35&&position!==37)||(position^reverse)!==6||document.compareDocumentPosition(body)!==position)throw 'separate documents';
var second=document.createElement('iframe');second.hidden=true;document.body.appendChild(second);
var secondBody=second.contentDocument.body,a=body.compareDocumentPosition(secondBody),b=secondBody.compareDocumentPosition(body);
if((a!==35&&a!==37)||(a^b)!==6||body.compareDocumentPosition(secondBody)!==a)throw 'separate child contexts';
var saved=d.compareDocumentPosition;document.title='child-position-ok';
"#);
        let (mut doc, mut forms, mut page, outcome) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "child-position-ok");
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for handle in [window(0)+3, window(0)+7, window(MAX_BLANK_FRAMES-1)+6] {
            assert!(bridge.method(handle, "compareDocumentPosition").is_none());
            assert!(bridge.call(handle, "compareDocumentPosition", &[Value::Host(DOCUMENT)]).is_err());
        }
        page.set_location(&crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None });
        let outcome = page.console_eval(&mut doc, &mut forms, "saved.call(d,body);");
        assert_eq!(outcome.errors, 1, "{}", page.report());
    }

    #[test]
    fn active_child_window_keys_use_real_realm_names_flags_order_and_prototypes() {
        let source = format!("{CREATE}{}", r#"
var parentSecret=17;
w.eval("var child_z=1;window.child_a=2;let child_lexical=3;const child_const=4;");
var keys=w.Object.keys(w),all=w.Object.getOwnPropertyNames(w);
if(Object.getPrototypeOf(keys)!==w.Array.prototype||Object.getPrototypeOf(all)!==w.Array.prototype)throw 'result realm';
for(var name of ['window','self','document','location','parent','top','frameElement','closed','length','origin']){if(keys.indexOf(name)<0)throw 'native own names';}
if(keys.indexOf('globalThis')>=0||all.indexOf('globalThis')<0||keys.indexOf('Object')>=0||all.indexOf('Object')<0)throw 'native/intrinsic attributes';
for(var name of ['parentSecret','child_lexical','child_const','setTimeout']){if(all.indexOf(name)>=0)throw 'unavailable/lexical/foreign names';}
if(keys.indexOf('child_z')<0||keys.indexOf('child_a')<keys.indexOf('child_z')||all.indexOf('URL')>all.indexOf('child_z'))throw 'creation order';
var inherited=w.Function("Object.prototype.child_inherited=1;Object.defineProperty(this,'child_hidden',{value:2,enumerable:false});Object.prototype.child_hidden=3;var names=[];for(var name in this){if(name.indexOf('child_')===0)names.push(name);}delete Object.prototype.child_inherited;delete Object.prototype.child_hidden;return names;")();
if(inherited.join(',')!=='child_z,child_a,child_inherited')throw 'inherited names/shadowing';
w.eval('delete window.URL;');if(w.Object.getOwnPropertyNames(w).indexOf('URL')>=0)throw 'deleted intrinsic resurrected';
if(parentSecret!==17||typeof child_z!=='undefined')throw 'realm leakage';document.title='child-window-keys-ok';
"#);
        let (mut doc, mut forms, mut page, outcome) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "child-window-keys-ok");
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(bridge.window_properties(window(0)+1).is_err());
        bridge.state.host_work = 0;
        assert!(bridge.window_properties(window(0)).is_err());
        page.set_location(&crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None });
        let outcome = page.console_eval(&mut doc, &mut forms, "w.Object.keys(w);");
        assert_eq!(outcome.errors, 1, "{}", page.report());
    }

    #[test]
    fn saved_child_window_key_builtins_validate_origin_before_deferred_allocation() {
        let source = format!("{CREATE}var savedKeys=w.Object.keys,savedHasOwn=w.Object.hasOwn;");
        let (mut doc, mut forms, mut page, outcome) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let before = page.runtime.window_allocation_state_for_test();
        page.set_location(&crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None });
        for source in ["savedKeys(w);", "savedHasOwn(w,'Uint8Array');"] {
            let outcome = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(outcome.errors, 1, "{}", page.report());
            assert_eq!(page.runtime.window_allocation_state_for_test(), before);
        }
    }

    #[test]
    fn tab_index_child_getter_keeps_actual_node_and_origin_validation() {
        let source = format!("{CREATE}for(var n of [w.document.documentElement,w.document.head,w.document.body]){{if(!('tabIndex' in n)||n.tabIndex!==-1)throw 'child tabIndex';}}var saved=w.document.body;");
        let (mut doc, mut forms, mut page, outcome) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for handle in [window(0)+3, window(0)+7, window(MAX_BLANK_FRAMES-1)+6] {
            assert!(bridge.get(handle, "tabIndex").is_err());
        }
        let location = crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None };
        page.set_location(&location);
        let outcome = page.console_eval(&mut doc, &mut forms, "saved.tabIndex;");
        assert_eq!(outcome.errors, 1, "{}", page.report());
    }

    #[test]
    fn default_namespace_blank_nodes_borrow_and_keep_origin_and_handle_boundaries() {
        let source = format!("{CREATE}{}", r#"
var saved=w.document,method=saved.isDefaultNamespace,uri='http://www.w3.org/1999/xhtml';
for(var n of [saved,saved.documentElement,saved.head,saved.body]){
if(!('isDefaultNamespace' in n)||!n.isDefaultNamespace(uri)||n.isDefaultNamespace(null)||!document.isDefaultNamespace.call(n,uri))throw 'child namespace';}
if(!method.call(document,uri)||!method.call(document.createTextNode('x'),null))throw 'borrowed child method';
var bad=false,calls=0;try{method.call(w,{toString:function(){calls++;return uri;}});}catch(e){bad=e.name==='TypeError';}
if(!bad||calls!==0)throw 'Window brand';document.title='child-namespace-ok';
"#);
        let (mut doc, mut forms, mut page, outcome) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "child-namespace-ok");
        {
            let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
            for handle in [window(0), window(0)+3, window(0)+7, window(MAX_BLANK_FRAMES-1)+1] {
                assert!(bridge.method(handle, "isDefaultNamespace").is_none());
                assert!(bridge.call(handle, "isDefaultNamespace", &[Value::Null]).is_err());
            }
        }
        let location = crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None };
        page.set_location(&location);
        for source in ["saved.isDefaultNamespace(null);", "'isDefaultNamespace' in saved;", "method.call(saved,null);"] {
            let result = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(result.errors, 1, "{source}: {}", page.report());
        }
    }

    #[test]
    fn real_blank_frame_identity_topology_inheritance_and_foreign_calls() {
        let source = format!("{CREATE}{}", r#"
var d=f.contentDocument;
if(w===window||d===document||w.document!==d||d.defaultView!==w||w.window!==w||w.self!==w||w.globalThis!==w)throw 'context identity';
if(w.parent!==window||w.top!==window||w.frameElement!==f||w.closed||w.length!==0)throw 'context links';
if(d.URL!=='about:blank'||d.documentURI!=='about:blank'||w.location.href!=='about:blank'||d.baseURI!=='https://example.test/assets/'||w.origin!=='https://example.test')throw 'inherited metadata';
if(d.nodeType!==9||d.nodeName!=='#document'||d.parentNode!==null||d.ownerDocument!==null)throw 'document root';
if(d.documentElement.tagName!=='HTML'||d.documentElement.parentNode!==d||d.documentElement.firstChild!==d.head||d.documentElement.lastChild!==d.body||d.head.nextSibling!==d.body||d.body.previousSibling!==d.head||d.body.ownerDocument!==d||d.body.firstChild!==null||d.body.textContent!=='')throw 'actual child tree';
if(w.Array===Array||w.Array.prototype===Array.prototype||Object.getPrototypeOf(w.Array(1))!==w.Array.prototype||Object.getPrototypeOf(w?.Array(1))!==w.Array.prototype)throw 'child intrinsics';
if(w.Function('return this;')()!==w||w.Function('return parent.Array;')()!==Array)throw 'child function globals';
Array.prototype.parentMutation=7;if(w.Array.prototype.parentMutation!==undefined)throw 'prototype alias';
w.eval('var childOnly=23;');if(w.childOnly!==23||typeof childOnly!=='undefined')throw 'global alias';
if(w.Function('return Symbol.iterator;')()!==Symbol.iterator)throw 'shared symbol';
document.title='blank-frame-ok';
"#);
        let (doc, _, page, result) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "blank-frame-ok");
        assert_eq!(page.state.blank_frames.len(), 1);
        assert_eq!(page.state.blank_frame_nodes, 4);
        assert_eq!(page.state.blank_frames[0].document.nodes.len(), 4);
        assert!(page.state.dynamic.pending.is_empty());
        assert_eq!(page.state.declined_images, 0);
    }

    #[test]
    fn node_constants_on_real_child_nodes_keep_readonly_and_origin_checks() {
        let source = format!("{CREATE}{}", r#"
var nodes=[w.document,w.document.documentElement,w.document.head,w.document.body],effects=0;
for(var node of nodes){if(!('DOCUMENT_TYPE_NODE' in node)||node.DOCUMENT_TYPE_NODE!==10||node.DOCUMENT_POSITION_FOLLOWING!==4)throw 'child constants';
node.DOCUMENT_TYPE_NODE=(effects++,0);if(node.DOCUMENT_TYPE_NODE!==10)throw 'readonly';
var caught=false;try{(function(){'use strict';node.DOCUMENT_TYPE_NODE=0;})();}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'strict readonly';}
if(effects!==4||w.document.nodeType!==w.document.DOCUMENT_NODE||w.document.body.nodeType!==w.document.body.ELEMENT_NODE)throw 'child type';
var saved=w.document;document.title='child-node-constants-ok';
"#);
        let (mut doc, mut forms, mut page, outcome) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "child-node-constants-ok");
        let location = crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None };
        page.set_location(&location);
        for operation in ["saved.DOCUMENT_TYPE_NODE;", "'DOCUMENT_TYPE_NODE' in saved;", "saved.DOCUMENT_TYPE_NODE=0;"] {
            let outcome = page.console_eval(&mut doc, &mut forms, operation);
            assert_eq!(outcome.errors, 1, "{operation}: {}", page.report());
        }
    }

    #[test]
    fn node_constants_reject_missing_child_and_reserved_frame_offsets() {
        let (mut doc, mut forms, mut page, outcome) = start(CREATE, &crate::csp::PolicySet::default());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for handle in [window(0) + 3, window(0) + 7, window(MAX_BLANK_FRAMES - 1) + 1] {
            assert!(bridge.get(handle, "DOCUMENT_TYPE_NODE").is_err());
            assert!(bridge.has(handle, "DOCUMENT_TYPE_NODE").is_err());
            assert!(!bridge.readonly_property(handle, "DOCUMENT_TYPE_NODE"));
            assert!(bridge.set(handle, "DOCUMENT_TYPE_NODE", Value::Number(0.0)).is_err());
        }
    }

    #[test]
    fn blank_frame_csp_is_an_independent_inherited_snapshot() {
        let mut policy = crate::csp::PolicySet::default();
        policy.add_header("script-src 'unsafe-inline'");
        let source = format!("{CREATE}var blocked=0;try{{w.Function('return 1;');}}catch(e){{if(Object.getPrototypeOf(e)===w.EvalError.prototype)blocked++;}}try{{w.eval('1;');}}catch(e){{if(Object.getPrototypeOf(e)===w.EvalError.prototype)blocked++;}}if(blocked!==2)throw 'inherited CSP';");
        let (mut doc, mut forms, mut page, result) = start(&source, &policy);
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(page.state.blank_frames[0].policy, policy);
        page.state.dynamic.policy = crate::csp::PolicySet::default();
        page.state.eval_allowed = true;
        let result = page.console_eval(&mut doc, &mut forms, "try{w.Function('return 1;');throw 'missing denial';}catch(e){if(Object.getPrototypeOf(e)!==w.EvalError.prototype)throw e;}");
        assert_eq!(result.errors, 0, "{}", page.report());
    }

    #[test]
    fn blank_frame_removal_and_reinsertion_keep_old_identity_without_aliasing() {
        let source = format!("{CREATE}{}", r#"
var old=w,oldDoc=f.contentDocument,oldArray=w.Array;
document.body.removeChild(f);
if(f.contentWindow!==null||f.contentDocument!==null||!old.closed||old.frameElement!==null)throw 'removal';
document.body.appendChild(f);
var fresh=f.contentWindow;
if(fresh===old||f.contentDocument===oldDoc||fresh.Array===oldArray||fresh.closed||!old.closed)throw 'reinsert identity';
if(old.Function('return Array;')()!==oldArray||fresh.Function('return Array;')()!==fresh.Array)throw 'retained realm';
document.title='lifecycle-ok';
"#);
        let (doc, _, page, result) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "lifecycle-ok");
        assert_eq!(page.state.blank_frames.len(), 2);
        assert!(!page.state.blank_frames[0].connected && page.state.blank_frames[1].connected);
    }

    #[test]
    fn first_use_foreign_window_observations_reject_without_parent_effects() {
        for operation in ["w[Symbol('x')]", "Symbol('x') in w", "delete w[Symbol('x')]",
            "Object.getOwnPropertySymbols(w)", "Object.getOwnPropertyDescriptor(w,Symbol('x'))",
            "Object.getOwnPropertyDescriptor(w,'Array')", "Object.defineProperty(w,'x',{value:1})",
            "Object.defineProperties(w,{})", "Object.keys(w)", "Object.hasOwn(w,'Array')",
            "Object.prototype.hasOwnProperty.call(w,Symbol('x'))", "Object.prototype.propertyIsEnumerable.call(w,'Array')",
            "w.parentOnly=7", "delete w.Array", "'Array' in w", "w.location.href='https://other.test/'",
            "f.contentDocument.body.textContent='changed'", "f.hidden=false", "f.style.display='block'"] {
            let (mut doc, mut forms, mut page, result) = start(&format!("var parentOnly=3;{CREATE}"), &crate::csp::PolicySet::default());
            assert_eq!(result.errors, 0, "{}", page.report());
            let result = page.console_eval(&mut doc, &mut forms, operation);
            assert_eq!(result.errors, 1, "unexpected success for {operation}: {}", page.report());
            let result = page.console_eval(&mut doc, &mut forms, "if(parentOnly!==3||f.parentNode!==document.body||f.contentDocument.body.firstChild!==null)throw 'partial effect';");
            assert_eq!(result.errors, 0, "{operation}: {}", page.report());
        }
    }

    #[test]
    fn blank_window_member_resolution_and_optional_calls_keep_order() {
        let source = format!("{CREATE}{}", r#"
var effects=0;
if(w.missing?.(++effects)!==undefined||effects!==0)throw 'optional skip';
try{w?.missing(++effects);}catch(e){if(Object.getPrototypeOf(e)!==TypeError.prototype)throw e;}
if(effects!==1)throw 'argument before callability';
document.title='call-order-ok';
"#);
        let (doc, _, page, result) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "call-order-ok");
        let (mut doc, mut forms, mut page, result) = start(CREATE, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        let result = page.console_eval(&mut doc, &mut forms, "var effects=0;w.fetch(++effects);");
        assert_eq!(result.errors, 1, "{}", page.report());
        let result = page.console_eval(&mut doc, &mut forms, "if(effects!==0)throw 'lookup must precede arguments';");
        assert_eq!(result.errors, 0, "{}", page.report());
    }

    #[test]
    fn blank_frame_creation_quota_failure_is_atomic_and_contexts_are_bounded() {
        let (mut doc, mut forms, mut page, result) = start("var f=document.createElement('iframe');f.hidden=true;", &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        page.state.retained_limit = page.state.retained;
        let result = page.console_eval(&mut doc, &mut forms, "document.body.appendChild(f);");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert!(page.state.blank_frames.is_empty());
        assert_eq!(page.state.blank_frame_nodes, 0);
        let result = page.console_eval(&mut doc, &mut forms, "if(f.parentNode!==null||f.contentWindow!==null)throw 'partial insertion';");
        assert_eq!(result.errors, 0, "{}", page.report());

        let source = "var count=0;for(var i=0;i<9;i++){var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);count++;document.body.removeChild(f);}";
        let (mut doc, mut forms, mut page, result) = start(source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 1, "{}", page.report());
        assert!(page.report().contains("8 retained contexts"));
        assert_eq!(page.state.blank_frames.len(), MAX_BLANK_FRAMES);
        assert_eq!(page.state.blank_frame_nodes, 32);
        let result = page.console_eval(&mut doc, &mut forms, "if(count!==8||f.parentNode!==null||f.contentWindow!==null)throw 'bounded atomic insertion';");
        assert_eq!(result.errors, 0, "{}", page.report());
    }

    #[test]
    fn visible_frames_and_observable_loads_reject_before_connection() {
        for source in [
            "var f=document.createElement('iframe');document.body.appendChild(f);",
            "var f=document.createElement('iframe');f.hidden=true;f.addEventListener('load',function(){document.title='false load';});document.body.appendChild(f);",
            "var f=document.createElement('iframe');f.hidden=true;document.body.addEventListener('load',function(){document.title='false load';},true);document.body.appendChild(f);",
        ] {
            let (mut doc, mut forms, mut page, result) = start(source, &crate::csp::PolicySet::default());
            assert_eq!(result.errors, 1, "{}", page.report());
            assert!(page.state.blank_frames.is_empty());
            assert_ne!(doc.title, "false load");
            let result = page.console_eval(&mut doc, &mut forms, "if(f.parentNode!==null||f.contentWindow!==null)throw 'partial insertion';");
            assert_eq!(result.errors, 0, "{}", page.report());
        }
    }

    #[test]
    fn frame_collection_and_nodelist_handles_are_disjoint() {
        let source = format!("var tags=document.getElementsByTagName('p'),list=document.querySelectorAll('p');{CREATE}{}", r#"
if(tags.length!==1||list.length!==1||tags[0]!==list[0]||tags[0].tagName!=='P')throw 'collection collision';
if(w===tags||w===list||w.document===tags||f.contentDocument.body.tagName!=='BODY')throw 'frame collision';
document.title='disjoint-ok';
"#);
        let (doc, _, page, result) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "disjoint-ok");
    }

    #[test]
    fn shared_child_node_reservation_blocks_image_and_text_growth() {
        for operation in ["new Image();", "target.textContent='new text';"] {
            let (mut doc, mut forms, mut page, result) = start(&format!("{CREATE}var target=document.createElement('span');document.body.appendChild(target);"), &crate::csp::PolicySet::default());
            assert_eq!(result.errors, 0, "{}", page.report());
            let nodes = doc.nodes.len();
            let frames = page.state.blank_frame_nodes;
            page.state.node_limit = nodes + frames;
            let result = page.console_eval(&mut doc, &mut forms, operation);
            assert_eq!(result.errors, 1, "{operation}: {}", page.report());
            assert!(page.report().contains("appended-node limit"), "{}", page.report());
            assert_eq!(doc.nodes.len(), nodes);
            assert_eq!(page.state.blank_frame_nodes, frames);
            assert_eq!(page.state.blank_frames.len(), 1);
        }
    }

    #[test]
    fn trusted_location_change_does_not_expose_retained_child_documents() {
        let (mut doc, mut forms, mut page, result) = start(&format!("{CREATE}var saved=f.contentDocument;"), &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        let location = crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0),
            path: None, fragment: String::new(), root: None };
        page.set_location(&location);
        let result = page.console_eval(&mut doc, &mut forms, "if(f.contentDocument!==null)throw 'foreign document';");
        assert_eq!(result.errors, 0, "{}", page.report());
        for operation in ["saved.body", "w.Array", "w.location.href"] {
            let result = page.console_eval(&mut doc, &mut forms, operation);
            assert_eq!(result.errors, 1, "{operation}: {}", page.report());
        }
    }

    #[test]
    fn deferred_families_keep_canonical_methods_and_global_mutation_semantics() {
        let source = format!("{CREATE}{}", r#"
var check=w.Function("var canonical=Array.prototype.toString;Object.defineProperty(Array.prototype,'toString',{get:function(){throw 'late getter';},configurable:true});if(Uint8Array.prototype.toString!==canonical)throw 'canonical method';if((0).constructor.EPSILON!==2.220446049250313e-16||(0).constructor.isInteger(2)!==true)throw 'eager Number aliases';if(Object.getOwnPropertyDescriptor(this,'URL').value!==URL)throw 'lazy descriptor';delete this.URL;if(typeof URL!=='undefined')throw 'deleted intrinsic resurrected';var TextEncoder=17;if(TextEncoder!==17)throw 'global override';return new Uint8Array([3])[0];");
if(check()!==3)throw 'deferred families';
document.title='deferred-ok';
"#);
        let (doc, _, page, result) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "deferred-ok");
    }

    #[test]
    fn first_deferred_access_in_nested_foreign_calls_is_bounded_on_one_mib() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let source = format!("{CREATE}var child=w.Function('n','parent','if(n>0)return parent(n-1);return new Uint8Array([42])[0];');function parent(n){{return child(n,parent);}}if(parent(3)!==42)throw 'nested deferred allocation';");
            let (mut doc, mut forms, mut page, result) = start(&source, &crate::csp::PolicySet::default());
            assert_eq!(result.errors, 0, "{}", page.report());
            // Keep the previous capacity witness as a successful case: unused
            // method deferral leaves room for its real second child URL.
            let result = page.console_eval(&mut doc, &mut forms, "document.body.removeChild(f);document.body.appendChild(f);var other=f.contentWindow;other.Function('return new URL(\"https://example.test/\");')();");
            assert_eq!(result.errors, 0, "{}", page.report());
            let result = page.console_eval(&mut doc, &mut forms, "if(other.Function('return new URL(\"https://example.test/\").href;')()!=='https://example.test/')throw 'second child URL';");
            assert_eq!(result.errors, 0, "{}", page.report());
            // An intentional fresh-identity workload still exhausts the same
            // aggregate 1,024-function cap. No callable memoization or cap raise.
            let result = page.console_eval(&mut doc, &mut forms, "for(var i=0;i<1100;i++){(function(){return 1;})();}");
            assert_eq!(result.errors, 1, "{}", page.report());
            assert!(page.report().contains("limit"), "{}", page.report());
            let result = page.console_eval(&mut doc, &mut forms, "if(6*7!==42||parent(1)!==42)throw 'caller recovery';");
            assert_eq!(result.errors, 0, "{}", page.report());
        }).unwrap().join().unwrap();
    }

    #[test]
    fn deferred_constructor_global_function_overrides_preserve_sibling_intrinsics() {
        for (name, sibling) in [("URL", "URLSearchParams"), ("Uint8Array", "DataView"), ("TextEncoder", "TextDecoder")] {
            let source = format!("{CREATE}var original={name};w.eval('function {name}(){{return 17;}}');if(w.{name}()!==17||typeof w.{sibling}!=='function'||{name}!==original)throw 'global function override';document.title='override-ok';");
            let (doc, _, page, result) = start(&source, &crate::csp::PolicySet::default());
            assert_eq!(result.errors, 0, "{name}: {}", page.report());
            assert_eq!(doc.title, "override-ok");
        }
    }

    #[test]
    fn inherited_sandbox_and_policy_limit_reject_before_frame_publication() {
        for header in ["sandbox allow-scripts allow-same-origin", &"x".repeat(65 * 1024)] {
            let mut policy = crate::csp::PolicySet::default();
            policy.add_header(header);
            let (mut doc, mut forms, mut page, result) = start(CREATE, &policy);
            assert_eq!(result.errors, 1, "{}", page.report());
            assert!(page.state.blank_frames.is_empty());
            let result = page.runtime.execute("if(f.parentNode!==null||f.contentWindow!==null)throw 'partial policy insertion';", &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state });
            assert!(result.is_ok(), "{result:?}");
        }
    }

    #[test]
    fn retained_document_does_not_claim_completed_browsing_context_teardown() {
        let source = format!("{CREATE}var oldDoc=f.contentDocument;document.body.removeChild(f);");
        let (mut doc, mut forms, mut page, result) = start(&source, &crate::csp::PolicySet::default());
        assert_eq!(result.errors, 0, "{}", page.report());
        let result = page.console_eval(&mut doc, &mut forms, "oldDoc.defaultView;");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert!(page.report().contains("browsing-context teardown is not implemented"));
    }
}
