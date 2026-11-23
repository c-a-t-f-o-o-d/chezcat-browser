//! HTMLMetaElement bindings read the same attribute list as getAttribute and Attr.
use super::*;
use crate::javascript::html_reflection_interface as interface;

impl Bridge<'_> {
    pub(super) fn meta_receiver(&self, object: u32) -> bool {
        if self.inert_node_type(object).is_some() {
            self.inert_interface(object) == Some("HTMLMetaElement")
        } else {
            self.attr_element_receiver(object) && node_index(object).is_some_and(|n|
                self.doc.nodes[n].namespace == crate::html::Namespace::Html && tag(self.doc, n) == "meta")
        }
    }

    pub(super) fn meta_member(&self, object: u32, name: &str) -> bool {
        interface::meta_attribute(name).is_some() && self.meta_receiver(object)
    }

    pub(super) fn validate_meta(&self, object: u32) -> Result<(), (&'static str, String)> {
        if !self.meta_receiver(object) { return Err(("TypeError", "Expected an HTMLMetaElement.".into())); }
        self.native_node_access(object)
    }

    fn meta_value(&self, object: u32, key: &str) -> Option<&DomString> {
        self.element_attribute_source(object)?.iter().find(|a| a.name.html_is(key)).map(|a| &a.value)
    }

    pub(super) fn meta_get(&mut self, object: u32, key: &str) -> Result<Value, String> {
        self.validate_meta(object).map_err(|(_, m)| m)?;
        let count = self.element_attribute_source(object).ok_or("Missing meta element attribute source.")?.len();
        self.state.work(count.saturating_mul(key.len() + 1))?;
        let Some(value) = self.meta_value(object, key) else { return Ok(Value::String(String::new())); };
        let bytes = value.logical_bytes();
        if bytes > self.state.limits.host.string_bytes { return Err("Meta attribute exceeds the host string allowance.".into()); }
        self.state.copies(bytes)?;
        Self::dom_string_copy(self.meta_value(object, key).unwrap())
    }

    pub(super) fn meta_set(&mut self, object: u32, key: &str, value: &Value) -> Result<(), String> {
        self.validate_meta(object).map_err(|(_, m)| m)?;
        let value = self.dom_text(value)?;
        self.state.copies(key.len())?;
        let name = crate::html::AttributeName::unqualified(script_clone_node::copy_text(key)?);
        if self.inert_node_type(object) == Some(1) {
            return self.write_inert_dom_attribute_ns(object, name, Some(value));
        }
        let node = self.node(object)?;
        self.public_dom_attribute(node, name, Some(value), script_attr::InputPreparation::Pending)
    }

    pub(super) fn meta_content_attribute(&self, node: usize, name: &crate::html::AttributeName) -> bool {
        self.doc.nodes[node].namespace == crate::html::Namespace::Html && tag(self.doc,node) == "meta"
            && interface::META.iter().any(|&(_, attr, _, _)| name.html_is(attr))
    }

    pub(super) fn check_meta_change(&self, node: usize, name: &crate::html::AttributeName, value: Option<&DomString>) -> Result<(), String> {
        if !*self.state.dom.connected.get(node).ok_or("Missing metadata connection state.")? { return Ok(()); }
        // Parser-applied document policy is an immutable snapshot. Connected
        // pragma changes require their own document-level processing.
        if name.html_is("http-equiv") || name.html_is("content") && self.doc.dom_attr(node,"http-equiv").is_some() {
            return Err("Changing a connected metadata pragma requires document processing.".into());
        }
        let active = |s: &DomString| ["referrer", "viewport", "color-scheme", "theme-color"]
            .iter().any(|name| s.as_str().eq_ignore_ascii_case(name));
        if (name.html_is("name") || name.html_is("content") || name.html_is("media"))
            && (self.doc.dom_attr(node,"name").is_some_and(active) || name.html_is("name") && value.is_some_and(active)) {
            return Err("Changing active connected document metadata requires document processing.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Document, Forms, Page) {
        let mut doc = crate::html::parse("<!doctype html><html><head><meta id=m name=timeout content=long><meta id=p http-equiv=content-security-policy content=\"img-src 'none'\"></head><body></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
        page.set_location(&crate::navigation::address("https://meta.test/", &crate::navigation::Location::home()).unwrap());
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        (doc, forms, page)
    }

    #[test]
    fn html_meta_element_prototype_attributes_and_current_owner_are_real() {
        let (mut doc, mut forms, mut page) = fixture();
        let result = page.console_eval(&mut doc, &mut forms, r#"
var m=document.getElementsByTagName('meta')[0], proto=HTMLMetaElement.prototype;
if(Object.getPrototypeOf(m)!==proto||Object.getPrototypeOf(proto)!==HTMLElement.prototype||m.name!=='timeout'||m.content!=='long'||m.media!=='')throw 'loaded native metadata';
var seen=0, getter=Object.getOwnPropertyDescriptor(proto,'name'), value={toString:function(){seen++;return 'changed';}};
var other=document.createElement('div'),bad=false;try{getter.set.call(other,value);}catch(e){bad=e instanceof TypeError;}
if(!bad||seen)throw 'receiver before conversion';getter.set.call(m,value);
if(m.getAttribute('name')!=='changed'||m.name!=='changed'||seen!==1)throw 'actual attribute write';
var old=m.getAttributeNode('content'), observer=new MutationObserver(function(){});observer.observe(m,{attributes:true,attributeOldValue:true});
m.content='\ud800';var records=observer.takeRecords();
if(records.length!==1||records[0].oldValue!=='long'||records[0].target!==m||old.value!==m.content||m.content.charCodeAt(0)!==0xd800)throw 'attribute identity and raw units';
var xml=document.implementation.createDocument(null,'Root'), detached=document.createElement('meta'), inert=xml.createElementNS('http://www.w3.org/1999/xhtml','meta');
for(var x of [detached,inert]){x.httpEquiv='x-test';x.media='screen';x.scheme=null;x.name='example';x.content='a\udfff';if(x.httpEquiv!==x.getAttribute('http-equiv')||x.scheme!=='null'||x.getAttribute('media')!=='screen'||x.content!==x.getAttribute('content')||x.cloneNode(false).name!=='example')throw 'factory native reflectors';}
var foreign=xml.createElementNS('urn:outside','meta');if(foreign instanceof HTMLMetaElement)throw 'foreign lookalike';
Object.defineProperty(proto,'name',{configurable:true,value:'override'});if(m.name!=='override'||getter.get.call(m)!=='changed')throw 'descriptor precedence';Object.defineProperty(proto,'name',getter);
"#);
        assert_eq!(result.errors, 0, "{}", page.report());
    }

    #[test]
    fn html_meta_element_saved_child_caller_checks_origin_before_conversion() {
        let (mut doc, mut forms, mut page) = fixture();
        let first = page.console_eval(&mut doc, &mut forms, r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);
var child=f.contentWindow,m=document.getElementById('m'),reads=0;
var put=child.Function('node','value',"Object.getOwnPropertyDescriptor(HTMLMetaElement.prototype,'name').set.call(node,value);");
var value={toString:function(){reads++;return 'sample';}};
put(m,value);if(reads!==1||m.name!=='sample')throw 'same origin metadata';
"#);
        assert_eq!(first.errors, 0, "{}", page.report());
        page.set_location(&crate::navigation::address("https://other.test/", &crate::navigation::Location::home()).unwrap());
        let denied = page.console_eval(&mut doc, &mut forms, "put(m,value);");
        assert_eq!(denied.errors, 1, "{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(page.console_eval(&mut doc, &mut forms, "if(reads!==1||m.name!=='sample')throw 'denied conversion';").errors, 0);
    }

    #[test]
    fn html_meta_element_connected_pragma_and_size_refusals_leave_attributes_intact() {
        let (mut doc, mut forms, mut page) = fixture();
        let result = page.console_eval(&mut doc, &mut forms, "var p=document.getElementById('p'),saved=p.getAttributeNode('content'),m=document.getElementById('m');");
        assert_eq!(result.errors, 0, "{}", page.report());
        for operation in ["p.content='changed';", "p.setAttribute('http-equiv','changed');", "saved.value='changed';"] {
            let result = page.console_eval(&mut doc, &mut forms, operation);
            assert_eq!(result.errors, 1, "{}", page.report());
            assert!(page.report().contains("connected metadata pragma"), "{}", page.report());
        }
        let result = page.console_eval(&mut doc, &mut forms, "if(p.httpEquiv!=='content-security-policy'||p.content!==\"img-src 'none'\"||saved.value!==p.content)throw 'partial policy change';m.content='safe';if(m.content!=='safe')throw 'recovery';");
        assert_eq!(result.errors, 0, "{}", page.report());
        let n = (0..doc.nodes.len()).find(|&n| doc.attr(n,"id")==Some("m")).unwrap();
        let mut bridge = Bridge { doc:&mut doc, forms:&mut forms, state:&mut page.state };
        let limit = bridge.state.limits.host.string_bytes;
        bridge.state.limits.host.string_bytes = 3;
        assert!(bridge.meta_get(node_handle(n),"content").unwrap_err().contains("allowance"));
        bridge.state.limits.host.string_bytes = limit;
        assert_eq!(bridge.meta_get(node_handle(n),"content").unwrap().to_text(), "safe");
    }
}
