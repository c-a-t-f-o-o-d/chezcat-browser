//! Original created text INPUT admission/name controls; no framework code.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Created native INPUT</title><body><form id=f action='/result'><input id=neighbor name=neighbor value=original><div id=host></div></form><form id=g action='/other'><div id=destination></div></form><svg id=foreign><g id=foreignchild></g></svg><template id=inert></template></body>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let base = current();
    page.set_location(&base);
    page.set_resource_base(&base);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}

fn current() -> crate::navigation::Location {
    crate::navigation::address("https://created-input.test/start", &crate::navigation::Location::home()).unwrap()
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

fn controls(forms: &Forms) -> Vec<(usize, Kind, String, String, bool, Option<usize>, bool, (usize, usize, usize), Option<String>)> {
    forms.controls.iter().map(|(&node, control)| (
        node, control.kind, control.name.clone(), control.value.text(), control.dirty_value,
        control.owner, control.disabled,
        (control.value.cursor, control.value.anchor, control.value.offset),
        forms.text_default_value(node).ok().map(str::to_owned),
    )).collect()
}

fn query(doc: &Document, forms: &Forms, owner: &str) -> Option<String> {
    let base = current();
    forms.submit_request(doc, id(doc, owner), None, &base, &base).unwrap().location.url.unwrap().query
}

#[test]
fn created_text_input_exact_vue_attribute_value_insert_order_is_native() {
    let (mut doc, mut forms, mut page) = fixture();
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "owner edited").unwrap();
    forms.controls.get_mut(&neighbor).unwrap().value.cursor = 2;
    forms.controls.get_mut(&neighbor).unwrap().value.anchor = 5;
    forms.controls.get_mut(&neighbor).unwrap().value.offset = 1;
    let before = controls(&forms);
    run(&mut doc, &mut forms, &mut page, r#"
var input=document.createElement('input');
input.setAttribute('name','reactive-name');
input.setAttribute('type','text');
if(input.name!=='reactive-name'||input.value!==''||input.defaultValue!=='')throw 'detached state before domProps';
if(input.form!==null||input.parentNode!==null||input.isConnected)throw 'detached ownership';
input.value='edited';
var label=document.createElement('label');label.appendChild(input);
document.getElementById('host').appendChild(label);
input.id='created';
if(input.ownerDocument!==document||input.form!==document.getElementById('f')||input.value!=='edited'||input.defaultValue!=='')throw 'inserted native state';
if(document.getElementById('f')['reactive-name']!==input)throw 'live native named lookup';
"#);
    let node = id(&doc, "created");
    let control = &forms.controls[&node];
    assert_eq!(control.kind, Kind::Text);
    assert_eq!(control.name, "reactive-name");
    assert_eq!(control.value.text(), "edited");
    assert!(control.dirty_value);
    assert_eq!(control.owner, Some(id(&doc, "f")));
    assert_eq!(controls(&forms).into_iter().find(|row| row.0 == neighbor), before.into_iter().find(|row| row.0 == neighbor));
    assert_eq!(query(&doc, &forms, "f").as_deref(), Some("neighbor=owner+edited&reactive-name=edited"));
}

#[test]
fn created_text_input_name_attr_aliases_defaults_reset_and_submission_agree() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var input=document.createElement('input');input.id='created';
input.defaultValue='a\r\nb';input.name='first';
document.getElementById('host').appendChild(input);
if(input.value!=='ab'||input.defaultValue!=='a\r\nb')throw 'initial default sanitization';
input.value='ab';input.defaultValue='latest';
if(input.value!=='ab'||input.defaultValue!=='latest')throw 'equal live assignment did not become dirty';
var conversions=0;input.name={toString:function(){conversions++;return 'second';}};
if(conversions!==1||input.name!=='second'||input.getAttribute('name')!=='second')throw 'ordered name conversion';
var alias=input.getAttributeNode('name');alias.value='submitted';
if(input.name!=='submitted'||input.getAttributeNode('name')!==alias||document.getElementById('f').submitted!==input)throw 'Attr name update';
input.removeAttributeNode(alias);
if(input.name!==''||alias.ownerElement!==null||alias.value!=='submitted')throw 'removed name alias';
input.setAttributeNode(alias);
if(input.name!=='submitted'||alias.ownerElement!==input||input.getAttributeNode('name')!==alias)throw 'reattached name alias';
"#);
    let node = id(&doc, "created");
    assert_eq!(forms.controls[&node].name, "submitted");
    assert_eq!(forms.controls[&node].value.text(), "ab");
    assert!(forms.controls[&node].dirty_value);
    assert_eq!(query(&doc, &forms, "f").as_deref(), Some("neighbor=original&submitted=ab"));
    forms.reset(id(&doc, "f"));
    assert_eq!(forms.controls[&node].value.text(), "latest");
    assert!(!forms.controls[&node].dirty_value);
    run(&mut doc, &mut forms, &mut page, r#"
input.setAttribute('value','after reset');
if(input.value!=='after reset'||input.defaultValue!=='after reset'||input.name!=='submitted')throw 'reset pristine reflection';
input.removeAttribute('name');
if(input.name!==''||input.value!=='after reset'||input.defaultValue!=='after reset')throw 'name removal touched value state';
"#);
    assert_eq!(forms.controls[&node].name, "");
    assert_eq!(query(&doc, &forms, "f").as_deref(), Some("neighbor=original"));
}

#[test]
fn created_text_input_bare_append_reads_text_type_and_hidden_first_keep_distinct_modes() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var bare=document.createElement('input');bare.id='bare';
document.getElementById('host').appendChild(bare);
if(bare.value!==''||bare.defaultValue!==''||bare.name!==''||bare.type!=='text'||bare.form!==document.getElementById('f'))throw 'bare native admission';
bare.setAttribute('type','TeXt');bare.type='';bare.removeAttribute('type');
if(bare.type!=='text'||bare.getAttribute('type')!==null||bare.value!=='')throw 'same text state reflection';
var read=document.createElement('input');read.id='read';
if(read.value!==''||read.defaultValue!==''||read.name!==''||read.form!==null)throw 'fresh read state';
var hidden=document.createElement('input');hidden.id='hidden';hidden.type='HIDDEN';
hidden.name='hidden-name';hidden.value='hidden value';
document.getElementById('host').appendChild(hidden);
if(hidden.type!=='hidden'||hidden.value!=='hidden value'||hidden.defaultValue!=='hidden value')throw 'hidden first changed mode';
var metadata=document.createElement('input');metadata.id='metadata';
metadata.setAttributeNS('urn:created-input-test','type','hidden');
metadata.setAttributeNS('urn:created-input-test','name','foreign-name');
metadata.setAttributeNS(null,'NAME','uppercase-name');
document.getElementById('host').appendChild(metadata);
if(metadata.type!=='text'||metadata.name!=='')throw 'metadata became native state';
if(metadata.getAttributeNS(null,'type')!==null||metadata.getAttributeNS(null,'name')!==null)throw 'metadata entered the native attribute namespace';
if(metadata.getAttribute('type')!=='hidden'||metadata.getAttributeNS('urn:created-input-test','type')!=='hidden'||metadata.getAttributeNS('urn:created-input-test','name')!=='foreign-name'||metadata.getAttributeNS(null,'NAME')!=='uppercase-name')throw 'metadata was not retained with its exact name';
"#);
    for name in ["bare", "read", "metadata"] {
        let control = &forms.controls[&id(&doc, name)];
        assert_eq!(control.kind, Kind::Text);
        assert_eq!(control.value.text(), "");
        assert!(!control.dirty_value);
    }
    assert_eq!(forms.controls[&id(&doc, "hidden")].kind, Kind::Hidden);
    assert_eq!(forms.controls[&id(&doc, "read")].owner, None);
}

