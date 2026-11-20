//! Original bare LINK fragment controls. No real network work is performed.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn fixture(contents: &str, source: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(&format!("<html><head></head><body><form id=form><input id=neighbor value=seed><div id=holder>{contents}</div><input id=tail value=after></form></body></html>"));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let origin = crate::navigation::address(
        "https://fragment.test/page",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&origin);
    page.set_resource_base(&origin);
    let result = page.start(
        &mut doc,
        &mut forms,
        &[Script {
            node: 0,
            source: source.into(),
            label: "Original inert LINK fragment".into(),
        }],
    );
    assert_eq!(result.errors, 0, "{}", page.report());
    (doc, forms, page)
}
fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc, forms, source);
    assert_eq!(result.errors, 0, "{source}\n{}", page.report());
}

#[derive(Debug, PartialEq, Eq)]
struct Resources {
    entries: Vec<(
        usize,
        Option<(String, Option<String>, Option<String>, Option<String>)>,
        u64,
        usize,
        bool,
    )>,
    pending: Vec<(usize, u64, usize, bool, String, Option<String>)>,
    removed: Vec<usize>,
    totals: (usize, usize, usize, usize),
}
impl Resources {
    fn capture(page: &Page) -> Self {
        let sheets = &page.state.stylesheets;
        let mut entries: Vec<_> = sheets
            .entries
            .iter()
            .map(|(&node, entry)| {
                (
                    node,
                    entry.key.as_ref().map(|key| {
                        (
                            key.href.clone(),
                            key.nonce.clone(),
                            key.integrity.clone(),
                            key.crossorigin.clone(),
                        )
                    }),
                    entry.generation,
                    Arc::as_ptr(&entry.cancel) as usize,
                    entry.cancel.load(Ordering::Acquire),
                )
            })
            .collect();
        entries.sort_by_key(|entry| entry.0);
        let pending = sheets
            .pending
            .iter()
            .map(|request| {
                (
                    request.node,
                    request.generation,
                    Arc::as_ptr(&request.cancel) as usize,
                    request.cancel.load(Ordering::Acquire),
                    request.target.display(),
                    request.error.clone(),
                )
            })
            .collect();
        let mut removed: Vec<_> = sheets.removed.iter().copied().collect();
        removed.sort_unstable();
        Self {
            entries,
            pending,
            removed,
            totals: (
                sheets.pending_storage.load(Ordering::Acquire),
                sheets.in_flight,
                sheets.source_bytes,
                sheets.source_rules,
            ),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    nodes: String,
    parents: Vec<Option<usize>>,
    connected: Vec<bool>,
    created: HashSet<usize>,
    detached: HashSet<usize>,
    native: Vec<(usize, String, bool, Option<usize>, (usize, usize, usize))>,
    retained: usize,
    commits: Option<u64>,
    resources: Resources,
}
impl Snapshot {
    fn capture(doc: &Document, forms: &Forms, page: &Page) -> Self {
        Self {
            nodes: format!("{:?}", doc.nodes),
            parents: page.state.tree.parents.clone(),
            connected: page.state.dom.connected.clone(),
            created: page.state.dom.created.iter().collect(),
            detached: page.state.detached_nodes.iter().collect(),
            native: forms
                .controls
                .iter()
                .map(|(&node, control)| {
                    (
                        node,
                        control.value.text(),
                        control.dirty_value,
                        control.owner,
                        (
                            control.value.cursor,
                            control.value.anchor,
                            control.value.offset,
                        ),
                    )
                })
                .collect(),
            retained: page.state.retained,
            commits: page.state.dom_commits,
            resources: Resources::capture(page),
        }
    }
}
fn edit_neighbor(doc: &Document, forms: &mut Forms) {
    let control = forms.controls.get_mut(&id(doc, "neighbor")).unwrap();
    assert_eq!(control.value.text(), "edited");
    assert!(control.dirty_value);
    control.value.cursor = 2;
    control.value.anchor = 5;
    control.value.offset = 1;
}
fn neighbor(doc: &Document, forms: &Forms) {
    let node = id(doc, "neighbor");
    let control = &forms.controls[&node];
    assert_eq!(control.value.text(), "edited");
    assert!(control.dirty_value);
    assert_eq!(
        (
            control.value.cursor,
            control.value.anchor,
            control.value.offset
        ),
        (2, 5, 1)
    );
    assert_eq!(control.owner, Some(id(doc, "form")));
    assert_eq!(forms.text_default_value(node).unwrap(), "seed");
    assert_eq!(forms.controls[&id(doc, "tail")].value.text(), "after");
}

#[test]
fn inert_link_fragment_connected_detached_identity_serialization_and_retirement() {
    let (mut doc, mut forms, mut page) = fixture("<link>", "document.getElementById('neighbor').value='edited';var holder=document.getElementById('holder'),parsed=holder.firstChild;");
    edit_neighbor(&doc, &mut forms);
    let resources = Resources::capture(&page);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
function verify(root) {
  root.innerHTML='  <LiNk/><table></table><a href="/a">a</a>';
  var space=root.firstChild,link=space.nextSibling,table=link.nextSibling,anchor=table.nextSibling;
  if(space.nodeType!==3||space.textContent!=='  '||link.nodeType!==1||link.nodeName!=='LINK'||
     link.localName!=='link'||link.namespaceURI!=='http://www.w3.org/1999/xhtml'||link.ownerDocument!==document||
     link.parentNode!==root||link.isConnected!==root.isConnected||link.firstChild!==null||link.childNodes.length!==0||
     link.getAttribute('rel')!==null||link.getAttribute('href')!==null||table.nodeName!=='TABLE'||
     table.firstChild!==null||anchor.nodeName!=='A'||anchor.textContent!=='a'||anchor.nextSibling!==null)
    throw Error('real void LINK with original mixed siblings');
  if(root.innerHTML!=='  <link><table></table><a href="/a">a</a>')throw Error('void LINK serialization');
  root.innerHTML='<link>';
  if(link.parentNode!==null||link.isConnected||link.ownerDocument!==document||table.parentNode!==null)
    throw Error('retained detached original identities');
  var last=root.firstChild;root.innerHTML='';
  if(last.parentNode!==null||last.isConnected||last.ownerDocument!==document||root.firstChild!==null)
    throw Error('bare LINK clear');
}
verify(document.createElement('div'));verify(holder);
if(parsed.parentNode!==null||parsed.isConnected||parsed.ownerDocument!==document)throw Error('parsed bare LINK removal');
"#,
    );
    neighbor(&doc, &forms);
    assert_eq!(Resources::capture(&page), resources);
    assert!(page.state.images.entries.is_empty() && !page.state.images.has_changes());
    assert!(page.take_dynamic_scripts().is_empty());
}

#[test]
fn inert_link_fragment_mixed_native_batch_and_later_resource_failure_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture("", "document.getElementById('neighbor').value='edited';var holder=document.getElementById('holder');holder.innerHTML='<link><table></table><a href=/a>a</a><input id=fresh value=original>';document.getElementById('fresh').value='fresh edited';");
    edit_neighbor(&doc, &mut forms);
    page.state.rebuild(&doc).unwrap();
    let holder = id(&doc, "holder");
    let before = Snapshot::capture(&doc, &forms, &page);
    for source in [
        "<link id=metadata>",
        "<link rel=stylesheet href=/late.css>",
        "<link disabled>",
        "<link href=''>",
        "<link nonce=''>",
        "<link style='color:red'>",
        "<link/><table></table><a href=/a>a</a><input type=radio>",
        "<link><img src=/must-not-fetch.ppm><link rel=stylesheet href=/late.css>",
        "<link><img src=/must-not-fetch.ppm><iframe></iframe>",
    ] {
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .set_inner_html(holder, source);
        assert!(result.is_err(), "{source}");
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{source}");
        assert!(page.state.images.entries.is_empty() && !page.state.images.has_changes());
        neighbor(&doc, &forms);
    }
    eval(&mut doc,&mut forms,&mut page,"var fresh=document.getElementById('fresh');if(fresh.value!=='fresh edited'||fresh.defaultValue!=='original'||fresh.form!==document.getElementById('form')||fresh.ownerDocument!==document)throw Error('native rollback');");
}

