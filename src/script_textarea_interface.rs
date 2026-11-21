//! Canonical textarea identity/value over the existing admitted native editor.
//! https://html.spec.whatwg.org/multipage/form-elements.html#htmltextareaelement
use super::*;
pub(super) fn accessor(method: &str) -> Option<bool> {
    match method {
        crate::javascript::TEXTAREA_VALUE_GET => Some(false),
        crate::javascript::TEXTAREA_VALUE_SET => Some(true),
        _ => None,
    }
}
impl Bridge<'_> {
    pub(super) fn textarea_value_receiver(&self, object: u32) -> bool {
        self.class_list_element_receiver(object)
            && node_index(object).is_some_and(|node| self.owned_native_textarea(node))
    }
    pub(super) fn textarea_value_member(&self, object: u32, property: &str) -> bool {
        property == "value" && self.textarea_value_receiver(object)
    }
    pub(super) fn validate_textarea_value(
        &self,
        object: u32,
    ) -> Result<(), (&'static str, String)> {
        if self.textarea_value_receiver(object) {
            Ok(())
        } else if self.inert_interface(object) == Some("HTMLTextAreaElement") {
            Err((
                "NotSupportedError",
                "Inert textarea values have no native editor state.".into(),
            ))
        } else {
            Err((
                "TypeError",
                "Textarea value requires a genuine admitted native textarea.".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Document, Forms, Page) {
        let mut doc=crate::html::parse_loaded_document("<html><body><form id=f><textarea id=area name=q maxlength=3 readonly>default\r\nline</textarea><input id=neighbor value=seed></form></body></html>",&||Ok(())).unwrap();
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::new();
        let location = crate::navigation::address(
            "https://textarea-interface.test/start",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        page.set_resource_base(&location);
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        run(&mut doc,&mut forms,&mut page,"var area=document.getElementById('area'),neighbor=document.getElementById('neighbor'),C=HTMLTextAreaElement,P=C.prototype,d=Object.getOwnPropertyDescriptor(P,'value'),get=d.get,set=d.set;");
        (doc, forms, page)
    }
    fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
        let result = page.console_eval(doc, forms, source);
        assert_eq!(result.errors, 0, "{source}\n{}", page.report());
    }
    #[test]
    fn textarea_interface_genuine_identity_descriptors_value_editor_and_ordinary_overrides() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
if(!(area instanceof C)||!(area instanceof HTMLElement)||area instanceof HTMLInputElement||area.constructor!==C||Object.getPrototypeOf(area)!==P||Object.getPrototypeOf(P)!==HTMLElement.prototype||Object.getPrototypeOf(C)!==HTMLElement||Object.prototype.toString.call(area)!=='[object HTMLTextAreaElement]')throw 'real textarea interface';
if(!d.enumerable||!d.configurable||d.get.name!=='get value'||d.set.name!=='set value'||get.length!==0||set.length!==1||Object.getOwnPropertyDescriptor(area,'value')!==undefined||Object.keys(area).indexOf('value')!==-1)throw 'IDL descriptors';
var count=0;for(var key in area){if(key==='value')count++;}if(count!==1)throw 'canonical enumerable value';
var original=area.defaultValue,calls=0;if(get.call(area)!=='default\nline')throw 'initial normalized value';
set.call(area,{toString:function(){calls++;return 'script\r\n😀';}});if(calls!==1||area.value!=='script\n😀'||area.defaultValue!==original||area.getAttribute('value')!==null||area.form!==document.getElementById('f'))throw 'actual native value/default distinction';
set.call(area,null);if(area.value!=='')throw 'LegacyNullToEmptyString';set.call(area);if(area.value!=='undefined')throw 'missing setter argument';
try{set.call(area,Symbol('value'));throw 'missing symbol failure';}catch(e){if(!(e instanceof TypeError))throw e;}if(area.value!=='undefined')throw 'symbol changed value';
Object.defineProperty(P,'value',{get:function(){return 'author';},set:function(value){this.setAttribute('data-probe',value);},enumerable:true,configurable:true});area.value='descriptor';
if(area.value!=='author'||area.getAttribute('data-probe')!=='descriptor'||get.call(area)!=='undefined')throw 'author prototype versus native slot';
Object.defineProperty(area,'value',{value:'own',writable:true,configurable:true});set.call(area,'native');if(area.value!=='own'||get.call(area)!=='native')throw 'own override versus saved setter';delete area.value;Object.defineProperty(P,'value',d);if(area.value!=='native')throw 'restored value';
var saved=C;delete window.HTMLTextAreaElement;if(!(area instanceof saved)||P.constructor!==saved)throw 'saved roots';
"#,
        );
        let node = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="area"))
            .unwrap();
        let control = &forms.controls[&node];
        assert_eq!(control.value.text(), "native");
        assert!(control.dirty_value);
        assert!(control.readonly);
    }
    #[test]
    fn textarea_interface_wrong_brand_inert_and_owner_guards_precede_conversion() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