#[test]
fn created_text_input_whole_moves_keep_editor_default_and_neighbor_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var input=document.createElement('input');input.id='created';input.name='q';input.defaultValue='default';input.value='owner edited';
var shell=document.createElement('div');shell.appendChild(input);document.getElementById('host').appendChild(shell);
document.getElementById('neighbor').value='neighbor edited';
"#);
    let node = id(&doc, "created");
    let neighbor = id(&doc, "neighbor");
    for entry in [node, neighbor] {
        let editor = &mut forms.controls.get_mut(&entry).unwrap().value;
        editor.cursor = 2; editor.anchor = 5; editor.offset = 1;
    }
    let before = controls(&forms);
    run(&mut doc, &mut forms, &mut page, r#"
var destination=document.getElementById('destination');destination.appendChild(shell);
if(input.form!==document.getElementById('g')||input.value!=='owner edited'||input.defaultValue!=='default')throw 'move state';
destination.removeChild(shell);
if(input.form!==null||input.isConnected||input.value!=='owner edited')throw 'detached state';
document.getElementById('host').appendChild(shell);
if(input.form!==document.getElementById('f')||!input.isConnected||input.value!=='owner edited')throw 'reconnect state';
"#);
    assert_eq!(controls(&forms), before);
    assert_eq!(query(&doc, &forms, "f").as_deref(), Some("neighbor=neighbor+edited&q=owner+edited"));
}