#[test]
fn inert_link_fragment_preserves_unrelated_pending_generation_and_real_callback() {
    let (mut doc, mut forms, mut page) = fixture("", "document.getElementById('neighbor').value='edited';var loads=0,holder=document.getElementById('holder'),sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/pending.css';sheet.onload=function(){loads++;};document.head.appendChild(sheet);");
    edit_neighbor(&doc, &mut forms);
    let before = Resources::capture(&page);
    assert_eq!(before.pending.len(), 1);
    eval(&mut doc,&mut forms,&mut page,"holder.innerHTML='<link><table></table>';var old=holder.firstChild;holder.innerHTML='<p>replacement</p>';if(old.isConnected||old.parentNode!==null)throw Error('old bare LINK');");
    assert_eq!(Resources::capture(&page), before);
    neighbor(&doc, &forms);
    let pending = page.take_dynamic_stylesheets().pop_front().unwrap();
    assert!(page.owns_dynamic_stylesheet(&pending));
    let mut styles = HashMap::new();
    let result = page.complete_dynamic_stylesheet(
        &mut doc,
        &mut forms,
        &mut styles,
        &pending,
        Ok("#holder{color:red}".into()),
    );
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(
        styles.get(&pending.node).map(String::as_str),
        Some("#holder{color:red}")
    );
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        "if(loads!==1)throw Error('original pending callback');",
    );
}

