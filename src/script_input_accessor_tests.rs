//! Original canonical INPUT accessors and their existing native mode boundaries.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc, forms, source);
    assert_eq!(result.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &forms.controls[&node];
    (
        c.value.text(),
        c.value.cursor,
        c.value.anchor,
        c.value.offset,
        c.dirty_value,
        c.owner,
    )
}

fn fixture() -> (Document, Forms, Page) {
    fixture_with_page(Page::for_test())
}
fn fixture_with_page(mut page:Page)->(Document,Forms,Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Original input interface</title><body><form id=f><input id=neighbor name=keep value=seed><input id=text name=q value=default><input id=box type=checkbox name=choice value=yes checked><input id=bare type=checkbox><input id=r1 type=radio name=pick value=one checked><input id=r2 type=radio name=pick value=two><input id=hidden type=hidden name=h value=secret><input id=submit type=submit value=Send><input id=reset type=reset value=Reset><input id=button type=button value=Button><input id=image type=image value=Image><input id=file type=file><textarea id=area>area</textarea><button id=apply type=button>Apply</button><div id=destination></div><span id=out>quiet</span></form></body>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let source = location("https://input-interface.test/start");
    page.set_location(&source);
    page.set_resource_base(&source);
    let result = page.start(
        &mut doc,
        &mut forms,
        &[Script {
            node: 0,
            label: "Original INPUT accessor controls".into(),
            source: r#"
            var text=document.getElementById('text'),box=document.getElementById('box');
            var neighbor=document.getElementById('neighbor'),form=document.getElementById('f');
            var out=document.getElementById('out'),apply=document.getElementById('apply');
            neighbor.value='edited neighbor';
            if(text.value!=='default'||text.defaultValue!=='default'||box.value!=='yes'||
               !box.checked||text.form!==form||text.ownerDocument!==document)throw 'native prefix';
            var proto=HTMLInputElement.prototype;
            var valueDescriptor=Object.getOwnPropertyDescriptor(proto,'value');
            var checkedDescriptor=Object.getOwnPropertyDescriptor(proto,'checked');
            if(!valueDescriptor||!checkedDescriptor)throw 'canonical INPUT descriptors missing';
            var getValue=valueDescriptor.get,setValue=valueDescriptor.set;
            var getChecked=checkedDescriptor.get,setChecked=checkedDescriptor.set;
        "#
            .into(),
        }],
    );
    assert_eq!(result.errors, 0, "{}", page.report());
    let c = &mut forms.controls.get_mut(&id(&doc, "neighbor")).unwrap().value;
    c.cursor = 2;
    c.anchor = 5;
    c.offset = 1;
    (doc, forms, page)
}

#[test]
fn input_accessor_descriptors_are_canonical_and_reuse_existing_supported_modes() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        for(var pair of [['value',valueDescriptor],['checked',checkedDescriptor]]){
            var name=pair[0],d=pair[1];
            if(!d.enumerable||!d.configurable||'value' in d||'writable' in d||
               typeof d.get!=='function'||typeof d.set!=='function'||d.get===d.set||
               d.get.name!=='get '+name||d.set.name!=='set '+name||
               d.get.length!==0||d.set.length!==1||
               Object.getOwnPropertyDescriptor(d.get,'prototype')!==undefined)
                throw 'IDL accessor shape';
        }
        if(Object.getPrototypeOf(text)!==proto||text.constructor!==HTMLInputElement||
           Object.getOwnPropertyDescriptor(text,'value')!==undefined||
           Object.getOwnPropertyDescriptor(box,'checked')!==undefined||
           !('value' in text)||!('checked' in box))throw 'inherited native attributes';
        var inherited=[];
        for(var key in text){if(key==='value'||key==='checked')inherited.push(key);}
        if(inherited.join(',')!=='value,checked'||Object.keys(text).indexOf('value')!==-1||
           Object.keys(text).indexOf('checked')!==-1)throw 'enumerable inherited accessor keys';
        if(getValue.call(text)!=='default'||getChecked.call(box)!==true)throw 'saved getters';
        for(var name of ['hidden','submit','reset','button','image','r1']){
            var input=document.getElementById(name),expected=input.value;
            if(input.constructor!==HTMLInputElement||getValue.call(input)!==expected)
                throw 'genuine native mode identity';
            if(setValue.call(input,'changed-'+name)!==undefined||input.value!=='changed-'+name||
               input.getAttribute('value')!=='changed-'+name)throw 'native default/value mode';
        }
        setChecked.call(document.getElementById('r2'),true);
        if(document.getElementById('r1').checked||!document.getElementById('r2').checked)
            throw 'native radio group';
        setValue.call(text,'edited\r\ntext');
        if(text.value!=='editedtext'||text.defaultValue!=='default'||
           text.getAttribute('value')!=='default')throw 'native text dirty/default distinction';
    "#,
    );
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
}