#[test]
fn created_text_input_unsupported_context_type_and_interior_changes_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var input=document.createElement('input');input.id='created';input.name='q';input.defaultValue='default';input.value='edited';document.getElementById('host').appendChild(input);");
    let node = id(&doc, "created");
    let host = id(&doc, "host");
    let foreign = id(&doc, "foreign");
    let inert = id(&doc, "inert");
    for source in [
        "input.type='hidden';", "input.type='checkbox';", "input.type='search';", "input.type=' text ';",
        "document.getElementById('foreign').appendChild(input);",
        "document.getElementById('inert').appendChild(input);",
        "input.innerHTML='<span>not native contents</span>';",
        "input.appendChild(document.createTextNode('not native contents'));",
    ] {
        let original = controls(&forms);
        let children = [host, foreign, inert, node].map(|entry| doc.nodes[entry].children.clone());
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}\n{}", page.report());
        assert_eq!(controls(&forms), original, "{source}");
        assert_eq!([host, foreign, inert, node].map(|entry| doc.nodes[entry].children.clone()), children, "{source}");
        assert_eq!(doc.attr(node, "type"), None);
        assert_eq!(doc.attr(node, "name"), Some("q"));
    }
    run(&mut doc, &mut forms, &mut page, "var associated=document.createElement('input');associated.id='associated';");
    let associated = id(&doc, "associated");
    // The public form-attribute policy already rejects this setup. Inject the
    // content attribute as a fixture to exercise admission's independent guard.
    if let NodeKind::Element { attrs, .. } = &mut doc.nodes[associated].kind {
        attrs.push(crate::html::Attribute { name: crate::html::AttributeName::unqualified("form".into()), value: "f".into() });
    }
    let before = controls(&forms);
    let children = doc.nodes[host].children.clone();
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('host').appendChild(associated);");
    assert_eq!(result.errors, 1, "{}", page.report());
    assert_eq!(controls(&forms), before);
    assert_eq!(doc.nodes[host].children, children);
    assert!(!forms.controls.contains_key(&associated));
}

#[test]
fn created_text_input_attribute_and_read_quota_failures_publish_no_control() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var fresh=document.createElement('input');fresh.id='fresh';");
    let node = id(&doc, "fresh");
    let before = controls(&forms);
    let retained = page.state.retained;
    {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for (name, value) in [("name", "n".repeat(8193)), ("value", "🦀".repeat(forms::MAX_VALUE_UNITS / 2 + 1))] {
            assert!(bridge.public_attribute(node, crate::html::AttributeName::unqualified(name.into()), Some(value)).is_err());
            assert!(!bridge.forms.controls.contains_key(&node));
            assert_eq!(bridge.doc.attr(node, name), None);
            assert_eq!(bridge.state.retained, retained);
        }
        bridge.state.retained_limit = retained;
        assert!(bridge.public_attribute(node, crate::html::AttributeName::unqualified("name".into()), Some("growth".into())).is_err());
        assert!(!bridge.forms.controls.contains_key(&node));
        assert_eq!(bridge.doc.attr(node, "name"), None);
        assert_eq!(bridge.state.retained, retained);
        bridge.state.retained_limit = retained + 1024 * 1024;
        bridge.state.host_work = 0;
        assert!(bridge.public_attribute(node, crate::html::AttributeName::unqualified("name".into()), Some("work".into())).is_err());
        assert!(!bridge.forms.controls.contains_key(&node));
        assert_eq!(bridge.doc.attr(node, "name"), None);
    }
    assert_eq!(controls(&forms), before);

    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var fresh=document.createElement('input');fresh.id='fresh';");
    let node = id(&doc, "fresh");
    // A nonempty preexisting raw name makes the result-copy check observable;
    // no native state has yet been published for this created detached node.
    if let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind {
        attrs.push(crate::html::Attribute { name: crate::html::AttributeName::unqualified("name".into()), value: "copy result".into() });
    }
    page.state.retained += "name".len() + "copy result".len();
    let retained = page.state.retained;
    page.state.copy_work = 0;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.get(node_handle(node), "name").is_err());
    assert!(!bridge.forms.controls.contains_key(&node), "A failed property read cannot materialize native state");
    assert_eq!(bridge.doc.attr(node, "name"), Some("copy result"));
    assert_eq!(bridge.state.retained, retained);
}

