//! Original Page/native text INPUT fragment admission and retirement controls.
use super::*;

fn fixture(source: &str) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse("<html><body><input id=keep value=original><form id=a action='/result'><div id=host></div><input name=after value=tail></form><form id=b><div id=other></div></form><input id=associated form=a value=associated><fieldset disabled><div id=blocked></div><legend><div id=allowed></div></legend></fieldset><svg id=foreign></svg><template id=inert></template></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap());
    let result = page.start(&mut doc, &mut forms, &[Script { node: 0, source: source.into(), label: "original text INPUT fragment".into() }]);
    (doc, forms, page, result)
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn controls(forms: &Forms) -> Vec<(usize, Kind, String, bool, Option<usize>, bool)> {
    forms.controls.iter().map(|(&node, control)|
        (node, control.kind, control.value.text(), control.dirty_value, control.owner, control.disabled)).collect()
}

#[test]
fn text_fragment_admits_default_empty_and_text_with_real_native_defaults() {
    let (mut doc, mut forms, mut page, result) = fixture("document.getElementById('keep').value='owner edited';");
    assert_eq!(result.errors, 0, "{}", page.report());
    let keep = id(&doc, "keep");
    forms.controls.get_mut(&keep).unwrap().value.cursor = 2;
    forms.controls.get_mut(&keep).unwrap().value.anchor = 5;
    forms.controls.get_mut(&keep).unwrap().value.offset = 1;
    let result = page.console_eval(&mut doc, &mut forms,
        r#"document.getElementById('host').innerHTML='<input id=plain><input id=empty type=""><input id=typed type=TeXt name=q value="a&#13;&#10;b" maxlength=3 tabindex=+2tail readonly required>';"#);
    assert_eq!(result.errors, 0, "{}", page.report());
    for name in ["plain", "empty", "typed"] {
        let control = &forms.controls[&id(&doc, name)];
        assert_eq!(control.kind, Kind::Text);
        assert_eq!(control.owner, Some(id(&doc, "a")));
        assert!(!control.dirty_value);
    }
    let typed = id(&doc, "typed");
    assert_eq!(forms.controls[&typed].name, "q");
    assert_eq!(forms.controls[&typed].value.text(), "ab");
    assert_eq!(forms.text_default_value(typed).unwrap(), "a\r\nb");
    assert_eq!((forms.controls[&typed].max_length, forms.controls[&typed].tab_index), (3, 2));
    assert!(forms.controls[&typed].readonly && forms.controls[&typed].required);
    assert_eq!(forms.controls[&keep].value.text(), "owner edited");
    assert!(forms.controls[&keep].dirty_value);
    assert_eq!((forms.controls[&keep].value.cursor, forms.controls[&keep].value.anchor, forms.controls[&keep].value.offset), (2, 5, 1));
}

#[test]
fn text_fragment_whole_moves_retirement_and_reconnect_preserve_editor_and_form_order() {
    let (mut doc, mut forms, mut page, result) = fixture("var host=document.getElementById('host');host.innerHTML='<section id=group><input id=fresh name=q value=default></section>';var group=document.getElementById('group'),fresh=document.getElementById('fresh');fresh.value='edited';");
    assert_eq!(result.errors, 0, "{}", page.report());
    let fresh = id(&doc, "fresh");
    forms.controls.get_mut(&fresh).unwrap().value.cursor = 1;
    forms.controls.get_mut(&fresh).unwrap().value.anchor = 3;
    forms.controls.get_mut(&fresh).unwrap().value.offset = 1;
    let current = crate::navigation::address("http://fixture.test/start", &crate::navigation::Location::home()).unwrap();
    assert_eq!(forms.submit_request(&doc, id(&doc, "a"), None, &current, &current).unwrap().location.url.unwrap().query.as_deref(), Some("q=edited&after=tail"));
    let result = page.console_eval(&mut doc, &mut forms, "host.innerHTML='<p>replacement</p>';");
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(forms.controls[&fresh].owner, None);
    assert_eq!(forms.controls[&fresh].value.text(), "edited");
    assert!(forms.controls[&fresh].dirty_value);
    assert_eq!(forms.submit_request(&doc, id(&doc, "a"), None, &current, &current).unwrap().location.url.unwrap().query.as_deref(), Some("after=tail"));
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('other').appendChild(group);");
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(forms.controls[&fresh].owner, Some(id(&doc, "b")));
    assert_eq!((forms.controls[&fresh].value.cursor, forms.controls[&fresh].value.anchor, forms.controls[&fresh].value.offset), (1, 3, 1));
    assert_eq!(forms.submit_request(&doc, id(&doc, "b"), None, &current, &current).unwrap().location.url.unwrap().query.as_deref(), Some("q=edited"));
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('blocked').appendChild(group);");
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(forms.controls[&fresh].disabled);
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('allowed').appendChild(group);");
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(!forms.controls[&fresh].disabled);
    assert_eq!(forms.controls[&fresh].value.text(), "edited");
    assert!(forms.controls[&fresh].dirty_value);
}

#[test]
fn text_fragment_unsupported_types_contexts_and_associations_refuse_atomically() {
    let (mut doc, mut forms, mut page, result) = fixture("document.getElementById('keep').value='owner edited';");
    assert_eq!(result.errors, 0);
    let host = id(&doc, "host");
    let node_count = doc.nodes.len();
    let children = doc.nodes[host].children.clone();
    let original = controls(&forms);
    {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for source in ["<input type=hidden>", "<input type=checkbox>", "<input type=radio>", "<input type=search>", "<input type=file>", "<input type=unknown>", "<input type=' text '>", "<input form=a>", "<input onclick='x=1'>", "<svg><input></svg>", "<input><link rel=stylesheet href=x>"] {
            assert!(bridge.set_inner_html(host, source).is_err(), "{source}");
            assert_eq!(bridge.doc.nodes.len(), node_count);
            assert_eq!(bridge.doc.nodes[host].children, children);
            assert_eq!(controls(bridge.forms), original);
            assert!(!bridge.state.images.has_changes());
        }
    }
    let result = page.console_eval(&mut doc, &mut forms, "document.getElementById('host').innerHTML='<input id=fresh>';var fresh=document.getElementById('fresh');");
    assert_eq!(result.errors, 0, "{}", page.report());
    let fresh = id(&doc, "fresh");
    let host_children = doc.nodes[host].children.clone();
    let original = controls(&forms);
    for source in ["document.getElementById('foreign').appendChild(fresh);", "document.getElementById('inert').appendChild(fresh);", "fresh.innerHTML='not native contents';", "document.getElementById('host').appendChild(document.getElementById('associated'));"] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}: {}", page.report());
        assert_eq!(doc.nodes[host].children, host_children);
        assert_eq!(controls(&forms), original);
        assert_eq!(forms.controls[&fresh].owner, Some(id(&doc, "a")));
    }
}