#[test]
fn input_accessor_value_converts_once_and_checked_uses_boolean_without_author_hooks() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var trace='',marker={},conversion={
            toString:function(){trace+='S';return 'a\r\nb';},
            valueOf:function(){trace+='N';throw 'wrong hint';}
        };
        setValue.call(text,conversion);
        setValue.call(box,conversion);
        if(trace!=='SS'||text.value!=='ab'||box.value!=='a\r\nb')throw 'one DOMString conversion';
        setValue.call(text,null);if(text.value!=='')throw 'LegacyNullToEmptyString';
        setValue.call(text);if(text.value!=='undefined')throw 'omitted setter value';
        setValue.call(box,undefined);if(box.value!=='undefined')throw 'explicit undefined';
        setValue.call(box,{toString:function(){return null;}});
        if(box.value!=='null')throw 'null conversion result was treated as original null';
        var before=text.value,caught=0;
        try{setValue.call(text,Symbol('value'));}catch(e){if(e instanceof TypeError)caught++;}
        try{setValue.call(text,{toString:function(){throw marker;}});}catch(e){if(e===marker)caught++;}
        if(caught!==2||text.value!==before)throw 'abrupt value conversion mutated native state';
        var hooks=0,truth={toString:function(){hooks++;throw marker;},valueOf:function(){hooks++;throw marker;}};
        setChecked.call(box,truth);if(!box.checked||hooks)throw 'boolean object conversion';
        setChecked.call(box,Symbol('checked'));if(!box.checked||hooks)throw 'boolean Symbol conversion';
        setChecked.call(box);if(box.checked)throw 'undefined false';
        setChecked.call(box,'');if(box.checked)throw 'empty false';
        setChecked.call(box,'false');if(!box.checked)throw 'nonempty true';
        var invalid=0;
        for(var receiver of [null,undefined,{},proto,document,document.getElementById('area'),apply]){
            try{setValue.call(receiver,conversion);}catch(e){if(e instanceof TypeError)invalid++;}
            try{getChecked.call(receiver);}catch(e){if(e instanceof TypeError)invalid++;}
        }
        if(invalid!==14||trace!=='SS')throw 'brand must precede conversion';
        try{new getValue();throw 'getter was constructible';}catch(e){if(!(e instanceof TypeError))throw e;}
    "#,
    );
}