#[test]
fn created_text_input_name_and_utf16_value_exact_boundaries_remain_complete() {
    let (mut doc, mut forms, mut page) = fixture();
    let name = "n".repeat(8192);
    let value = "🦀".repeat(forms::MAX_VALUE_UNITS / 2);
    // Keep the console source below its separate 8 KiB allowance while the
    // resulting native strings reach their exact byte/UTF-16 boundaries.
    run(&mut doc, &mut forms, &mut page, r#"
var input=document.createElement('input');input.id='created';
var boundaryName='n',boundaryValue='🦀';
for(var n=0;n<13;n++)boundaryName=boundaryName+boundaryName;
for(var v=0;v<11;v++)boundaryValue=boundaryValue+boundaryValue;
input.name=boundaryName;input.defaultValue=boundaryValue;
"#);
    let node = id(&doc, "created");
    assert_eq!(forms.controls[&node].name, name);
    assert_eq!(forms.controls[&node].value.text(), value);
    assert_eq!(forms.text_default_value(node).unwrap(), value);
    assert_eq!(forms.controls[&node].value.text().encode_utf16().count(), forms::MAX_VALUE_UNITS);
    assert!(!forms.controls[&node].dirty_value);
    run(&mut doc, &mut forms, &mut page, "input.value=input.value;input.defaultValue='';");
    assert!(forms.controls[&node].dirty_value);
    assert_eq!(forms.controls[&node].value.text(), value);
    assert_eq!(forms.text_default_value(node).unwrap(), "");
}

#[test]
fn created_text_input_control_and_retained_default_payload_limits_are_atomic() {
    for existing in ["<input type=hidden>".repeat(forms::MAX_CONTROLS), (0..32).map(|_| format!("<input value='{}'>", "x".repeat(forms::MAX_VALUE_UNITS))).collect::<String>()] {
        let mut doc = crate::html::parse(&format!("<html><body><form id=f><div id=host></div>{existing}</form></body></html>"));
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        // Editing defaults down to empty must not hide their retained Reset
        // payload from a subsequent admission calculation.
        let text: Vec<_> = forms.controls.iter().filter(|(_, control)| control.kind == Kind::Text).map(|(&node, _)| node).collect();
        for node in text { forms.script_set_value(node, "").unwrap(); }
        let mut page = Page::for_test();
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        run(&mut doc, &mut forms, &mut page, "var fresh=document.createElement('input');fresh.id='fresh';");
        let node = id(&doc, "fresh");
        let host = id(&doc, "host");
        let before = controls(&forms);
        let retained = page.state.retained;
        let result = page.console_eval(&mut doc, &mut forms, "fresh.name='q';");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert!(!forms.controls.contains_key(&node));
        assert_eq!(doc.attr(node, "name"), None);
        assert!(doc.nodes[host].children.is_empty());
        assert_eq!(controls(&forms), before);
        assert_eq!(page.state.retained, retained);
        if forms.controls.len() == forms::MAX_CONTROLS {
            let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('host').appendChild(fresh);");
            assert_eq!(result.errors, 1, "{}", page.report());
            assert!(doc.nodes[host].children.is_empty());
            assert!(!forms.controls.contains_key(&node));
            assert_eq!(controls(&forms), before);
        }
    }
}

#[test]
fn created_text_input_internal_mixed_resource_topology_refuses_before_publication() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "document.getElementById('host').innerHTML='<span id=old>old</span>';var shell=document.createElement('div');shell.id='shell';var input=document.createElement('input');input.id='created';var script=document.createElement('script');script.id='resource';script.src='/unrequested.js';");
    let shell = id(&doc, "shell");
    let input = id(&doc, "created");
    let script = id(&doc, "resource");
    let host = id(&doc, "host");
    // This is an internal-topology control, not a claimed reachable JS RED:
    // public append normally admits the input before an aggregate can form.
    doc.nodes[shell].children = vec![input, script];
    page.state.tree.parents[input] = Some(shell);
    page.state.tree.parents[script] = Some(shell);
    let before = controls(&forms);
    let children = [host, shell, input, script].map(|node| doc.nodes[node].children.clone());
    let parents = page.state.tree.parents.clone();
    let connected = page.state.dom.connected.clone();
    let retained = page.state.retained;
    for source in ["document.getElementById('host').appendChild(shell);", "document.getElementById('host').replaceChild(shell,document.getElementById('old'));"] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert_eq!(controls(&forms), before);
        assert!(!forms.controls.contains_key(&input));
        assert_eq!([host, shell, input, script].map(|node| doc.nodes[node].children.clone()), children);
        assert_eq!(page.state.tree.parents, parents);
        assert_eq!(page.state.dom.connected, connected);
        assert_eq!(page.state.retained, retained);
        assert!(page.take_dynamic_scripts().is_empty());
        assert!(page.take_dynamic_stylesheets().is_empty());
        assert!(page.take_image_changes().2.is_empty());
    }
}

