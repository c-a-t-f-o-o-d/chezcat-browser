//! Original ARIAMixin string reflection and author-descriptor integration.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><body><chezcat-example id=target></chezcat-example><svg><rect id=icon width=20 height=20></rect></svg><input id=neighbor value=seed><form id=form><input name=ariaLabel id=named></form></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(
        page.console_eval(doc, forms, source).errors,
        0,
        "{}",
        page.report()
    );
}
#[test]
fn aria_reflection_real_element_descriptors_and_reactive_author_overrides_keep_native_slots() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('target'),desc=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel'),state='author',reads=0,writes=0;
if(!desc||!desc.enumerable||!desc.configurable||desc.get.name!=='get ariaLabel'||desc.set.name!=='set ariaLabel'||desc.get.length!==0||desc.set.length!==1||node.hasOwnProperty('ariaLabel')||Object.hasOwn(HTMLElement.prototype,'ariaLabel'))throw 'native inherited Element descriptor';
Object.defineProperty(node,'ariaLabel',{get:function(){reads++;return state;},set:function(value){writes++;state=value;},configurable:true,enumerable:true});node.ariaLabel='custom';if(node.ariaLabel!=='custom'||reads!==1||writes!==1||desc.get.call(node)!==null||node.hasAttribute('aria-label'))throw 'author state independent';
desc.set.call(node,'native');if(desc.get.call(node)!=='native'||node.getAttribute('aria-label')!=='native'||writes!==1)throw 'saved canonical setter';delete node.ariaLabel;if(node.ariaLabel!=='native'||Object.hasOwn(node,'ariaLabel'))throw 'restore native reflection';
Object.defineProperty(node,'ariaLabel',{value:17,writable:true,configurable:true});node.ariaLabel=23;if(node.ariaLabel!==23||desc.get.call(node)!=='native')throw 'ordinary data override';delete node.ariaLabel;
if(document.getElementById('neighbor').value!=='seed'||'ariaControlsElements' in Element.prototype||'ariaNotify' in Element.prototype)throw 'native neighbor and distinct unimplemented contracts';
"#,
    );
}
#[test]
fn aria_reflection_nullable_strings_coerce_once_and_preserve_namespace_and_attr_identity() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('target'),set=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel').set,calls=0;
node.ariaLabel='';var attr=node.getAttributeNode('aria-label');if(node.ariaLabel!==''||!node.hasAttribute('aria-label'))throw 'present empty string';node.ariaLabel={toString:function(){calls++;return 'mixed Case';}};if(calls!==1||attr!==node.getAttributeNode('aria-label')||attr.value!=='mixed Case'||node.ariaLabel!=='mixed Case')throw 'one conversion and live Attr';
node.ariaLabel=null;if(node.ariaLabel!==null||node.hasAttribute('aria-label')||attr.ownerElement!==null||attr.value!=='mixed Case')throw 'nullable removal and detached Attr';node.ariaLabel=undefined;if(node.ariaLabel!==null)throw 'undefined nullable';set.call(node);if(node.ariaLabel!==null)throw 'missing setter value';
node.ariaChecked=false;node.ariaValueNow=0;if(node.getAttribute('aria-checked')!=='false'||node.ariaChecked!=='false'||node.ariaValueNow!=='0')throw 'DOMString is not boolean reflection';
node.ariaLabel={toString:function(){return null;}};if(node.ariaLabel!=='null')throw 'nullable checks original value';
node.ariaLabel=null;node.setAttributeNS('urn:chezcat-test','q:aria-label','foreign');if(node.ariaLabel!==null)throw 'namespaced lookalike';node.ariaLabel='local';if(node.getAttributeNS('urn:chezcat-test','aria-label')!=='foreign'||node.getAttribute('aria-label')!=='local')throw 'real expanded names';
var caught=false;try{node.ariaLabel=Symbol('x');}catch(e){caught=e instanceof TypeError;}if(!caught||node.ariaLabel!=='local')throw 'symbol failure publishes nothing';
"#,
    );
}
#[test]
fn aria_reflection_mappings_svg_and_inert_owners_use_real_attributes_and_notifications() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('target');for(var pair of [['role','role'],['ariaAutoComplete','aria-autocomplete'],['ariaBrailleRoleDescription','aria-brailleroledescription'],['ariaColIndexText','aria-colindextext'],['ariaHasPopup','aria-haspopup'],['ariaKeyShortcuts','aria-keyshortcuts'],['ariaMultiLine','aria-multiline'],['ariaPosInSet','aria-posinset'],['ariaReadOnly','aria-readonly'],['ariaRoleDescription','aria-roledescription'],['ariaRowIndexText','aria-rowindextext'],['ariaValueMax','aria-valuemax'],['ariaValueText','aria-valuetext']]){var d=Object.getOwnPropertyDescriptor(Element.prototype,pair[0]);if(!d||!d.enumerable||!d.configurable)throw 'native string family descriptor';d.set.call(node,'raw');if(node.getAttribute(pair[1])!=='raw'||d.get.call(node)!=='raw')throw 'actual attribute correspondence';d.set.call(node,null);if(node.hasAttribute(pair[1])||d.get.call(node)!==null)throw 'actual removal';}
var svg=document.getElementById('icon'),get=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel').get;svg.ariaLabel='svg';svg.role='img';if(svg.getAttribute('aria-label')!=='svg'||get.call(svg)!=='svg'||svg.getAttribute('role')!=='img')throw 'shared Element SVG state';
var inert=document.implementation.createHTMLDocument('inert'),other=inert.createElement('div'),observer=new MutationObserver(function(){});observer.observe(other,{attributes:true,attributeOldValue:true});other.ariaLabel='first';var attr=other.getAttributeNode('aria-label');other.ariaLabel='second';other.ariaLabel=null;var records=observer.takeRecords();if(records.length!==3||records[0].oldValue!==null||records[1].oldValue!=='first'||records[2].oldValue!=='second'||attr.ownerElement!==null||other.ariaLabel!==null)throw 'inert reflected observation';other.ariaLabel=null;if(observer.takeRecords().length!==0)throw 'absent removal no mutation';
var inertSvg=inert.createElementNS('http://www.w3.org/2000/svg','svg');inertSvg.ariaLabel='inert svg';if(inertSvg.getAttribute('aria-label')!=='inert svg'||get.call(inertSvg)!=='inert svg')throw 'real inert SVG attribute';
"#,
    );
}
#[test]
fn aria_reflection_brands_form_named_lookup_and_native_disabled_state_stay_distinct() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var descriptor=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel'),calls=0;for(var receiver of [document,document.createTextNode('x'),document.createDocumentFragment(),{},Object.create(Element.prototype)]){var caught=false;try{descriptor.set.call(receiver,{toString:function(){calls++;return 'bad';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls!==0)throw 'Element brand before conversion';}
var form=document.getElementById('form'),named=document.getElementById('named');if(form.ariaLabel!==named)throw 'named form read';descriptor.set.call(form,'form native');if(form.ariaLabel!==named||descriptor.get.call(form)!=='form native')throw 'saved native form slots';form.ariaLabel='assigned native';if(form.ariaLabel!==named||form.getAttribute('aria-label')!=='assigned native')throw 'assignment ignores named lookup';
var input=document.getElementById('neighbor');input.ariaDisabled=true;if(input.disabled||input.hasAttribute('disabled')||input.getAttribute('aria-disabled')!=='true'||input.value!=='seed')throw 'ARIA does not fake native disabled';
"#,
    );
}
#[test]
fn aria_reflection_child_accessors_keep_owner_origin_checks_and_unsupported_write_order() {
    let (mut d, mut f, mut p) = fixture();
    let origin = crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/root").unwrap().0),
        path: None,
        fragment: String::new(),
        root: None,
    };
    p.set_location(&origin);
    p.set_resource_base(&origin);
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,root=document.getElementById('target'),childRoot=w.document.documentElement,calls=0;
var get=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel').get,set=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel').set,childGet=w.Object.getOwnPropertyDescriptor(w.Element.prototype,'ariaLabel').get;
root.ariaLabel='main';if(childGet===get||Object.getPrototypeOf(childGet)!==w.Function.prototype||childGet.call(root)!=='main'||get.call(childRoot)!==null)throw 'genuine owner accessors';
var caught=false;try{set.call(childRoot,{toString:function(){calls++;return 'bad';}});}catch(e){caught=e.name==='NotSupportedError';}if(!caught||calls!==0)throw 'child write refusal before conversion';
"#,
    );
    p.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://other.test/").unwrap().0),
        path: None,
        fragment: String::new(),
        root: None,
    });
    for denied in [
        "childGet.call(root);",
        "get.call(childRoot);",
        "set.call(childRoot,{toString:function(){calls++;return 'bad';}});",
    ] {
        assert_eq!(
            p.console_eval(&mut d, &mut f, denied).errors,
            1,
            "{denied}: {}",
            p.report()
        );
    }
    run(
        &mut d,
        &mut f,
        &mut p,
        "if(calls!==0||root.ariaLabel!=='main')throw 'denied access changed state';",
    );
}

