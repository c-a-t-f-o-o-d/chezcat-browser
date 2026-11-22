//! Real Node roots over the supported ordinary DOM trees.
//!
//! DOM: https://dom.spec.whatwg.org/#dom-node-getrootnode
//! https://dom.spec.whatwg.org/#concept-shadow-including-root
//! Options must pass through Host::dictionary_argument(0, &["composed"]).
//! The VM performs the observable property Get before this module walks the
//! current tree. No options getter or arbitrary runtime object is read here.
//! Current main/blank-child trees contain no ShadowRoot. Both composed values
//! therefore have the same ordinary root; this does not add a shadow DOM API.
use super::*;

pub(super) const ROOT_METHOD: &str = "\0Node.getRootNode";

impl Bridge<'_> {
    pub(super) fn node_tree_method_member(&self, object: u32, name: &str) -> bool {
        let member = match name {
            "getRootNode" => self.get_root_receiver(object),
            "substringData"=>self.substring_data_receiver(object),
            "isDefaultNamespace"|"lookupNamespaceURI"|"lookupPrefix"=>self.namespace_receiver(object),
            "cloneNode" => self.root_node_receiver(object),
            // Prototype selection depends on the genuine Node interface. The
            // VM guards property access from its current realm, and the call
            // rechecks the incumbent before accessing either document.
            "contains" => self.root_node_receiver(object) || self.frame_contains_node(object).is_some(),
            "replaceChild" if self.inert_replace_receiver(object) => true,
            "appendChild" if self.template_fragment_receiver(object) => true,
            "appendChild" | "insertBefore" | "removeChild" if self.inert_mutation_receiver(object) => true,
            _ if dom_tree::node_mutation_method(name) => self.node_mutation_receiver(object),
            _ => return false,
        };
        member && !self.form_named_control(object, name)
    }

    pub(super) fn node_mutation_receiver(&self, object: u32) -> bool {
        node_index(object).is_some_and(|node| self.dom_accessible(node))||self.inert_node_type(object)==Some(10)||self.attr_receiver(object)
    }

    pub(super) fn root_node_receiver(&self, object: u32) -> bool {
        // Same genuine Node/access boundary as existing namespace/position
        // operations: main Document, retained arena Nodes, Attr and supported
        // same-origin blank-child Nodes. Never accept a Node-shaped JS object.
        self.namespace_receiver(object)
    }

    pub(super) fn get_root_receiver(&self, object: u32) -> bool {
        self.root_node_receiver(object) || self.inert_node_type(object).is_some()
    }

    /// Pure validation for Host::validate_call, before dictionary member Get.
    pub(super) fn validate_root_node_options(
        &self,
        object: u32,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if !self.get_root_receiver(object) {
            return Err(("TypeError", "getRootNode requires a genuine accessible Node.".into()));
        }
        if args.first().is_some_and(|value| !matches!(value,
            Value::Undefined | Value::Null | Value::Object(_) | Value::Function(_) | Value::Host(_))) {
            return Err(("TypeError", "getRootNode options must be a dictionary or nullish.".into()));
        }
        Ok(())
    }

    pub(super) fn get_root_node(
        &mut self,
        object: u32,
        args: &[Value],
        fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        self.validate_root_node_options(object, args).map_err(|(_, message)| message)?;
        let dictionary = matches!(args.first(), Some(Value::Object(_) | Value::Function(_) | Value::Host(_)));
        let _composed = match fields {
            Some([("composed", value)]) if dictionary => value.truthy(),
            None if !dictionary => false,
            // An object must not silently skip its getter merely because all
            // currently supported roots are ordinary roots. Direct host callers
            // also must supply the result of the VM dictionary conversion.
            _ => return Err("getRootNode requires the runtime GetRootNodeOptions conversion.".into()),
        };

        // The root of an Attr is itself, even while it has an ownerElement.
        // compareDocumentPosition's special Attr-owner ordering is different.
        if self.attr_receiver(object) {
            self.state.work(1)?;
            return Ok(Value::Host(object));
        }
        if let Some(result) = self.frame_position_path(object) {
            // Existing child context, origin checks, bounds and work charging
            // remain authoritative. A child Document is not the outer iframe.
            return result.map(|path| Value::Host(path.root));
        }
        if object == DOCUMENT {
            self.state.work(1)?;
            if self.doc.nodes.is_empty() {
                return Err("Main Document root is unavailable.".into());
            }
            return Ok(Value::Host(DOCUMENT));
        }

        if self.inert_node_type(object).is_some() {
            return self.inert_root(object);
        }

        // Rebuild after dictionary conversion: its getter may have moved the
        // node. Detached ordinary subtrees retain their real physical parents.
        let mut node = self.dom_node(object)?;
        for _ in 0..self.doc.nodes.len() {
            self.state.work(1)?;
            let parent = *self.state.tree.parents.get(node).ok_or("Invalid Node root index.")?;
            let Some(parent) = parent else {
                return Ok(Value::Host(if node == 0 { DOCUMENT } else { node_handle(node) }));
            };
            if node == 0 {
                return Err("Main Document cannot have a parent.".into());
            }
            let children = &self.doc.nodes.get(parent).ok_or("Invalid Node root parent.")?.children;
            self.state.work(children.len())?;
            if children.iter().filter(|&&child| child == node).count() != 1 {
                return Err("Node root parent/child identity mismatch.".into());
            }
            node = parent;
        }
        Err("Cyclic Node root ancestry.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_with_profile(source: &str,browser:bool) -> (Document, Forms, Page) {
        let mut doc = crate::html::parse("<html><body><div id=outer><p id=a>text<span id=child>child</span></p></div></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = if browser {Page::new()}else{Page::for_test()};
        page.set_location(&crate::navigation::Location { url: Some(crate::url::parse("https://example.test/root").unwrap().0), path: None, root: None, fragment: String::new() });
        let outcome = page.start(&mut doc, &mut forms, &[Script { node: 0, label: "Original Node root fixture".into(), source: source.into() }]);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        (doc, forms, page)
    }

    fn fixture(source:&str)->(Document,Forms,Page){fixture_with_profile(source,false)}

    fn element(doc: &Document, id: &str) -> usize {
        doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(value), .. } if value == id)).unwrap()
    }

    fn root(bridge: &mut Bridge<'_>, object: u32, composed: bool) -> Value {
        bridge.get_root_node(object, &[Value::Object(0)], Some(&[("composed", Value::Bool(composed))])).unwrap()
    }

    fn assert_host(value: Value, expected: u32) {
        assert!(matches!(value, Value::Host(actual) if actual == expected));
    }

    #[test]
    fn node_root_reads_real_document_element_text_and_created_comment_topology() {
        let (mut doc, mut forms, mut page) = fixture("var c=document.createComment('original');document.getElementById('a').appendChild(c);");
        let a = element(&doc, "a");
        let child = element(&doc, "child");
        let text = doc.nodes[a].children[0];
        let comment = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Comment(value) if value == "original")).unwrap();
        let commits = page.state.dom_commits;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for handle in [DOCUMENT, node_handle(a), node_handle(child), node_handle(text), node_handle(comment)] {
            assert_host(root(&mut bridge, handle, false), DOCUMENT);
            assert_host(root(&mut bridge, handle, true), DOCUMENT);
        }
        assert_eq!(bridge.state.dom_commits, commits);
    }

    #[test]
    fn node_root_tracks_detached_subtree_and_reconnection_without_owner_document_substitution() {
        let (mut doc, mut forms, mut page) = fixture("var saved=document.getElementById('a');document.getElementById('outer').removeChild(saved);");
        let a = element(&doc, "a");
        let child = element(&doc, "child");
        let text = doc.nodes[a].children[0];
        {
            let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
            for handle in [node_handle(a), node_handle(child), node_handle(text)] {
                assert_host(root(&mut bridge, handle, false), node_handle(a));
                assert_host(root(&mut bridge, handle, true), node_handle(a));
            }
        }
        let outcome = page.console_eval(&mut doc, &mut forms, "document.body.appendChild(saved);");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert_host(root(&mut bridge, node_handle(text), true), DOCUMENT);
    }

    #[test]
    fn node_root_new_detached_nodes_remain_their_own_roots() {
        let (mut doc, mut forms, mut page) = fixture("var n=document.createElement('section');n.id='fresh';var t=document.createTextNode('fresh text');var c=document.createComment('fresh comment');");
        let created: Vec<_> = page.state.dom.created.iter().collect();
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for node in created {
            assert_host(root(&mut bridge, node_handle(node), false), node_handle(node));
        }
    }

    #[test]
    fn node_root_dictionary_gate_rejects_primitives_and_unconverted_objects() {
        let (mut doc, mut forms, mut page) = fixture("");
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for value in [Value::Bool(true), Value::Number(1.0), Value::String("options".into())] {
            assert_eq!(bridge.validate_root_node_options(DOCUMENT, &[value]).unwrap_err().0, "TypeError");
        }
        assert_eq!(bridge.validate_root_node_options(WINDOW, &[]).unwrap_err().0, "TypeError");
        assert!(bridge.get_root_node(DOCUMENT, &[Value::Object(0)], None).is_err());
        for args in [vec![], vec![Value::Null], vec![Value::Undefined]] {
            assert_host(bridge.get_root_node(DOCUMENT, &args, None).unwrap(), DOCUMENT);
        }
        bridge.state.host_work = 0;
        assert_eq!(bridge.validate_root_node_options(WINDOW, &[]).unwrap_err().0, "TypeError");
        assert!(bridge.get_root_node(DOCUMENT, &[], None).is_err());
    }

    #[test]
    fn node_root_invalid_handles_edges_cycles_and_work_refuse_without_dom_commit() {
        let (mut doc, mut forms, mut page) = fixture("");
        page.state.rebuild(&doc).unwrap();
        let a = element(&doc, "a");
        let child = element(&doc, "child");
        let commits = page.state.dom_commits;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(bridge.get_root_node(node_handle(bridge.doc.nodes.len() + 4), &[], None).is_err());
        let old_parent = bridge.state.tree.parents[child];
        bridge.state.tree.parents[child] = Some(bridge.doc.nodes.len() + 2);
        assert!(bridge.get_root_node(node_handle(child), &[], None).is_err());
        bridge.state.tree.parents[child] = Some(child);
        assert!(bridge.get_root_node(node_handle(child), &[], None).is_err());
        bridge.state.tree.parents[child] = old_parent;
        // A consistent cyclic pair must hit the bounded ancestor walk, rather
        // than recurse or allocate an unbounded visited set.
        let a_parent = bridge.state.tree.parents[a];
        let old_children = bridge.doc.nodes[child].children.clone();
        bridge.state.tree.parents[a] = Some(child);
        bridge.doc.nodes[child].children.push(a);
        assert!(bridge.get_root_node(node_handle(child), &[], None).is_err());
        bridge.state.tree.parents[a] = a_parent;
        bridge.doc.nodes[child].children = old_children;
        assert_host(root(&mut bridge, node_handle(child), false), DOCUMENT);
        bridge.state.host_work = 0;
        assert!(bridge.get_root_node(node_handle(child), &[], None).is_err());
        assert_eq!(bridge.state.dom_commits, commits);
    }

    #[test]
    fn node_root_vm_has_genuine_prototype_descriptors_borrowing_and_author_overrides() {
        let (_, _, _) = fixture(r#"
var P=Node.prototype,D=Object.getOwnPropertyDescriptor(P,'getRootNode'),method=D.value,a=document.getElementById('a'),text=a.firstChild,comment=document.createComment('c'),attr=a.getAttributeNode('id');
if(typeof method!=='function'||method.length!==0||method.name!=='getRootNode'||!D.enumerable||!D.writable||!D.configurable||D.get!==undefined||Object.getPrototypeOf(method)!==Function.prototype)throw 'genuine method descriptor';
if(document.getRootNode!==method||a.getRootNode!==method||text.getRootNode!==method||comment.getRootNode!==method||attr.getRootNode!==method)throw 'one actual inherited method';
if(method.call(document)!==document||method.call(a)!==document||method.call(text)!==document||method.call(comment)!==comment||method.call(attr)!==attr||method.call(attr,{composed:true})!==attr)throw 'real Node roots and Attr self';
if(Object.hasOwn(document,'getRootNode')||Object.hasOwn(a,'getRootNode')||Object.hasOwn(text,'getRootNode')||Object.hasOwn(attr,'getRootNode'))throw 'method is inherited';
var calls=0;P.getRootNode=function(){calls++;return method.call(this);};if(a.getRootNode()!==document||calls!==1)throw 'real prototype override';
delete P.getRootNode;if('getRootNode' in document||'getRootNode' in a||'getRootNode' in text||'getRootNode' in attr||typeof a.getRootNode!=='undefined')throw 'no ghost host method after deletion';
Object.defineProperty(P,'getRootNode',D);a.getRootNode=function(){return 'own';};if(a.getRootNode()!=='own'||method.call(a)!==document)throw 'ordinary own shadow';delete a.getRootNode;
if(a.getRootNode!==method||!delete document.getRootNode||document.getRootNode!==method)throw 'deleting absent own method preserves prototype';
var caught=false;try{new method();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'method is not a constructor';
"#);
    }

    #[test]
    fn node_root_vm_dictionary_get_runs_once_before_live_root_walk_without_primitive_coercion() {
        let (_, _, _) = fixture(r#"
var a=document.getElementById('a'),outer=document.getElementById('outer'),method=Node.prototype.getRootNode,hits=0,coercions=0,nested=null;
var truth={valueOf:function(){coercions++;throw 'no ToPrimitive';},toString:function(){coercions++;throw 'no ToString';}};
var options={get composed(){hits++;nested=method.call(a);outer.removeChild(a);return truth;}};
if(method.call(a,options)!==a||nested!==document||hits!==1||coercions!==0||a.ownerDocument!==document)throw 'getter once before current detached root';
var reattach={get composed(){hits++;outer.appendChild(a);return 0;}};
if(method.call(a.firstChild,reattach)!==document||hits!==2||coercions!==0)throw 'getter live reconnection and false ToBoolean';
if(method.call(a)!==document||method.call(a,null)!==document||method.call(a,undefined)!==document||method.call(a,{composed:undefined})!==document)throw 'default false dictionary';
if(method.call(a,{composed:''})!==document||method.call(a,{composed:1})!==document||method.call(a,{composed:Symbol()})!==document)throw 'ordinary ToBoolean values';
"#);
    }

    #[test]
    fn node_root_vm_brand_options_and_abrupt_getter_precede_walk_and_invoke_no_unrelated_getters() {
        let (_, _, _) = fixture(r#"
var method=Node.prototype.getRootNode,a=document.getElementById('a'),hits=0,coercions=0,marker={},options={get composed(){hits++;throw marker;},get unrelated(){throw 'unrelated member';}};
var caught=0;
for(var receiver of [null,undefined,{},window,Node.prototype,document.childNodes,a.style]){try{method.call(receiver,options);}catch(e){if(e instanceof TypeError)caught++;else throw e;}}
if(caught!==7||hits)throw 'brand before dictionary Get';
var abrupt=false;try{method.call(a,options);}catch(e){abrupt=e===marker;}if(!abrupt||hits!==1||a.parentNode!==document.getElementById('outer'))throw 'same abrupt value without mutation';
caught=0;for(var value of [true,false,0,1,'',Symbol()]){try{method.call(a,value);}catch(e){if(e instanceof TypeError)caught++;else throw e;}}if(caught!==6)throw 'primitive dictionary TypeErrors';
var functionOptions=function(){};Object.defineProperty(functionOptions,'composed',{get:function(){hits++;return false;}});
if(method.call(a,functionOptions)!==document||hits!==2)throw 'callable dictionary ordinary Get';
var good={get composed(){hits++;return true;},get unrelated(){throw 'unrelated member';}},ignored={get composed(){throw 'ignored extra argument';},valueOf:function(){coercions++;throw 'ignored';}};
if(method.call(a,good,ignored)!==document||hits!==3||coercions)throw 'only composed read and extras ignored';
"#);
    }

    #[test]
    fn node_root_vm_child_owners_are_real_and_cross_origin_refusal_precedes_options_get() {
        let (mut doc, mut forms, mut page) = fixture_with_profile(r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildFunction=w.Function,childDoc=frame.contentDocument,childBody=childDoc.body;
var root=Node.prototype.getRootNode,childRoot=ChildFunction('return Node.prototype.getRootNode;')(),hits=0;
if(childRoot===root||Object.getPrototypeOf(childRoot)!==ChildFunction.prototype||childRoot.length!==0||childRoot.name!=='getRootNode')throw 'actual child function owner';
if(childDoc.getRootNode!==childRoot)throw 'actual inherited child method';
if(childDoc.getRootNode()!==childDoc)throw 'child Document is its own root';
if(childRoot.call(document)!==document||root.call(childDoc)!==childDoc||root.call(childBody,{composed:true})!==childDoc)throw 'same-origin borrowed roots do not cross frame';
"#,true);
        let commits = page.state.dom_commits;
        page.set_location(&crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, root: None, fragment: String::new() });
        for source in [
            "childRoot.call(document,{get composed(){hits++;return false;}});",
            "root.call(childDoc,{get composed(){hits++;return true;}});",
        ] {
            let outcome = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(outcome.errors, 1, "{source}: {}", page.report());
            assert_eq!(page.state.dom_commits, commits);
        }
        let outcome = page.console_eval(&mut doc, &mut forms, "if(hits!==0||root.call(document)!==document)throw 'owner refusal leaked dictionary Get or main root';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        page.set_location(&crate::navigation::Location { url: Some(crate::url::parse("https://example.test/root").unwrap().0), path: None, root: None, fragment: String::new() });
        let outcome = page.console_eval(&mut doc, &mut forms, "if(childRoot.call(document)!==document||root.call(childBody)!==childDoc)throw 'origin recovery';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
}