var calls=0;for(var bad of [{},document,neighbor,Object.create(P)]){try{set.call(bad,{toString:function(){calls++;return 'bad';}});throw 'missing brand refusal';}catch(e){if(!(e instanceof TypeError))throw e;}}
if(calls||area.value!=='default\nline')throw 'receiver refusal converted or mutated';
for(var call of [function(){new C();},function(){C();}]){try{call();throw 'missing illegal constructor';}catch(e){if(!(e instanceof TypeError))throw e;}}
var owner=document.implementation.createHTMLDocument('inert'),inert=owner.createElement('textarea');if(!(inert instanceof C)||inert.ownerDocument!==owner)throw 'real inert identity';
try{get.call(inert);throw 'fake inert editor value';}catch(e){if(!(e instanceof DOMException)||e.name!=='NotSupportedError')throw e;}
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,read=w.Function('value','return value.value;');
if(w.HTMLTextAreaElement===C||area instanceof w.HTMLTextAreaElement||read(area)!==area.value)throw 'owner realm identity';
"#,
        );
        let before = forms.controls.len();
        let location = crate::navigation::address(
            "https://other-textarea.test/changed",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        let result = page.console_eval(&mut doc, &mut forms, "read(area);");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert!(page.report().contains("origin"));
        assert_eq!(forms.controls.len(), before);
    }
    #[test]
    fn textarea_interface_saved_setter_refusal_preserves_real_editor_and_recovers() {
        let (mut doc, mut forms, mut page) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "set.call(area,'kept value');",
        );
        let node = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="area"))
            .unwrap();
        {
            let e = &mut forms.controls.get_mut(&node).unwrap().value;
            e.cursor = 4;
            e.anchor = 1;
            e.offset = 2;
        }
        let capture = |forms: &Forms| {
            let c = &forms.controls[&node];
            (
                c.value.text(),
                c.value.cursor,
                c.value.anchor,
                c.value.offset,
                c.dirty_value,
                c.owner,
            )
        };
        let kept = capture(&forms);
        let retained = page.state.retained;
        let copies = page.state.limits.copy_bytes;
        page.state.limits.copy_bytes = 0;
        let result = page.console_eval(&mut doc, &mut forms, "set.call(area,'replacement');");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert_eq!(capture(&forms), kept);
        assert_eq!(page.state.retained, retained);
        page.state.limits.copy_bytes = copies;
        let result = page.console_eval(&mut doc, &mut forms, r"set.call(area,'\uD800');");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert_eq!(capture(&forms), kept);
        run(&mut doc,&mut forms,&mut page,"set.call(area,{toString:function(){set.call(area,'nested');return 'outer\\r\\nvalue';}});if(area.value!=='outer\\nvalue')throw 'reentrant native value setter';");
        assert_eq!(forms.controls[&node].value.text(), "outer\nvalue");
    }
}
