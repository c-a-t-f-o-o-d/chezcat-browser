//! Original native INPUT fragment/default-value and metadata reductions.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<!doctype html><title>Native INPUT</title><body><form id=f><input id=neighbor name=neighbor value=original><div id=box><span id=old>Before</span></div><input id=parsed name=parsed value=before><button id=button type=BuTtOn>Apply</button><button id=auto>Auto</button><button id=invalid type=unknown>Unknown</button><button id=command command=--original>Command</button></form><form id=g><div id=destination></div></form><svg><g id=foreign></g></svg>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let base=crate::navigation::address("https://native-input.test/source",&crate::navigation::Location::home()).unwrap();
    page.set_location(&base);page.set_resource_base(&base);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{source}\n{}",page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

#[test]
fn text_input_fragment_exact_native_sequence_and_neighbor_identity() {
    let (mut doc,mut forms,mut page)=fixture();let neighbor=id(&doc,"neighbor");
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    forms.controls.get_mut(&neighbor).unwrap().value.cursor=3;
    forms.controls.get_mut(&neighbor).unwrap().value.anchor=1;
    let editor=&forms.controls[&neighbor].value;
    let before=(editor.chars.clone(),editor.cursor,editor.anchor,editor.offset);
    run(&mut doc,&mut forms,&mut page,r#"
var box=document.getElementById('box'), neighbor=document.getElementById('neighbor');
box.innerHTML='<input/>';
var input=box.firstChild;
input.setAttribute('value','');
if(input.getAttribute('value')!==''||input.value!==''||input.defaultValue!==''||input.type!=='text')throw 'native fragment sequence';
if(input.ownerDocument!==document||input.parentNode!==box||input.form!==document.getElementById('f'))throw 'native ownership';
input.setAttribute('value','first');
if(input.value!=='first'||input.defaultValue!=='first')throw 'pristine reflection';
if(neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original')throw 'neighbor value reset';
"#);
    let control=&forms.controls[&neighbor];
    assert_eq!(control.value.chars,before.0);assert_eq!(control.value.cursor,before.1);assert_eq!(control.value.anchor,before.2);assert_eq!(control.value.offset,before.3);
    assert!(control.dirty_value);assert_eq!(control.owner,Some(id(&doc,"f")));
}

#[test]
fn text_input_default_attr_and_idl_preserve_dirty_value_and_reset_latest_default() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var input=document.getElementById('parsed'), attr=input.getAttributeNode('value');
input.defaultValue={toString:function(){return 'a\r\nb';}};
if(input.defaultValue!=='a\r\nb'||attr.value!=='a\r\nb'||input.value!=='ab')throw 'raw default and sanitized live value';
input.value='ab';
input.setAttribute('value','next default');
if(input.value!=='ab'||input.defaultValue!=='next default'||input.getAttributeNode('value')!==attr)throw 'equal assignment was not dirty';
attr.value='latest';
if(input.value!=='ab'||input.defaultValue!=='latest')throw 'Attr default replaced edited value';
"#);
    forms.reset(id(&doc,"f"));
    run(&mut doc,&mut forms,&mut page,r#"
if(input.value!=='latest')throw 'reset default';
input.defaultValue='after reset';
if(input.value!=='after reset')throw 'reset left dirty flag';
input.removeAttribute('value');
if(input.value!==''||input.defaultValue!==''||attr.ownerElement!==null||attr.value!=='after reset')throw 'default removal';
"#);
}

#[test]
fn text_input_fragment_moves_preserve_native_editor_owner_and_detached_state() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var box=document.getElementById('box'), destination=document.getElementById('destination');
box.innerHTML='<input id="inserted" name="inserted" value="initial">';
var input=box.firstChild;input.value='edited';
destination.appendChild(input);
if(input.form!==document.getElementById('g')||input.ownerDocument!==document||input.value!=='edited'||input.defaultValue!=='initial')throw 'moved native identity';
destination.removeChild(input);
if(input.form!==null||input.value!=='edited')throw 'detached native state';
box.appendChild(input);
if(input.form!==document.getElementById('f')||input.value!=='edited'||input.ownerDocument!==document)throw 'reconnection';
"#);
    let input=id(&doc,"inserted");assert!(forms.controls[&input].dirty_value);
    assert_eq!(forms.controls[&input].owner,Some(id(&doc,"f")));
}

#[test]
fn text_input_failed_default_and_fragment_transactions_are_atomic() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var input=document.getElementById('parsed'), box=document.getElementById('box'), old=box.firstChild, attr=input.getAttributeNode('value');input.value='edited';");
    let node=id(&doc,"parsed");let retained=page.state.retained;
    page.state.retained_limit=retained;
    let result=page.console_eval(&mut doc,&mut forms,"input.defaultValue='a much longer default';");
    assert_eq!(result.errors,1,"{}",page.report());
    assert_eq!(doc.attr(node,"value"),Some("before"));assert_eq!(forms.text_default_value(node).unwrap(),"before");
    assert_eq!(forms.controls[&node].value.text(),"edited");assert_eq!(page.state.retained,retained);
    page.state.retained_limit=page.state.limits.host.retained_bytes;
    for source in ["box.innerHTML='<input type=checkbox>';","document.getElementById('foreign').innerHTML='<input>';","input.innerHTML='<span>inside</span>';"] {
        let nodes=doc.nodes.len();let controls=forms.controls.len();
        let result=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(result.errors,1,"{source}\n{}",page.report());
        assert_eq!(doc.nodes.len(),nodes);assert_eq!(forms.controls.len(),controls);
        run(&mut doc,&mut forms,&mut page,"if(box.firstChild!==old||input.value!=='edited'||input.defaultValue!=='before'||attr!==input.getAttributeNode('value'))throw 'failed transaction changed state';");
    }
}

#[test]
fn text_input_button_type_metadata_uses_real_html_receiver_and_auto_state() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
if(document.getElementById('button').type!=='button'||document.getElementById('auto').type!=='submit'||document.getElementById('invalid').type!=='submit'||document.getElementById('command').type!=='button')throw 'button type metadata';
var detached=document.createElement('button');
if(detached.type!=='submit'||!('type' in detached))throw 'detached button';
var input=document.getElementById('parsed');
input.setAttributeNS('urn:metadata','m:value','foreign default');
input.setAttributeNS(null,'VALUE','uppercase metadata');
if(input.value!=='before'||input.defaultValue!=='before')throw 'metadata changed native default';
"#);
}
