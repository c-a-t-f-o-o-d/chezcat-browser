//! Genuine sandbox attribute lists; unsupported sandboxed contexts stay refused.
use super::*;
// HTML defines the supported set as allowed values actually supported by the
// user agent. Native admission still refuses every sandboxed context, so none
// of the permission-relaxation flags is supported. Merely storing a token does
// not add it to this set. Add future entries only with real enforcement.
pub(super) const SUPPORTED_TOKENS:&[&str]=&[];
pub(super) fn accessor(method: &str) -> Option<bool> {
    match method {
        crate::javascript::IFRAME_SANDBOX_GET => Some(false),
        crate::javascript::IFRAME_SANDBOX_SET => Some(true),
        _ => None,
    }
}
impl Bridge<'_> {
    pub(super) fn iframe_sandbox_receiver(&self, object: u32) -> bool {
        self.class_list_element_receiver(object)
            && node_index(object).is_some_and(|node| self.iframe_srcdoc_property(node))
    }
    pub(super) fn iframe_sandbox_member(&self, object: u32, property: &str) -> bool {
        property == "sandbox" && self.iframe_sandbox_receiver(object)
    }
    pub(super) fn validate_iframe_sandbox(
        &self,
        object: u32,
    ) -> Result<(), (&'static str, String)> {
        if self.iframe_sandbox_receiver(object) {
            Ok(())
        } else if self.inert_interface(object) == Some("HTMLIFrameElement") {
            Err((
                "NotSupportedError",
                "Inert iframe sandbox lists are not implemented.".into(),
            ))
        } else {
            Err((
                "TypeError",
                "sandbox requires a genuine native HTML iframe.".into(),
            ))
        }
    }
    pub(super) fn iframe_sandbox_get(&mut self, object: u32) -> Result<Value, String> {
        self.validate_iframe_sandbox(object)
            .map_err(|(_, message)| message)?;
        self.node(object)?;
        self.state.work(1)?;
        Ok(Value::Host(script_class_list::sandbox_handle(object)))
    }
    pub(super) fn iframe_sandbox_set(&mut self, object: u32, value: Value) -> Result<(), String> {
        self.validate_iframe_sandbox(object)
            .map_err(|(_, message)| message)?;
        self.class_list_set(script_class_list::sandbox_handle(object), "value", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Document, Forms, Page) {
        let mut doc=crate::html::parse("<html><body><iframe id=parsed sandbox='allow-scripts allow-scripts allow-forms'></iframe><div id=other></div></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
        let location = crate::navigation::address(
            "https://sandbox-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        page.set_resource_base(&location);
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        (doc, forms, page)
    }
    fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
        let out = page.console_eval(doc, forms, source);
        assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    }
    #[test]
    fn iframe_sandbox_list_is_same_live_attribute_with_canonical_putforwards() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
var parsed=document.getElementById('parsed'),f=document.createElement('iframe'),list=f.sandbox,d=Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype,'sandbox'),get=d.get,set=d.set;
if(!d.enumerable||!d.configurable||get.name!=='get sandbox'||set.name!=='set sandbox'||get.length!==0||set.length!==1||get.call(f)!==list||list!==f.sandbox||list===f.classList||f.hasAttribute('sandbox')||list.length!==0||list.value!=='')throw 'genuine SameObject default';
if(parsed.sandbox.value!=='allow-scripts allow-scripts allow-forms'||parsed.sandbox.length!==2||parsed.sandbox[0]!=='allow-scripts'||parsed.sandbox.item(1)!=='allow-forms')throw 'real parsed attribute';
list.add('allow-scripts','allow-same-origin','allow-scripts');if(f.getAttribute('sandbox')!=='allow-scripts allow-same-origin'||list.length!==2||!list.contains('allow-scripts')||list.contains('ALLOW-SCRIPTS')||list.item(99)!==null||list[99]!==undefined)throw 'ordered real tokens';
f.setAttribute('sandbox','allow-forms allow-forms allow-popups');if(list.length!==2||list[1]!=='allow-popups'||list.toString()!=='allow-forms allow-forms allow-popups')throw 'live raw attribute';
var calls=0;f.sandbox={toString:function(){calls++;return 'allow-scripts allow-forms';}};if(calls!==1||f.sandbox!==list||list.value!=='allow-scripts allow-forms')throw 'genuine PutForwards';
set.call(f,'allow-popups');if(list.value!=='allow-popups')throw 'saved setter';list.remove('allow-popups');if(list.length!==0||!f.hasAttribute('sandbox')||f.getAttribute('sandbox')!=='')throw 'empty attribute presence';
var add=list.add;add.call(f.classList,'ordinary');if(f.className!=='ordinary'||list.length!==0)throw 'borrowed operation changed wrong attribute';
f.removeAttribute('sandbox');if(list.value!==''||list.length!==0||f.hasAttribute('sandbox'))throw 'live attribute removal';
delete HTMLIFrameElement.prototype.sandbox;if(f.sandbox!==undefined||get.call(f)!==list)throw 'hidden getter resurrection';Object.defineProperty(HTMLIFrameElement.prototype,'sandbox',d);
"#,
        );
    }
    #[test]
    fn iframe_sandbox_list_receiver_coercion_and_token_refusal_are_atomic() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
