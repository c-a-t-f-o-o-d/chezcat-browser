//! Original native-control-import construction, coercion, owner and storage checks.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><form id=owner><div id=target>Original</div></form></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://native-control-import-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn native_control_import_defaults_detached_dirty_state_connection_and_reset_are_real() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<original-controls><input id="text" name="q" value="seed"><input id="check" type="checkbox" name="agree" value="yes" checked><textarea id="area" name="note">&lt;seed&gt;</textarea><select id="select" name="pick"><optgroup label="Group"><option value="a">A</option><option value="b" selected>B</option></optgroup></select><button id="button" type="button" name="action" value="go">Commit</button></original-controls>';
var source=t.content,owner=source.ownerDocument,fragment=document.importNode(source,true),text=fragment.querySelector('#text'),check=fragment.querySelector('#check'),area=fragment.querySelector('#area'),select=fragment.querySelector('#select'),button=fragment.querySelector('#button');
if(text.value!=='seed'||text.defaultValue!=='seed'||text.form!==null||check.checked!==true||check.defaultChecked!==true||check.value!=='yes'||area.value!=='<seed>'||area.defaultValue!=='<seed>'||select.value!=='b'||select.selectedIndex!==1||button.value!=='go'||button.type!=='button'||button.form!==null||text.isConnected||text.ownerDocument!==document)throw 'actual detached native defaults';
text.value='edited';check.checked=false;area.value='edited area';select.value='a';
document.getElementById('target').appendChild(fragment);var form=document.getElementById('owner');
if(text.form!==form||area.form!==form||select.form!==form||button.form!==form||check.form!==form||!text.isConnected||text.value!=='edited'||check.checked||area.value!=='edited area'||select.value!=='a'||source.ownerDocument!==owner||source.querySelector('#text').getAttribute('value')!=='seed')throw 'connection preserved real edited values and owners';

"##,
    );
    let owner = doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="owner"))
        .unwrap();
    forms.reset_in_document(&doc, owner).unwrap();
    run(&mut doc,&mut forms,&mut page,"if(text.value!=='seed'||!check.checked||area.value!=='<seed>'||select.value!=='b')throw 'real native reset defaults';");
    assert!(!page.has_host_tasks());
}
#[test]
fn native_control_import_option_identity_and_shallow_boundaries_are_explicit() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<select><option value="one">One</option><option value="two" selected>Two</option></select><textarea>Text</textarea>';var source=t.content.firstChild,copy=document.importNode(source,true),shallow=document.importNode(source,false);
if(copy===source||copy.value!=='two'||copy.children.length!==2||copy.children[0]!==copy.firstChild||copy.children[1]!==copy.lastChild||copy.firstChild.ownerDocument!==document||shallow.children.length!==0||shallow.selectedIndex!==-1||shallow.value!=='')throw 'real option identities and shallow select';
"##,
    );
    let (node, control) = forms
        .controls
        .iter()
        .find(|(_, control)| {
            control
                .select
                .as_ref()
                .is_some_and(|select| select.options.len() == 2)
        })
        .unwrap();
    let options = &control.select.as_ref().unwrap().options;
    assert_eq!(options[0].node, doc.nodes[*node].children[0]);
    assert_eq!(options[1].node, doc.nodes[*node].children[1]);
    for code in [
        "document.importNode(t.content.lastChild,false);",
        "document.importNode(source.firstChild,true);",
    ] {
        let before = doc.nodes.len();
        let controls = forms.controls.len();
        let out = page.console_eval(&mut doc, &mut forms, code);
        assert_eq!(out.errors, 1, "{}", page.report());
        assert_eq!(doc.nodes.len(), before);
        assert_eq!(forms.controls.len(), controls);
    }
}
#[test]
fn native_control_import_unsupported_members_preserve_all_native_and_dom_state() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var t=document.createElement('template');",
    );
    for markup in ["<original-controls><input value=prefix><input type=radio></original-controls>","<original-controls><input value=prefix><input form=owner></original-controls>","<original-controls><input value=prefix><button onclick=forbidden()>Bad</button></original-controls>","<original-controls><input value=prefix><select><option>One</option></select><iframe></iframe></original-controls>"] {
  run(&mut doc,&mut forms,&mut page,&format!("t.innerHTML={markup:?};"));page.state.rebuild(&doc).unwrap();let retained=page.state.retained;let nodes=doc.nodes.len();let controls=forms.controls.len();let commits=page.state.dom_commits;
  let out=page.console_eval(&mut doc,&mut forms,"document.importNode(t.content,true);");assert_eq!(out.errors,1,"{}",page.report());assert_eq!(doc.nodes.len(),nodes);assert_eq!(forms.controls.len(),controls);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);
 }
    run(&mut doc,&mut forms,&mut page,"t.innerHTML='<input value=recovered>';var copy=document.importNode(t.content,true);if(copy.firstChild.value!=='recovered'||copy.firstChild.form!==null)throw 'recovery';");
}
#[test]
fn native_control_import_backing_refusal_never_publishes_default_controls() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<input value=seed><textarea>Text</textarea><button type=button>Go</button>';var f=t.content;");
    page.state.rebuild(&doc).unwrap();
    let Value::Host(handle) = page
        .runtime
        .execute(
            "f;",
            &mut Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state,
            },
        )
        .unwrap()
    else {
        panic!("real source")
    };
    let retained = page.state.retained;
    let count = doc.nodes.len();
    let controls = forms.controls.len();
    let limit = page.state.retained_limit;
    let copies = page.state.copy_work;
    for allowance in [0, 512] {
        page.state.retained_limit = retained + allowance;
        assert!(Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state
        }
        .import_node(&[Value::Host(handle), Value::Bool(true)], None)
        .is_err());
        assert_eq!(page.state.retained, retained);
        assert_eq!(doc.nodes.len(), count);
        assert_eq!(forms.controls.len(), controls);
    }
    page.state.retained_limit = limit;
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .import_node(&[Value::Host(handle), Value::Bool(true)], None)
    .is_err());
    assert_eq!(page.state.retained, retained);
    assert_eq!(doc.nodes.len(), count);
    assert_eq!(forms.controls.len(), controls);
    page.state.copy_work = copies;
    run(&mut doc,&mut forms,&mut page,"var copy=document.importNode(f,true);if(copy.querySelector('input').value!=='seed'||copy.querySelector('textarea').value!=='Text'||copy.querySelector('button').type!=='button')throw 'recovery';");
}