#[test]
fn text_fragment_batch_control_and_value_limits_publish_no_partial_state() {
    for (existing, source) in [
        ("<input>".repeat(forms::MAX_CONTROLS - 1), "<input><input>".to_owned()),
        (String::new(), format!("<input value='{}'>", "🦀".repeat(forms::MAX_VALUE_UNITS / 2 + 1))),
        ((0..32).map(|_| format!("<input value='{}'>", "x".repeat(forms::MAX_VALUE_UNITS))).collect::<String>(), "<input value=x>".to_owned()),
    ] {
        let mut doc = crate::html::parse(&format!("<html><body>{existing}<div id=host></div></body></html>"));
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let result = page.start(&mut doc, &mut forms, &[]);
        assert_eq!(result.errors, 0);
        let host = id(&doc, "host");
        let nodes = doc.nodes.len();
        let original = controls(&forms);
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(bridge.set_inner_html(host, &source).is_err());
        assert_eq!(bridge.doc.nodes.len(), nodes);
        assert!(bridge.doc.nodes[host].children.is_empty());
        assert_eq!(controls(bridge.forms), original);
    }
    let (mut doc, mut forms, mut page, result) = fixture("");
    assert_eq!(result.errors, 0);
    let host = id(&doc, "host");
    let nodes = doc.nodes.len();
    let original = controls(&forms);
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let retained = bridge.state.retained;
    bridge.state.retained_limit = retained;
    assert!(bridge.set_inner_html(host, "<input>").is_err());
    assert_eq!(bridge.state.retained, retained);
    bridge.state.retained_limit = retained + 1024 * 1024;
    bridge.state.node_limit = nodes;
    assert!(bridge.set_inner_html(host, "<input>").is_err());
    bridge.state.node_limit = nodes + 16;
    bridge.state.host_work = 0;
    assert!(bridge.set_inner_html(host, "<input>").is_err());
    assert_eq!(bridge.doc.nodes.len(), nodes);
    assert!(bridge.doc.nodes[host].children.is_empty());
    assert_eq!(controls(bridge.forms), original);
}