#[test]
fn input_accessor_ordinary_shadows_forward_saved_native_accessors_and_trusted_event_delivery() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var reads=0,writes=0,checkedWrites=0,events=0,raw={};
        Object.defineProperty(text,'value',{
            configurable:true,enumerable:true,
            get:function(){reads++;return getValue.call(this);},
            set:function(v){writes++;return setValue.call(this,v);}
        });
        Object.defineProperty(box,'checked',{
            configurable:true,
            get:function(){return getChecked.call(this);},
            set:function(v){checkedWrites++;return setChecked.call(this,v);}
        });
        text.addEventListener('input',function(e){
            if(!e.isTrusted||e.target!==text||e.currentTarget!==text||this!==text)
                throw 'native input event';
            events++;out.textContent=text.value;
        });
        apply.addEventListener('click',function(e){
            if(!e.isTrusted)throw 'trusted button';
            text.value={toString:function(){return 'from button';}};
            box.checked=false;
            if(getValue.call(text)!=='from button'||getChecked.call(box)!==false)
                throw 'own forwarding descriptor skipped native slots';
        });
    "#,
    );
    let apply = id(&doc, "apply");
    page.begin_job();
    let click = page.dispatch(&mut doc, &mut forms, apply, "click", true);
    assert_eq!(click.errors, 0, "{}", page.report());
    let text = id(&doc, "text");
    forms.script_set_value(text, "from native editor").unwrap();
    page.begin_job();
    let input = page.dispatch(&mut doc, &mut forms, text, "input", true);
    assert_eq!(input.errors, 0, "{}", page.report());
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(writes!==1||checkedWrites!==1||events!==1||out.textContent!=='from native editor'||
           !reads||getValue.call(text)!=='from native editor'||text.defaultValue!=='default')
            throw 'native delivery/forwarded live state';
        delete text.value;delete box.checked;
        if(text.value!=='from native editor'||box.checked)throw 'own deletion did not reveal native accessor';
        Object.defineProperty(proto,'value',{configurable:true,get:function(){return 'author';},
            set:function(v){if(this!==text||v!==raw)throw 'raw inherited setter';writes++;}});
        text.value=raw;
        if(text.value!=='author'||getValue.call(text)!=='from native editor'||writes!==2||
           Object.getOwnPropertyDescriptor(text,'value')!==undefined)throw 'author prototype replacement';
        delete proto.value;
        if(text.value!==undefined||'value' in text)throw 'ghost native fallback after deletion';
        text.value=raw;
        if(text.value!==raw||getValue.call(text)!=='from native editor')throw 'ordinary own data after deletion';
        delete text.value;Object.defineProperty(proto,'value',valueDescriptor);
        if(text.value!=='from native editor')throw 'canonical restoration';
    "#,
    );
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
}

#[test]
fn input_accessor_detached_instances_moves_and_reset_preserve_native_defaults_and_owners() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var detached=document.createElement('input');
        if(Object.getPrototypeOf(detached)!==proto||getValue.call(detached)!==''||
           detached.form!==null||detached.isConnected)throw 'fresh detached native admission';
        detached.defaultValue='original\r\ndefault';
        setValue.call(detached,'edited\r\nvalue');
        var attr=detached.getAttributeNode('value');
        if(detached.value!=='editedvalue'||detached.defaultValue!=='original\r\ndefault'||
           detached.getAttributeNode('value')!==attr)throw 'detached native default/live values';
        document.getElementById('destination').appendChild(detached);
        if(detached.form!==form||!detached.isConnected||detached.ownerDocument!==document)
            throw 'connected native owner';
        document.getElementById('destination').removeChild(detached);
        if(detached.form!==null||detached.isConnected||getValue.call(detached)!=='editedvalue')
            throw 'retained detached native value';
        document.getElementById('destination').appendChild(detached);
        setChecked.call(box,false);setValue.call(box,'changed default');
        if(box.getAttribute('checked')!==''||box.getAttribute('value')!=='changed default')
            throw 'checked dirty state vs default/on attribute';
    "#,
    );
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    forms.reset(id(&doc, "f"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(getValue.call(detached)!=='originaldefault'||detached.defaultValue!=='original\r\ndefault'||
           detached.getAttributeNode('value')!==attr||attr.ownerElement!==detached||
           !getChecked.call(box)||getValue.call(box)!=='changed default')
            throw 'native reset/default/identity';
    "#,
    );
    // Reset includes the neighboring input by design; no preceding operation
    // may disturb its selection or live value before that explicit reset.
    let neighbor = &forms.controls[&id(&doc, "neighbor")];
    assert_eq!(neighbor.value.text(), "seed");
    assert!(!neighbor.dirty_value);
}