var f=document.createElement('iframe'),list=f.sandbox,d=Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype,'sandbox'),effects=0,other=document.getElementById('other');
var caught=false;try{d.set.call(other,{toString:function(){effects++;return 'bad';}});}catch(e){caught=e instanceof TypeError;}if(!caught||effects!==0)throw 'brand before coercion';
f.sandbox='allow-scripts';var raw=f.getAttribute('sandbox'),checks=0;
for(var call of [function(){list.add('allow-forms','');},function(){list.add('allow-forms','bad token');},function(){list.remove('');},function(){f.sandbox=Symbol('bad');}]){try{call();}catch(e){checks++;}if(f.getAttribute('sandbox')!==raw)throw 'refused mutation published';}if(checks!==4)throw 'required refusal';
var marker={},identity=false;try{f.sandbox={toString:function(){throw marker;}};}catch(e){identity=e===marker;}if(!identity||list.value!==raw)throw 'coercion exception';
f.sandbox=null;if(list.value!=='null')throw 'PutForwards DOMString null';
"#,
        );
    }
    #[test]
    fn iframe_sandbox_list_does_not_publish_sandboxed_context_or_mutate_live_frame_policy() {
        let (mut doc, mut forms, mut page) = fixture();
        run(&mut doc,&mut forms,&mut page,"var f=document.createElement('iframe');f.hidden=true;var list=f.sandbox;list.add('allow-scripts','allow-same-origin');");
        let count = page.state.blank_frames.len();
        let commits = page.state.dom_commits;
        let out = page.console_eval(&mut doc, &mut forms, "document.body.appendChild(f);");
        assert_eq!(out.errors, 1, "{}", page.report());
        assert!(page.report().contains("sandbox"));
        assert_eq!(page.state.blank_frames.len(), count);
        assert_eq!(page.state.dom_commits, commits);
        run(&mut doc,&mut forms,&mut page,"if(f.isConnected||f.parentNode!==null||f.contentWindow!==null||list.value!=='allow-scripts allow-same-origin')throw 'refused context publication';f.removeAttribute('sandbox');document.body.appendChild(f);var w=f.contentWindow;var savedDoc=w.document;");
        let node = (0..doc.nodes.len())
            .find(|&n| doc.attr(n, "hidden").is_some() && tag(&doc, n) == "iframe")
            .unwrap();
        let before = doc.attr(node, "sandbox").map(str::to_owned);
        let count = page.state.blank_frames.len();
        for source in [
            "list.add('allow-scripts');",
            "f.sandbox='allow-same-origin';",
            "list.value='';",
        ] {
            let out = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(out.errors, 1, "{source}\n{}", page.report());
            assert_eq!(doc.attr(node, "sandbox").map(str::to_owned), before);
            assert_eq!(page.state.blank_frames.len(), count);
        }
        run(&mut doc,&mut forms,&mut page,"if(f.contentWindow!==w||w.document!==savedDoc||f.hasAttribute('sandbox'))throw 'refusal changed existing frame';");
    }
    #[test]
    fn iframe_sandbox_list_native_copy_and_retained_refusal_preserve_attribute() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "var f=document.createElement('iframe');f.sandbox='allow-scripts';",
        );
        let node = doc.nodes.len() - 1;
        assert_eq!(tag(&doc, node), "iframe");
        let handle = script_class_list::sandbox_handle(node_handle(node));
        let retained = page.state.retained;
        let old_limit = page.state.retained_limit;
        let copies = page.state.copy_work;
        page.state.retained_limit = retained;
        assert!(Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state
        }
        .class_list_set(
            handle,
            "value",
            Value::String("allow-scripts allow-same-origin".into())
        )
        .is_err());
        assert_eq!(doc.attr(node, "sandbox"), Some("allow-scripts"));
        assert_eq!(page.state.retained, retained);
        page.state.retained_limit = old_limit;
        page.state.copy_work = 0;
        assert!(Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state
        }
        .class_list_get(handle, "value")
        .is_err());
        assert_eq!(doc.attr(node, "sandbox"), Some("allow-scripts"));
        page.state.copy_work = copies;
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "f.sandbox='allow-forms';if(f.sandbox.value!=='allow-forms')throw 'real recovery';",
        );
    }
    #[test]
    fn iframe_sandbox_saved_child_getters_toggle_and_unsupported_list_methods_keep_guards() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
