//! Original Page-level fragment/control ownership regressions; no libraries.
use super::*;
fn run(source: &str) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse("<html><body><input id=keep value=original><form id=a action='/result'><div id=host></div><input name=after value=tail></form><form id=b><div id=other></div></form><fieldset disabled><div id=blocked></div><legend><div id=allowed></div></legend></fieldset><p id=events></p></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap());
    let result = page.start(&mut doc, &mut forms, &[Script { node: 0, source: source.into(), label: "original select fragment".into() }]);
    (doc, forms, page, result)
}
fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}
const SELECT: &str = r#"var host=document.getElementById('host');host.innerHTML='<select id=s name=choice><option value=a>Alpha<optgroup label=Disabled disabled><option value=x>Disabled</optgroup><optgroup label=Choices><option value=b selected>Beta</optgroup></select>';var s=document.getElementById('s');"#;
#[test]
fn select_fragment_page_has_genuine_native_defaults_groups_and_live_selected_value() {
    let (doc, mut forms, page, result) = run(&format!("{SELECT}if(s.value!=='b'||s.selectedIndex!==2||s.form!==document.getElementById('a'))throw 'native state';if(host.querySelectorAll('[selected]').length!==1||host.querySelectorAll(':checked').length!==1)throw 'selected attribute';s.value='a';if(s.selectedIndex!==0)throw 'live value';"));
    assert_eq!(result.errors, 0, "{}", page.report());
    let control = &forms.controls[&id(&doc, "s")];
    assert_eq!(control.kind, Kind::Select);
    assert_eq!(control.value.text(), "a");
    assert_eq!(control.select.as_ref().unwrap().options.len(), 3);
    assert!(control.select.as_ref().unwrap().options[1].disabled);
    forms.reset(id(&doc, "a"));
    assert_eq!(forms.controls[&id(&doc, "s")].value.text(), "b", "New native select retains its authored reset default");
}
#[test]
fn select_fragment_moves_disconnects_and_reconnects_preserve_identity_value_and_submit_order() {
    let (doc, forms, page, result) = run(&format!("{SELECT}var a=document.getElementById('a'),b=document.getElementById('b'),saved=s;s.value='a';b.appendChild(s);if(s!==saved||s.form!==b||s.value!=='a')throw 'move';b.removeChild(s);if(s.form!==null||s.isConnected||document.getElementById('s')!==null||s.value!=='a')throw 'detach';a.insertBefore(s,host);if(s!==saved||s.form!==a||!s.isConnected||s.value!=='a')throw 'reconnect';var replacement=document.createElement('div');a.replaceChild(replacement,s);if(saved.form!==null||saved.isConnected||saved.value!=='a')throw 'replace out';a.replaceChild(saved,replacement);if(saved.form!==a||saved.value!=='a'||!saved.isConnected)throw 'replace in';"));
    assert_eq!(result.errors, 0, "{}", page.report());
    let current = crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap();
    let submission = forms.submit_request(&doc, id(&doc, "a"), None, &current, &current).unwrap();
    assert_eq!(submission.location.url.unwrap().query.as_deref(), Some("choice=a&after=tail"));
}
#[test]
fn select_fragment_detached_values_survive_replacement_and_owner_does_not_leak_to_submission() {
    let (doc, forms, page, result) = run(&format!("{SELECT}var saved=s,option=s.firstChild;s.value='a';host.innerHTML='<p id=replacement>Next</p>';if(saved.value!=='a'||saved.form!==null||saved.isConnected||saved.firstChild!==option||option.parentNode!==saved)throw 'retained state';"));
    assert_eq!(result.errors, 0, "{}", page.report());
    let old = forms.controls.values().find(|control| control.kind == Kind::Select).unwrap();
    assert_eq!(old.value.text(), "a");
    assert_eq!(old.owner, None);
    let current = crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap();
    assert_eq!(forms.submit_request(&doc, id(&doc, "a"), None, &current, &current).unwrap().location.url.unwrap().query.as_deref(), Some("after=tail"));
}
#[test]
fn select_fragment_respects_disabled_fieldset_legend_and_keeps_unrelated_edited_editor() {
    let (mut doc, mut forms, mut page, result) = run("document.getElementById('keep').value='owner edited';");
    assert_eq!(result.errors, 0);
    let keep = id(&doc, "keep");
    forms.controls.get_mut(&keep).unwrap().value.cursor = 2;
    forms.controls.get_mut(&keep).unwrap().value.anchor = 5;
    forms.controls.get_mut(&keep).unwrap().value.offset = 1;
    let result = page.console_eval(&mut doc, &mut forms, &format!("{SELECT}document.getElementById('blocked').appendChild(s);"));
    assert_eq!(result.errors, 0, "{}", page.report());
    let select = id(&doc, "s");
    assert!(forms.controls[&select].disabled);
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('allowed').appendChild(s);");
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(!forms.controls[&select].disabled);
    assert_eq!(forms.controls[&keep].value.text(), "owner edited");
    assert_eq!((forms.controls[&keep].value.cursor, forms.controls[&keep].value.anchor, forms.controls[&keep].value.offset), (2, 5, 1));
}
#[test]
fn select_fragment_guard_and_budget_refusals_keep_original_tree_controls_and_resources() {
    let (mut doc, mut forms, mut page, result) = run("document.getElementById('keep').value='owner edited';");
    assert_eq!(result.errors, 0);
    let host = id(&doc, "host");
    let nodes = doc.nodes.len();
    let children = doc.nodes[host].children.clone();
    let controls: Vec<_> = forms.controls.keys().copied().collect();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for source in ["<select><option>safe<iframe src=x></iframe></select>", "<select><select><option>x</select>", "<select form=a><option>x</select>", "<select multiple><option>x</select>", "<input type=hidden>", "<select><option>x</select><link rel=stylesheet href=x>"] {
        assert!(bridge.set_inner_html(host, source).is_err(), "{source}");
        assert_eq!(bridge.doc.nodes.len(), nodes);
        assert_eq!(bridge.doc.nodes[host].children, children);
        assert_eq!(bridge.forms.controls.keys().copied().collect::<Vec<_>>(), controls);
        assert!(!bridge.state.images.has_changes());
    }
    let source = "<select><option value=x>X</select>";
    let retained = bridge.state.retained;
    bridge.state.retained_limit = retained;
    assert!(bridge.set_inner_html(host, source).is_err());
    assert_eq!(bridge.state.retained, retained);
    assert_eq!(bridge.doc.nodes.len(), nodes);
    bridge.state.retained_limit = retained + 1024 * 1024;
    bridge.state.node_limit = nodes;
    assert!(bridge.set_inner_html(host, source).is_err());
    assert_eq!(bridge.doc.nodes.len(), nodes);
    bridge.state.node_limit = nodes + 256;
    bridge.state.host_work = 0;
    assert!(bridge.set_inner_html(host, source).is_err());
    assert_eq!(bridge.doc.nodes[host].children, children);
    assert_eq!(bridge.forms.controls.keys().copied().collect::<Vec<_>>(), controls);
    let keep = id(bridge.doc, "keep");
    assert_eq!(bridge.forms.controls[&keep].value.text(), "owner edited");
}