#[test]
fn aria_reflection_small_adversarial_function_profile_refuses_intrinsic_bootstrap_and_keeps_main_slots(
) {
    let mut doc = crate::html::parse("<html><body><div id=target></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let origin = crate::navigation::Location {
        url: Some(
            crate::url::parse("https://example.test/aria-profile")
                .unwrap()
                .0,
        ),
        path: None,
        fragment: String::new(),
        root: None,
    };
    page.set_location(&origin);
    page.set_resource_base(&origin);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let result=page.console_eval(&mut doc,&mut forms,"var target=document.getElementById('target');target.ariaLabel='before';var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var rejectedConstructor=frame.contentWindow.Object;");
    assert_eq!(result.errors, 1, "{}", page.report());
    assert!(
        page.report().contains("Function limit reached (1024)"),
        "{}",
        page.report()
    );
    assert_eq!(page.console_eval(&mut doc,&mut forms,"if(typeof rejectedConstructor!=='undefined')throw 'partial intrinsic publication';target.ariaLabel='after';if(target.ariaLabel!=='after')throw 'native main slots after realm refusal';").errors,0,"{}",page.report());
}

#[test]
fn aria_reflection_detached_import_and_parked_promotion_preserve_native_and_author_state() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var descriptor=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel'),svg=document.getElementById('icon');svg.ariaLabel='native svg';Object.defineProperty(svg,'ariaLabel',{value:'author svg',writable:true,configurable:true});if(svg.ariaLabel!=='author svg'||descriptor.get.call(svg)!=='native svg')throw 'SVG author/native split';delete svg.ariaLabel;if(svg.ariaLabel!=='native svg')throw 'SVG native restore';
var detached=document.createElement('div');detached.ariaLabel='detached';if(detached.isConnected||detached.getAttribute('aria-label')!=='detached')throw 'real detached state';
var template=document.createElement('template');template.innerHTML='<div aria-label="parked"></div>';var parked=template.content.firstElementChild,attr=parked.getAttributeNode('aria-label'),oldOwner=parked.ownerDocument;
Object.defineProperty(parked,'ariaLabel',{get:function(){return descriptor.get.call(this);},set:function(v){descriptor.set.call(this,v);},configurable:true});parked.ariaLabel='before';if(attr.value!=='before'||parked.ownerDocument!==oldOwner)throw 'parked native source';document.body.appendChild(template.content);parked.ariaLabel='after';if(!parked.isConnected||parked.ownerDocument!==document||attr.ownerDocument!==document||attr!==parked.getAttributeNode('aria-label')||attr.value!=='after'||parked.ariaLabel!=='after')throw 'promoted native Attr and author accessor';
var inert=document.implementation.createHTMLDocument('source'),source=inert.createElement('div');source.ariaLabel='source';var imported=document.importNode(source,true),copy=imported.cloneNode(true);imported.ariaLabel='imported';copy.ariaLabel='copy';if(source.ariaLabel!=='source'||imported.getAttribute('aria-label')!=='imported'||copy.getAttribute('aria-label')!=='copy'||imported.ownerDocument!==document||copy.ownerDocument!==document)throw 'independent real imported and cloned sources';
"#,
    );
}