#[test]
fn inert_link_fragment_later_attributes_use_checked_dynamic_stylesheet_path() {
    for detached in [false, true] {
        let source = format!("var holder=document.getElementById('holder'),shell={};shell.innerHTML='<link>';var sheet=shell.firstChild,loads=0;sheet.onload=function(){{loads++;}};sheet.rel='stylesheet';sheet.setAttribute('href','/first.css');",
        if detached {"document.createElement('div')"} else {"holder"});
        let (mut doc, mut forms, mut page) = fixture("", &source);
        if detached {
            assert!(page.state.stylesheets.pending.is_empty());
            assert_eq!(page.state.stylesheets.pending_storage.load(Ordering::Acquire), 0);
            eval(
                &mut doc,
                &mut forms,
                &mut page,
                "holder.appendChild(shell);",
            );
        }
        let first = page.take_dynamic_stylesheets().pop_front().unwrap();
        assert!(first.error.is_none());
        assert!(first.target.display().ends_with("/first.css"));
        assert!(page.owns_dynamic_stylesheet(&first));
        eval(&mut doc, &mut forms, &mut page, "sheet.href='/second.css';");
        let second = page.take_dynamic_stylesheets().pop_front().unwrap();
        assert!(second.generation > first.generation);
        assert!(first.cancel.load(Ordering::Acquire));
        assert!(!page.owns_dynamic_stylesheet(&first));
        assert!(page.owns_dynamic_stylesheet(&second));
        let mut styles = HashMap::new();
        page.complete_dynamic_stylesheet(
            &mut doc,
            &mut forms,
            &mut styles,
            &first,
            Ok("#holder{color:blue}".into()),
        );
        assert!(styles.is_empty());
        let result = page.complete_dynamic_stylesheet(
            &mut doc,
            &mut forms,
            &mut styles,
            &second,
            Ok("#holder{color:red}".into()),
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            "if(loads!==1)throw Error('checked LINK source ownership');",
        );
    }
}

#[test]
fn inert_link_fragment_active_and_retired_sources_never_lose_history() {
    for retired in [false, true] {
        let (mut doc, mut forms, mut page) = fixture("", "var loads=0,holder=document.getElementById('holder');holder.innerHTML='<link>';var sheet=holder.firstChild;sheet.onload=function(){loads++;};sheet.rel='stylesheet';sheet.href='/pending.css';");
        let request = page.take_dynamic_stylesheets().pop_front().unwrap();
        if retired {
            eval(
                &mut doc,
                &mut forms,
                &mut page,
                "sheet.removeAttribute('href');sheet.removeAttribute('rel');",
            );
            assert!(bare_fragment_link(&doc, request.node));
            assert!(page.state.stylesheets.entries[&request.node].generation > 0);
            assert!(page.state.stylesheets.entries[&request.node].key.is_none());
        }
        page.state.rebuild(&doc).unwrap();
        let before = Snapshot::capture(&doc, &forms, &page);
        let holder = id(&doc, "holder");
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .set_inner_html(holder, "<p>replacement</p>");
        assert!(result.unwrap_err().contains("stylesheet/frame"));
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before);
        let mut styles = HashMap::new();
        let result = page.complete_dynamic_stylesheet(
            &mut doc,
            &mut forms,
            &mut styles,
            &request,
            Ok("#holder{color:red}".into()),
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        if retired {
            assert!(styles.is_empty());
        } else {
            assert!(styles.contains_key(&request.node));
        }
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            if retired {
                "if(loads!==0)throw Error('retired callback');"
            } else {
                "if(loads!==1)throw Error('preserved current callback');"
            },
        );
    }
}

#[test]
fn inert_link_fragment_pristine_empty_entry_is_retained_and_removal_notifications_refuse() {
    let (mut doc, mut forms, mut page) = fixture("", "var holder=document.getElementById('holder'),sheet=document.createElement('link');holder.appendChild(sheet);");
    let node = doc.nodes[id(&doc, "holder")].children[0];
    let entry = &page.state.stylesheets.entries[&node];
    assert!(entry.key.is_none());
    assert_eq!(entry.generation, 0);
    let before = Resources::capture(&page);
    eval(&mut doc,&mut forms,&mut page,"holder.innerHTML='<link>';if(sheet.isConnected||sheet.parentNode!==null||sheet.ownerDocument!==document)throw Error('pristine removal');");
    assert_eq!(
        Resources::capture(&page),
        before,
        "No entry is discarded or retired by the fragment transaction"
    );
    let current = doc.nodes[id(&doc, "holder")].children[0];
    page.state.stylesheets.removed.insert(current);
    page.state.rebuild(&doc).unwrap();
    let before = Snapshot::capture(&doc, &forms, &page);
    let holder = id(&doc, "holder");
    let result = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .set_inner_html(holder, "");
    assert!(result.is_err());
    assert_eq!(Snapshot::capture(&doc, &forms, &page), before);
}
