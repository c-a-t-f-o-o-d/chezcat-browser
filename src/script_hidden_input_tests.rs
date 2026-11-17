//! Original hidden-input attribute/tree/form controls, without library code.
use super::*;

fn run(source: &str) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse("<html><body><form id=f action='/result'><div id=host></div><input id=keep name=neighbor value=keep></form><form id=g><div id=other></div><input id=parsed type=hidden name=P value=seed></form><fieldset disabled><div id=blocked></div><legend><div id=allowed></div></legend></fieldset><svg id=foreign><g id=foreignchild></g></svg><p id=out>ready</p></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap());
    let outcome = page.start(&mut doc, &mut forms, &[Script { node: 0, source: source.into(), label: "original hidden input".into() }]);
    (doc, forms, page, outcome)
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()
}

#[test]
fn hidden_input_values_defaults_tree_moves_and_submission_keep_identity() {
    let (doc, mut forms, page, outcome) = run(r#"
var input=document.createElement('input');input.type='HIDDEN';input.id='new';
input.name='D';input.defaultValue='default';input.value='edited 雪&+';
if(input.type!=='hidden'||input.getAttribute('type')!=='HIDDEN'||input.defaultValue!=='edited 雪&+'||input.getAttribute('value')!=='edited 雪&+')throw 'hidden default value mode';
var host=document.getElementById('host'),other=document.getElementById('other'),saved=input;
host.appendChild(input);if(input.form!==document.getElementById('f'))throw 'first owner';
other.appendChild(input);if(input.form!==document.getElementById('g')||input!==saved)throw 'second owner';
other.removeChild(input);if(input.form!==null||input.isConnected||input.value!=='edited 雪&+')throw 'detach';
host.appendChild(input);if(input!==saved||input.form!==document.getElementById('f'))throw 'reconnect';
var shell=document.createElement('div');host.replaceChild(shell,input);if(saved.form!==null||saved.value!=='edited 雪&+')throw 'replace out';
host.replaceChild(saved,shell);if(saved.form!==document.getElementById('f')||!saved.isConnected)throw 'replace in';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let input=id(&doc,"new");
    assert_eq!(forms.controls[&input].kind,Kind::Hidden);
    forms.reset(id(&doc,"f"));
    assert_eq!(forms.controls[&input].value.text(),"edited 雪&+","Hidden reset follows the current value attribute");
    let current=crate::navigation::address("http://fixture.test/start",&crate::navigation::Location::home()).unwrap();
    assert_eq!(forms.submit_request(&doc,id(&doc,"f"),None,&current,&current).unwrap().location.url.unwrap().query.as_deref(),Some("D=edited+%E9%9B%AA%26%2B&neighbor=keep"));
}

#[test]
fn hidden_input_parsed_attribute_nodes_and_null_value_follow_default_mode() {
    let (doc, mut forms, page, outcome)=run(r#"
var input=document.getElementById('parsed');
if(input.type!=='hidden'||input.defaultValue!=='seed'||input.value!=='seed'||input.name!=='P')throw 'parsed state';
var value=input.getAttributeNode('value');value.value='attr edited';
if(input.value!=='attr edited'||input.defaultValue!=='attr edited'||input.getAttributeNode('value')!==value)throw 'Attr update';
input.setAttribute('name','renamed');if(input.name!=='renamed'||document.getElementById('g').renamed!==input)throw 'live name';
input.removeAttribute('value');if(input.value!==''||input.defaultValue!=='')throw 'removed default';
var conversions=0;input.value={toString:function(){conversions++;return 'converted';}};
if(conversions!==1||input.value!=='converted'||input.defaultValue!=='converted')throw 'ordered native string conversion';
input.value=null;if(input.value!==''||input.getAttribute('value')!=='')throw 'null value';
input.defaultValue=null;if(input.value!=='null'||input.defaultValue!=='null')throw 'ordinary default conversion';
input.setAttributeNS('urn:original-test','value','foreign');
if(input.value!=='null'||input.getAttributeNS('urn:original-test','value')!=='foreign')throw 'namespace isolation';
input.focus();
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(page.state.dom.pending_focus,None,"Hidden input cannot acquire a focus request");
    let input=id(&doc,"parsed");forms.reset(id(&doc,"g"));
    assert_eq!(forms.controls[&input].value.text(),"null");
    assert_eq!(forms.controls[&input].name,"renamed");
}

#[test]
fn hidden_input_container_retirement_and_disabled_legend_preserve_neighbor_edits() {
    let (mut doc, mut forms, mut page, outcome)=run("document.getElementById('keep').value='owner edited';");
    assert_eq!(outcome.errors,0,"{}",page.report());
    let keep=id(&doc,"keep");
    forms.controls.get_mut(&keep).unwrap().value.cursor=2;
    forms.controls.get_mut(&keep).unwrap().value.anchor=5;
    forms.controls.get_mut(&keep).unwrap().value.offset=1;
    let outcome=page.console_eval(&mut doc,&mut forms,r#"
var box=document.createElement('div'),input=document.createElement('input');
input.type='hidden';input.id='new';input.name='D';input.value='retained';box.appendChild(input);
document.getElementById('blocked').appendChild(box);
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());let input=id(&doc,"new");
    assert!(forms.controls[&input].disabled);
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('allowed').appendChild(box);");
    assert_eq!(outcome.errors,0,"{}",page.report());assert!(!forms.controls[&input].disabled);
    let outcome=page.console_eval(&mut doc,&mut forms,"var host=document.getElementById('host');host.appendChild(box);host.innerHTML='<p>replacement</p>';if(input.form!==null||input.isConnected||input.value!=='retained')throw 'whole container retired';host.appendChild(box);if(input.form!==document.getElementById('f')||input.value!=='retained')throw 'whole container reconnect';");
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(forms.controls[&keep].value.text(),"owner edited");
    assert_eq!((forms.controls[&keep].value.cursor,forms.controls[&keep].value.anchor,forms.controls[&keep].value.offset),(2,5,1));
    assert!(!forms.controls[&input].focusable());
}

#[test]
fn hidden_input_unsupported_type_changes_and_transaction_refusals_keep_both_models() {
    let (mut doc,mut forms,mut page,outcome)=run("");assert_eq!(outcome.errors,0);
    let input=id(&doc,"parsed");let text=id(&doc,"keep");
    let before=doc.attr(input,"value").unwrap().to_owned();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for (node,name,value) in [(text,"type",Some("hidden".to_owned())),(input,"type",Some("text".to_owned())),(input,"type",None),(input,"type",Some("file".to_owned())),(input,"name",Some("n".repeat(8193))),(input,"value",Some("雪".repeat(4097)))] {
        assert!(bridge.public_attribute(node,crate::html::AttributeName::unqualified(name.into()),value).is_err());
        assert_eq!(bridge.doc.attr(input,"type"),Some("hidden"));
        assert_eq!(bridge.doc.attr(text,"type"),None);
        assert_eq!(bridge.forms.controls[&input].value.text(),before);
        assert_eq!(bridge.doc.attr(input,"value"),Some(before.as_str()));
        assert_eq!(bridge.forms.controls[&text].kind,Kind::Text);
    }
    let retained=bridge.state.retained;bridge.state.retained_limit=retained;
    assert!(bridge.public_attribute(input,crate::html::AttributeName::unqualified("value".into()),Some("grown".repeat(50))).is_err());
    assert_eq!(bridge.state.retained,retained);
    assert_eq!(bridge.forms.controls[&input].value.text(),before);
    assert_eq!(bridge.doc.attr(input,"value"),Some(before.as_str()));
    bridge.state.retained_limit=retained+1024*1024;bridge.state.host_work=0;
    assert!(bridge.public_attribute(input,crate::html::AttributeName::unqualified("name".into()),Some("changed".into())).is_err());
    assert_eq!(bridge.forms.controls[&input].name,"P");assert_eq!(bridge.doc.attr(input,"name"),Some("P"));
}

#[test]
fn hidden_input_namespace_spelling_preserves_default_text_admission() {
    let (mut doc,mut forms,mut page,outcome)=run("var input=document.createElement('input');input.id='new';input.setAttributeNS('urn:original-test','type','hidden');if(input.type!=='text'||input.getAttributeNS(null,'type')!==null)throw 'namespaced type became native';");
    assert_eq!(outcome.errors,0,"{}",page.report());let input=id(&doc,"new");
    assert!(!forms.controls.contains_key(&input));
    let host=id(&doc,"host");
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('host').appendChild(input);");
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert!(doc.nodes[host].children.contains(&input));assert_eq!(forms.controls[&input].kind,Kind::Text);
    assert_eq!(doc.attr(input,"type"),None,"Namespaced hidden metadata cannot select native hidden mode");
}

#[test]
fn hidden_input_control_limit_refuses_attribute_admission_without_partial_native_state() {
    let mut markup=String::from("<html><body><div id=host></div>");
    markup.push_str(&"<input type=hidden name=x>".repeat(forms::MAX_CONTROLS));markup.push_str("</body></html>");
    let mut doc=crate::html::parse(&markup);doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);assert_eq!(forms.controls.len(),forms::MAX_CONTROLS);
    let mut page=Page::for_test();
    let outcome=page.start(&mut doc,&mut forms,&[Script{node:0,source:"var input=document.createElement('input');input.id='new';input.setAttribute('type','hidden');".into(),label:"original hidden control limit".into()}]);
    assert_ne!(outcome.errors,0,"The native control limit must be enforced before type publication");
    assert!(page.report().contains("Native form control limit reached"),"{}",page.report());
    let input=id(&doc,"new");assert_eq!(doc.attr(input,"type"),None);
    assert!(!forms.controls.contains_key(&input));assert_eq!(forms.controls.len(),forms::MAX_CONTROLS);
}

#[test]
fn hidden_input_html_parent_succeeds_and_foreign_parent_refusal_is_atomic() {
    let (mut doc,mut forms,mut page,outcome)=run("var input=document.createElement('input');input.type='hidden';input.id='new';input.name='D';input.value='retained';document.getElementById('host').appendChild(input);if(input.form!==document.getElementById('f'))throw 'HTML owner';");
    assert_eq!(outcome.errors,0,"{}",page.report());
    let input=id(&doc,"new");let host=id(&doc,"host");let foreign=id(&doc,"foreign");
    assert_ne!(doc.nodes[foreign].namespace,crate::html::Namespace::Html,"Original foreign parser provenance must be retained");
    let original_html=doc.nodes[host].children.clone();let original_foreign=doc.nodes[foreign].children.clone();
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('foreign').appendChild(input);");
    assert_ne!(outcome.errors,0,"Native hidden form state must not cross an unimplemented foreign ancestry boundary");
    assert_eq!(doc.nodes[host].children,original_html);assert_eq!(doc.nodes[foreign].children,original_foreign);
    assert_eq!(forms.controls[&input].owner,Some(id(&doc,"f")));
    assert_eq!(forms.controls[&input].value.text(),"retained");
}

#[test]
fn hidden_input_created_type_append_and_name_are_real_native_form_state() {
    let (doc, forms, page, outcome) = run(r#"
var input=document.createElement('input');
input.setAttribute('type','hidden');
document.getElementById('host').appendChild(input);
input.setAttribute('name','D');
if(input.getAttribute('type')!=='hidden'||input.getAttribute('name')!=='D')throw 'content attributes';
if(input.form!==document.getElementById('f')||input.value!=='')throw 'native hidden state';
document.getElementById('out').textContent='hidden input ready';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let control = forms.controls.values().find(|control| control.name == "D").expect("Created hidden input has retained native form state");
    assert_eq!(control.kind, Kind::Hidden);
    assert_eq!(control.value.text(), "");
    let current = crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap();
    let form = (0..doc.nodes.len()).find(|&node| doc.attr(node,"id")==Some("f")).unwrap();
    assert_eq!(forms.submit_request(&doc, form, None, &current, &current).unwrap().location.url.unwrap().query.as_deref(), Some("D=&neighbor=keep"));
}