#[test]
fn input_accessor_realms_keep_distinct_intrinsics_and_cached_origin_checks_before_conversion() {
    let (mut doc, mut forms, mut page) = fixture_with_page(Page::with_limits(crate::javascript::limits::BROWSER));
    let kept = editor(&forms, id(&doc, "neighbor"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
        var w=frame.contentWindow,F=w.Function,ChildTypeError=w.TypeError;
        var ChildInput=w.HTMLInputElement,childProto=ChildInput.prototype;
        var childValue=Object.getOwnPropertyDescriptor(childProto,'value');
        var childChecked=Object.getOwnPropertyDescriptor(childProto,'checked');
        var childGet=childValue.get,childSet=childValue.set;
        if(ChildInput===HTMLInputElement||childProto===proto||childGet===getValue||
           childSet===setValue||Object.getPrototypeOf(childGet)!==w.Function.prototype)
            throw 'realm-owned descriptor functions';
        if(childGet.call(text)!=='default'||childChecked.get.call(box)!==true)
            throw 'same-origin borrowed native receiver';
        var childError=false;
        try{childGet.call(w.document.body);}catch(e){childError=Object.getPrototypeOf(e)===ChildTypeError.prototype;}
        if(!childError)throw 'accessor brand error realm';
        var conversions=0,conversion={toString:function(){conversions++;return 'converted';}};
        childSet.call(text,'same-origin');
        if(text.value!=='same-origin'||conversions)throw 'borrowed setter';
    "#,
    );
    let text = editor(&forms, id(&doc, "text"));
    page.set_location(&location("https://other-input-interface.test/changed"));
    for source in ["childGet.call(text);", "childSet.call(text,conversion);"] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(editor(&forms, id(&doc, "text")), text);
        assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    }
    page.set_location(&location("https://input-interface.test/start"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(conversions!==0||childGet.call(text)!=='same-origin')throw 'origin refusal effects or recovery';
        childSet.call(text,conversion);
        if(conversions!==1||text.value!=='converted')throw 'restored native caller realm';
    "#,
    );
}

#[test]
fn input_accessor_existing_native_mode_and_resource_limits_refuse_without_state_changes() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var longValue='x';while(longValue.length<8192)longValue+=longValue;
        var file=document.getElementById('file'),area=document.getElementById('area');
        if(getChecked.call(text)!==false||Object.getOwnPropertyDescriptor(text,'checked')!==undefined)
            throw 'genuine inherited noncheckable checkedness';
        setChecked.call(text,true);
        if(!getChecked.call(text)||text.getAttribute('checked')!==null||text.value!=='default')
            throw 'noncheckable native boolean state';
        setChecked.call(text,false);
    "#,
    );
    let text_before = editor(&forms, id(&doc, "text"));
    let box_before = editor(&forms, id(&doc, "box"));
    let before_nodes = format!("{:?}", doc.nodes);
    let retained = page.state.retained;
    for source in [
        "setValue.call(text,longValue);",
        "setValue.call(file,'not a file');",
        "setChecked.call(area,true);",
        "getChecked.call(area);",
        "setValue.call(area,'textarea has a separate interface');",
        "box.innerHTML='<input>';",
        "text.type='file';",
    ] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}\n{}", page.report());
        assert_eq!(format!("{:?}", doc.nodes), before_nodes, "{source}");
        assert_eq!(editor(&forms, id(&doc, "text")), text_before, "{source}");
        assert_eq!(editor(&forms, id(&doc, "box")), box_before, "{source}");
        assert_eq!(editor(&forms, id(&doc, "neighbor")), kept, "{source}");
        assert_eq!(page.state.retained, retained, "{source}");
        assert!(!page.has_navigation_request());
    }
    // A new native default must preflight its retained bytes before mutation.
    page.state.retained_limit = retained;
    let result = page.console_eval(
        &mut doc,
        &mut forms,
        "setValue.call(box,'a longer default that cannot be retained');",
    );
    assert_eq!(result.errors, 1, "{}", page.report());
    assert_eq!(format!("{:?}", doc.nodes), before_nodes);
    assert_eq!(editor(&forms, id(&doc, "box")), box_before);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    page.state.retained_limit = page.state.limits.host.retained_bytes;
    run(&mut doc, &mut forms, &mut page, "setValue.call(text,'recovered');if(text.value!=='recovered'||box.value!=='yes')throw 'later job recovery';");
}