#[test]
fn created_text_input_attribute_adoption_keeps_native_retained_charge_and_combined_limit() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var input=document.createElement('input');input.id='created';");
    let node = id(&doc, "created");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let attr = bridge.create_attribute(&[Value::String("name".into())], false).unwrap();
    let Value::Host(handle) = attr else { panic!("Real detached Attr identity"); };
    bridge.attr_set(handle, "value", Value::String("q".into())).unwrap();
    let identity = bridge.state.attributes.len() - 1;
    let retained = bridge.state.retained;
    let qname_bytes = bridge.state.attributes[identity].name.bytes();
    // Adopting this Attr adds one element QName copy and one byte of retained
    // native control name. The Attr value itself transfers without duplication.
    bridge.state.retained_limit = retained + qname_bytes;
    assert!(bridge.set_attribute_node(node_handle(node), &[Value::Host(handle)]).is_err());
    assert_eq!(bridge.state.retained, retained);
    assert_eq!(bridge.state.attributes[identity].owner, None);
    assert_eq!(bridge.state.attributes[identity].value, "q");
    assert_eq!(bridge.doc.attr(node, "name"), None);
    assert!(!bridge.forms.controls.contains_key(&node));
    bridge.state.retained_limit = retained + qname_bytes + 1;
    bridge.set_attribute_node(node_handle(node), &[Value::Host(handle)]).unwrap();
    assert_eq!(bridge.state.retained, retained + qname_bytes + 1);
    assert_eq!(bridge.state.attributes[identity].owner, Some(node));
    assert_eq!(bridge.doc.attr(node, "name"), Some("q"));
    assert_eq!(bridge.forms.controls[&node].name, "q");
    assert_eq!(bridge.forms.controls[&node].kind, Kind::Text);
}

#[test]
fn created_text_input_type_dispatch_converts_once_for_hidden_and_refused_transitions() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var typeCalls=0, hidden=document.createElement('input');hidden.id='hidden';
hidden.type={toString:function(){typeCalls++;return 'hidden';}};
if(typeCalls!==1||hidden.type!=='hidden')throw 'hidden-first conversion';
hidden.type={toString:function(){typeCalls++;return 'HIDDEN';}};
if(typeCalls!==2||hidden.type!=='hidden'||hidden.getAttribute('type')!=='HIDDEN')throw 'existing hidden conversion';
var refusedCalls=0, refusedType={toString:function(){refusedCalls++;return 'hidden';}};
"#);
    let before = controls(&forms);
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('neighbor').type=refusedType;");
    assert_eq!(result.errors, 1, "{}", page.report());
    assert_eq!(controls(&forms), before);
    run(&mut doc, &mut forms, &mut page, "if(refusedCalls!==1||document.getElementById('neighbor').type!=='text')throw 'refused transition conversion';");

    // Host string conversion has its own copy charge even when the VM has
    // already performed the observable toString hook. Compare with the prior
    // single hidden dispatcher to catch a second host conversion as well.
    fn copy_cost(use_hidden_dispatch: bool, existing_hidden: bool) -> usize {
        let (mut doc, mut forms, mut page) = fixture();
        if existing_hidden {
            run(&mut doc, &mut forms, &mut page, "var hidden=document.createElement('input');hidden.id='hidden';hidden.type='hidden';");
        }
        let node = id(&doc, if existing_hidden { "hidden" } else { "neighbor" });
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let before = bridge.state.copy_work;
        let value = Value::String("hidden".into());
        let result = if use_hidden_dispatch {
            bridge.hidden_input_set(node_handle(node), "type", &value).map(|accepted| assert!(accepted))
        } else { bridge.set(node_handle(node), "type", value) };
        assert_eq!(result.is_ok(), existing_hidden);
        before - bridge.state.copy_work
    }
    for existing_hidden in [true, false] {
        assert_eq!(copy_cost(false,existing_hidden),copy_cost(true,existing_hidden), "Type dispatch must charge one converted RHS");
    }
}
