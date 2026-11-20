//! Node position over actual physical trees, including detached subtrees/Attrs.
//! https://dom.spec.whatwg.org/#dom-node-comparedocumentposition
use super::*;

pub(super) struct PositionPath {
    pub(super) root: u32,
    pub(super) steps: Vec<usize>,
    pub(super) attribute: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn start(source: &str) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse("<html><body><div id=outer><p id=a title=A>one<span id=s>span</span></p><p id=b title=B>two</p></div><template><span id=inert>hidden</span></template></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let outcome = page.start(&mut doc, &mut forms, &[Script { node: 0, label: "Original Node position fixture".into(), source: source.into() }]);
        (doc, forms, page, outcome)
    }

    #[test]
    fn node_position_tracks_document_text_ancestry_reordering_and_detached_subtrees() {
        let (doc, _, page, outcome) = start(r#"
var outer=document.getElementById('outer'),a=document.getElementById('a'),b=document.getElementById('b'),s=document.getElementById('s'),text=a.firstChild;
if(a.compareDocumentPosition!==a.compareDocumentPosition||a.compareDocumentPosition.length!==1)throw 'method identity/arity';
if(a.compareDocumentPosition(a)!==0||document.compareDocumentPosition(a)!==20||a.compareDocumentPosition(document)!==10)throw 'Document ancestry';
if(a.compareDocumentPosition(text)!==20||text.compareDocumentPosition(a)!==10||text.compareDocumentPosition(s)!==4||s.compareDocumentPosition(text)!==2)throw 'text tree order';
if(a.compareDocumentPosition(b)!==4||b.compareDocumentPosition(a)!==2||outer.compareDocumentPosition(s)!==20)throw 'connected order';
outer.insertBefore(b,a);if(a.compareDocumentPosition(b)!==2||b.compareDocumentPosition(a)!==4)throw 'new sibling order';
outer.removeChild(a);if(a.compareDocumentPosition(s)!==20||text.compareDocumentPosition(s)!==4)throw 'detached subtree ancestry';
var first=a.compareDocumentPosition(b),reverse=b.compareDocumentPosition(a);if((first!==35&&first!==37)||(first^reverse)!==6||a.compareDocumentPosition(b)!==first)throw 'stable reciprocal disconnected order';
outer.appendChild(a);if(b.compareDocumentPosition(a)!==4||document.compareDocumentPosition(text)!==20)throw 'reconnection';
var other=document.createElement('div'),again=document.createElement('div');
if((other.compareDocumentPosition(again)&33)!==33||other.compareDocumentPosition(other)!==0)throw 'separate detached roots';
document.title='position-tree-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "position-tree-ok");
    }

    #[test]
    fn node_position_uses_real_attr_owner_and_attribute_list_order() {
        let (doc, _, page, outcome) = start(r#"
var a=document.getElementById('a'),b=document.getElementById('b'),s=document.getElementById('s'),outer=document.getElementById('outer');
var title=a.getAttributeNode('title'),id=a.getAttributeNode('id'),other=b.getAttributeNode('title');
if(title.compareDocumentPosition(title)!==0||id.compareDocumentPosition(title)!==36||title.compareDocumentPosition(id)!==34)throw 'attribute list order';
if(a.compareDocumentPosition(title)!==20||title.compareDocumentPosition(a)!==10||outer.compareDocumentPosition(title)!==20)throw 'owner ancestry';
if(title.compareDocumentPosition(s)!==4||s.compareDocumentPosition(title)!==2||title.compareDocumentPosition(other)!==4)throw 'attribute tree precedence';
if(document.compareDocumentPosition(title)!==20||title.compareDocumentPosition(document)!==10)throw 'Document/Attr';
a.removeAttributeNode(title);var position=title.compareDocumentPosition(a),reverse=a.compareDocumentPosition(title);
if((position!==35&&position!==37)||(position^reverse)!==6||title.compareDocumentPosition(a)!==position)throw 'detached Attr';
a.setAttribute('title','fresh');if(a.getAttributeNode('title')===title||title.ownerElement!==null)throw 'stable detached identity';
document.title='position-attr-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "position-attr-ok");
    }

    #[test]
    fn node_position_brand_arity_and_work_errors_invoke_no_coercion_or_mutation() {
        let (mut doc, mut forms, mut page, outcome) = start(r#"
var a=document.getElementById('a'),method=a.compareDocumentPosition,calls=0;
for(var bad of [null,undefined,1,'node',Symbol(),{nodeType:1,toString:function(){calls++;return 'node';},valueOf:function(){calls++;return 1;}}]){
 var caught=false;try{method.call(a,bad);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'argument brand';
}
var caught=false;try{method.call(a);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'arity';
for(var bad of [window,{},null]){caught=false;try{method.call(bad,a);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'receiver brand';}
if(calls||method.call(a,a)!==0||document.compareDocumentPosition.call(a,a)!==0)throw 'coercion/borrow';
document.title='position-errors-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "position-errors-ok");
        page.begin_job();
        let a = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "a")).unwrap();
        let before = (doc.nodes.len(), doc.attr(a, "title").map(str::to_owned));
        let inert = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "inert")).unwrap();
        page.state.dirty = true;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(bridge.method(node_handle(inert), "compareDocumentPosition").is_none());
        assert!(bridge.call(DOCUMENT, "compareDocumentPosition", &[Value::Host(node_handle(inert))]).is_err());
        assert!(bridge.call(node_handle(bridge.doc.nodes.len()+1), "compareDocumentPosition", &[Value::Host(DOCUMENT)]).is_err());
        bridge.state.host_work = 0;
        assert!(bridge.call(DOCUMENT, "compareDocumentPosition", &[Value::Host(node_handle(a))]).is_err());
        assert_eq!((bridge.doc.nodes.len(), bridge.doc.attr(a, "title").map(str::to_owned)), before);
    }
}

impl Bridge<'_> {
    fn main_position_path(&mut self, mut node: usize, attribute: Option<usize>) -> Result<PositionPath, String> {
        self.state.rebuild(self.doc)?;
        let mut steps = Vec::new();
        for _ in 0..self.doc.nodes.len() {
            self.state.work(1)?;
            let parent = *self.state.tree.parents.get(node).ok_or("Invalid position node.")?;
            let Some(parent) = parent else {
                steps.reverse();
                return Ok(PositionPath { root: if node == 0 { DOCUMENT } else { node_handle(node) }, steps, attribute });
            };
            let children = &self.doc.nodes.get(parent).ok_or("Invalid position parent.")?.children;
            self.state.work(children.len())?;
            let ordinal = children.iter().position(|&child| child == node).ok_or("Position parent/child mismatch.")?;
            steps.try_reserve(1).map_err(|_| "Unable to allocate Node position path.")?;
            steps.push(ordinal);
            node = parent;
        }
        Err("Cyclic Node position ancestry.".into())
    }

    fn position_path(&mut self, handle: u32) -> Result<PositionPath, String> {
        if let Some(result) = self.frame_position_path(handle) { return result; }
        if let Some(id) = script_attr::index(handle) {
            let attr = self.state.attributes.get(id).ok_or("Invalid Attr position handle.")?;
            return match attr.owner {
                Some(owner) => {let node=node_index(owner).filter(|_|self.inert_node_type(owner).is_none()).ok_or("Inert Attr position traversal is unavailable.")?;self.main_position_path(node, Some(id))},
                None => Ok(PositionPath { root: handle, steps: Vec::new(), attribute: Some(id) }),
            };
        }
        let node = if handle == DOCUMENT { 0 } else { self.dom_node(handle)? };
        self.main_position_path(node, None)
    }

    pub(super) fn compare_document_position(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let Some(Value::Host(other)) = args.first() else { return Err("compareDocumentPosition requires a Node.".into()); };
        if !self.namespace_receiver(object) || !self.namespace_receiver(*other) {
            return Err("compareDocumentPosition requires genuine accessible Nodes.".into());
        }
        if object == *other { return Ok(Value::Number(0.0)); }
        let this = self.position_path(object)?;
        let other = self.position_path(*other)?;
        if this.root != other.root {
            // Stable opaque root identities define a reciprocal total order for
            // disconnected trees. No pointer, random seed or page data leaks.
            return Ok(Value::Number(if this.root < other.root { 37.0 } else { 35.0 }));
        }
        if this.steps == other.steps {
            return Ok(Value::Number(match (this.attribute, other.attribute) {
                (Some(this), Some(other)) => {
                    let owner = self.state.attributes[this].owner.and_then(node_index).ok_or("Missing main Attr owner.")?;
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { return Err("Invalid Attr owner.".into()); };
                    self.state.work(attrs.iter().fold(attrs.len(), |work, attr| work.saturating_add(attr.name.bytes().saturating_mul(2))))?;
                    let this_name = &self.state.attributes[this].name;
                    let other_name = &self.state.attributes[other].name;
                    let this_order = attrs.iter().position(|attr| attr.name.same_expanded_name(this_name)).ok_or("Attached Attr is missing from its owner.")?;
                    let other_order = attrs.iter().position(|attr| attr.name.same_expanded_name(other_name)).ok_or("Attached Attr is missing from its owner.")?;
                    if this_order < other_order { 36.0 } else { 34.0 }
                }
                (Some(_), None) => 10.0,
                (None, Some(_)) => 20.0,
                _ => return Err("Distinct Nodes share a physical position.".into()),
            }));
        }
        let value = if other.steps.starts_with(&this.steps) && this.attribute.is_none() {
            20.0
        } else if this.steps.starts_with(&other.steps) && other.attribute.is_none() {
            10.0
        } else if this.steps < other.steps { 4.0 } else { 2.0 };
        Ok(Value::Number(value))
    }
}