var f=document.createElement('iframe'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,get=Object.getOwnPropertyDescriptor(w.HTMLIFrameElement.prototype,'sandbox').get;
if(get.call(f)!==f.sandbox||f.sandbox===f.classList)throw 'saved child accessor receiver';
if(f.sandbox.toggle('allow-scripts')!==true||f.sandbox.value!=='allow-scripts'||f.sandbox.supports('allow-scripts')||f.contentWindow!==null)throw 'actual detached toggle metadata';
if(f.sandbox.replace('allow-scripts','allow-forms')!==true||f.sandbox.value!=='allow-forms'||f.sandbox.supports('allow-forms')||f.contentWindow!==null)throw 'actual detached replacement metadata';
"#,
        );
        let before = page.state.dom_commits;
        for source in ["f.sandbox.forEach(function(){});"] {
            let out = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(out.errors, 1, "{}", page.report());
            assert_eq!(page.state.dom_commits, before);
        }
        page.set_location(
            &crate::navigation::address(
                "https://foreign-owned.test/source",
                &crate::navigation::Location::home(),
            )
            .unwrap(),
        );
        let out = page.console_eval(&mut doc, &mut forms, "get.call(f);");
        assert_eq!(out.errors, 1, "{}", page.report());
        assert!(
            page.report().contains("origin") || page.report().contains("Security"),
            "{}",
            page.report()
        );
    }
    #[test]
    fn iframe_sandbox_supports_uses_actual_capability_set_not_stored_tokens() {
        let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var f=document.createElement('iframe'),s=f.sandbox,changes=0;f.sandbox='allow-scripts allow-same-origin arbitrary';var before=f.getAttribute('sandbox');
if(typeof s.supports!=='function'||s.supports.length!==1||s.supports.name!=='supports')throw 'real query shape';
for(var token of ['allow-scripts','ALLOW-SCRIPTS','allow-same-origin','allow-forms','allow-popups','allow-presentation','arbitrary','','bad token'])if(s.supports(token)!==false)throw 'unsupported native flag advertised';
if(!s.contains('allow-scripts')||s.value!==before||f.contentWindow!==null||f.isConnected)throw 'stored token conflated with capability';
if(s.supports({toString:function(){changes++;return 'allow-scripts';}})!==false||changes!==1)throw 'DOMString conversion';
var caught=false;try{f.classList.supports({toString:function(){changes++;return 'ordinary';}});}catch(e){caught=e instanceof TypeError;}if(!caught||changes!==2)throw 'undefined supported set TypeError after conversion';
var method=f.classList.supports;if(method.call(s,'allow-scripts')!==false)throw 'actual borrowed receiver attribute';
var effects=0;try{method.call({}, {toString:function(){effects++;return 'x';}});}catch(e){if(!(e instanceof TypeError))throw e;}if(effects!==0)throw 'wrong brand converted token';
if(s.value!==before||f.getAttribute('sandbox')!==before)throw 'query changed metadata';
"#);
    }
    #[test]
    fn iframe_sandbox_supports_conversion_exceptions_preserve_native_state() {
        let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var f=document.createElement('iframe'),s=f.sandbox,marker={},caught=false;
try{s.supports({toString:function(){throw marker;}});}catch(e){caught=e===marker;}if(!caught||f.hasAttribute('sandbox'))throw 'conversion exception identity or mutation';
var rejected=false;try{s.supports(Symbol('bad'));}catch(e){rejected=e instanceof TypeError;}if(!rejected||s.value!==''||f.contentWindow!==null)throw 'symbol query mutated state';
"#);
    }

}
