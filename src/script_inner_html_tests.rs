use super::*;

fn run(markup: &str, source: &str) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&crate::navigation::address("https://example.test/page", &crate::navigation::Location::home()).unwrap());
    let outcome = page.start(&mut doc, &mut forms, &[Script { node: 0, source: source.into(), label: "original innerHTML fixture".into() }]);
    (doc, forms, page, outcome)
}

#[test]
fn inner_html_serializes_actual_children_namespaces_and_raw_text() {
    let (doc, _, page, outcome) = run("<html><body><div id=root><p id=p title='a &quot; &amp; &lt; &gt;'>Café&nbsp;<b>real</b><br>tail</p><script>var text='<>&';</script></div></body></html>", r#"
var root=document.getElementById('root'),p=document.getElementById('p');
if(typeof root.innerHTML!=='string'||!('innerHTML' in root)||'innerHTML' in p.firstChild)throw Error('surface');
if(root.innerHTML!=='<p id="p" title="a &quot; &amp; &lt; &gt;">Café&nbsp;<b>real</b><br>tail</p><script>var text=\'<>&\';</script>')throw Error('actual serialization');
p.setAttributeNS('http://www.w3.org/XML/1998/namespace','other:lang','en');
p.setAttributeNS('http://www.w3.org/2000/xmlns/','xmlns:q','urn:q');
p.setAttributeNS('http://www.w3.org/1999/xlink','z:href','&x');
p.setAttributeNS('urn:other','o:Key','"<>');
if(root.innerHTML.indexOf('xml:lang="en" xmlns:q="urn:q" xlink:href="&amp;x" o:Key="&quot;&lt;&gt;"')<0)throw Error('namespace serialization');
if(root.innerHTML!==root.innerHTML)throw Error('stable order');document.title='serialization-ok';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "serialization-ok");
}

#[test]
fn inner_html_replaces_real_nodes_and_preserves_detached_references_and_values() {
    let (doc, forms, page, outcome) = run("<html><body><input id=field value=original><div id=root><p id=old title=kept>Old<b>child</b></p></div></body></html>", r#"
var root=document.getElementById('root'),old=document.getElementById('old'),child=old.lastChild,attr=old.getAttributeNode('title'),list=root.querySelectorAll('p');
old.saved={value:7};document.getElementById('field').value='edited';
root.innerHTML='<p id="new" title="&quot; &amp; &lt; &gt;">Café &nbsp;<b>real</b><br>tail</p><script id="inert">window.executed=true;</script>';
var fresh=document.getElementById('new'),inert=document.getElementById('inert');
if(root.firstChild!==fresh||fresh.nextSibling!==inert||inert.nextSibling!==null||fresh.parentNode!==root||fresh.firstChild.nodeType!==3||fresh.querySelector('b').textContent!=='real'||fresh.getAttribute('title')!=='" & < >')throw Error('real replacement');
if(old.parentNode!==null||old.isConnected||old.ownerDocument!==document||child.parentNode!==old||child.isConnected||old.textContent!=='Oldchild'||old.saved.value!==7||attr.ownerElement!==old||attr.value!=='kept'||list[0]!==old||list.length!==1)throw Error('detached identity');
if(document.getElementById('old')!==null||document.getElementById('field').value!=='edited'||window.executed!==undefined)throw Error('lookup/forms/inert');
inert.src='/must-not-fetch.js';inert.text='window.executed=true;';document.body.appendChild(inert);
if(window.executed!==undefined)throw Error('persistent inertness');
root.innerHTML=null;if(root.firstChild!==null||root.innerHTML!==''||fresh.parentNode!==null)throw Error('null clear');
root.innerHTML=undefined;if(root.firstChild.nodeType!==3||root.textContent!=='undefined')throw Error('undefined text');
var marker={},caught=false;try{root.innerHTML={toString:function(){throw marker;}};}catch(e){caught=e===marker;}
if(!caught||root.textContent!=='undefined')throw Error('abrupt conversion');
caught=false;try{root.innerHTML=Symbol();}catch(e){caught=e.name==='TypeError';}if(!caught||root.textContent!=='undefined')throw Error('Symbol');
root.innerHTML={toString:function(){root.innerHTML='<em>inner</em>';return '<strong>outer</strong>';}};
if(root.innerHTML!=='<strong>outer</strong>')throw Error('reentrant conversion');document.title='replacement-ok';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "replacement-ok");
    assert_eq!(forms.controls.values().next().unwrap().value.text(), "edited");
    assert!(page.state.dynamic.pending.is_empty());
    assert_eq!(page.state.dom.fragment_scripts.len(), 1);
}

#[test]
fn inner_html_context_and_parser_limits_do_not_commit_a_prefix() {
    let rcdata = crate::html::parse_fragment("\n<b>&amp;</b></textarea>", "textarea", true).unwrap();
    assert!(matches!(&rcdata.nodes[1].kind, NodeKind::Text(text) if text == "\n<b>&</b></textarea>"));
    let raw = crate::html::parse_fragment("<img src=x>&amp;", "script", true).unwrap();
    assert!(matches!(&raw.nodes[1].kind, NodeKind::Text(text) if text == "<img src=x>&amp;"));
    let data = crate::html::parse_fragment("<pre>\nX</pre><pre>&#10;Y</pre><p>a<p>b", "div", true).unwrap();
    assert!(matches!(&data.nodes[data.nodes[1].children[0]].kind, NodeKind::Text(text) if text == "X"));
    assert!(matches!(&data.nodes[data.nodes[3].children[0]].kind, NodeKind::Text(text) if text == "Y"));
    assert_eq!(data.nodes[0].children.len(), 4);
    for context in ["table", "tr", "select", "template", "svg"] { assert!(crate::html::parse_fragment("<b>x</b>", context, true).is_err()); }
    for source in ["<!--comment-->","<xmp><img src=x></xmp>","<plaintext><img src=x>","<picture><img src=x></picture>","<b><p>x</b>y","<p><b>x<div>y","a</br>b","</p>","<dl><dt>a<dd>b</dl>","<h1>a<h2>b</h2>","<script><!--<script></script><img src=x></script>","<img! src=x>","<imgé src=x>","<img =src=x>","<img\u{b}src=x>","</>","</!>","a\0b"] {
        assert!(crate::html::parse_fragment(source, "div", true).is_err(), "{source}");
    }
    let vt = crate::html::parse_fragment("<img src\u{b}='/not-a-source'>", "div", true).unwrap();
    assert_eq!(vt.attr(1, "src"), None);
    assert_eq!(vt.attr(1, "src\u{b}"), Some("/not-a-source"));
    let script = crate::html::parse_fragment("<script>x</script\u{b}><img src=still-script>", "div", true).unwrap();
    assert_eq!(script.nodes.len(), 3);
    assert!(matches!(&script.nodes[2].kind, NodeKind::Text(text) if text.contains("<img")));
    assert!(crate::html::parse_fragment(&"<div>".repeat(129), "div", true).is_err());
    assert!(crate::html::parse_fragment(&"<br>".repeat(20_000), "div", true).is_err());
}

#[test]
fn inner_html_empty_table_preserves_siblings_and_owner_in_flow() {
    let (doc, _, page, outcome) = run("<form id=f><input id=editor value=seed></form><div id=root></div>", r#"
var editor=document.getElementById('editor'),owner=document.getElementById('f');
editor.value='edited';
function verify(root) {
  root.innerHTML='<span>prefix</span><table></table>';
  var prefix=root.firstChild,table=prefix.nextSibling;
  if(prefix.textContent!=='prefix'||table.nodeName!=='TABLE'||table.parentNode!==root||
     table.ownerDocument!==document||table.firstChild!==null||table.nextSibling!==null)throw Error('empty table sibling');
  root.innerHTML='  <TABLE id="grid" data-label="a&amp;b"></TaBlE > after';
  table=root.firstChild.nextSibling;
  if(root.firstChild.nodeType!==3||root.firstChild.textContent!=='  '||table.namespaceURI!=='http://www.w3.org/1999/xhtml'||
     table.childNodes.length!==0||table.nextSibling.textContent!==' after'||table.getAttribute('data-label')!=='a&b'||
     table.isConnected!==root.isConnected||root.innerHTML!=='  <table id="grid" data-label="a&amp;b"></table> after')throw Error('table round trip');
}
verify(document.createElement('div'));verify(document.getElementById('root'));
if(editor!==document.getElementById('editor')||editor.ownerDocument!==document||editor.form!==owner||
   editor.value!=='edited'||editor.defaultValue!=='seed')throw Error('neighbor');
document.title='table-ok';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "table-ok");
}

#[test]
fn inner_html_empty_table_requires_explicit_balanced_tokens_in_flow() {
    for source in [
        "<table>", "<table", "<table id='unfinished", "</table>", "<table/>", "<table/></table>",
        "<table></tabl", "<table></table", "<table></table x>", "<table></table/>", "<table></table/ >",
        "<table / ></table>", "<table>\n</table>", "<table>&#32;</table>", "<table>x</table>",
        "<table><!--text--></table>", "<table><b>text</b></table>", "<table><tr><td>x</td></tr></table>",
        "<table><table></table></table>", "<table></div></table>", "<p><table></table></p>",
        "<div><b><table></table></b></div>", "<select><table></table></select>",
        "<div><table></table></table></div>",
    ] {
        assert!(crate::html::parse_fragment(source, "div", true).is_err(), "{source}");
    }
    for context in ["table", "tbody", "tr", "td", "select", "p", "svg"] {
        assert!(crate::html::parse_fragment("<table></table>", context, true).is_err(), "{context}");
    }
    let nested = crate::html::parse_fragment("<section><table></table><table></table></section>", "div", true).unwrap();
    assert_eq!(nested.nodes[1].children.len(), 2);
    assert!(crate::html::parse_fragment(&format!("{}<table></table>", "<div>".repeat(127)), "div", true).is_err());
}

#[test]
fn inner_html_rejection_keeps_tree_forms_resources_and_retained_storage() {
    let (mut doc, mut forms, mut page, outcome) = run(
        "<html><body><form id=f><input id=control form=f value=original><div id=root><span id=old title=kept>old</span><input id=inside value=seed></div></form></body></html>",
        "var outside=document.getElementById('control'),inside=document.getElementById('inside');outside.value='edited';inside.value='inside edited';"
    );
    assert_eq!(outcome.errors, 0);
    let root = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("root")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let before = bridge.doc.nodes.len();
    let children = bridge.doc.nodes[root].children.clone();
    // Radio groups still need separate native admission and rollback semantics.
    for source in ["<input type=radio value=new>","<form></form>","<div id=f>collision</div>","<style>p{color:red}</style>","<link rel=stylesheet href=x>","<iframe></iframe>","<p onclick='x()'>handler</p>","<div><svg></svg></div>","<img srcset=x src=y>","<span>prefix</span><table><tbody></tbody></table>","<img src='/pending.ppm'><table></table><table>content</table>","<table></table><link rel=stylesheet href='/late.css'>","<table></table><input type=checkbox><input type=radio>"] {
        assert!(bridge.set_inner_html(root, source).is_err(), "{source}");
        assert_eq!(bridge.doc.nodes.len(), before, "{source}");
        assert_eq!(bridge.doc.nodes[root].children, children, "{source}");
        assert!(!bridge.state.images.has_changes());
    }
    let retained = bridge.state.retained;
    let previous_limit = bridge.state.retained_limit;
    bridge.state.retained_limit = retained;
    assert!(bridge.set_inner_html(root, "<img src='/must-not-fetch.ppm'>").is_err());
    assert!(bridge.state.images.entries.is_empty() && !bridge.state.images.has_changes());
    assert_eq!(bridge.doc.nodes.len(), before);
    assert_eq!(bridge.doc.nodes[root].children, children);
    bridge.state.retained_limit = retained + 32;
    assert!(bridge.set_inner_html(root, &format!("<{}></{}>", "custom".repeat(20), "custom".repeat(20))).is_err());
    assert_eq!(bridge.state.retained, retained);
    bridge.state.retained_limit = previous_limit;
    let check = page.console_eval(&mut doc, &mut forms, r#"
var form=document.getElementById('f'),holder=document.getElementById('root');
if(outside!==document.getElementById('control')||outside.parentNode!==form||outside.form!==form||
   outside.value!=='edited'||outside.defaultValue!=='original'||outside.ownerDocument!==document)throw Error('neighbor rollback');
if(inside!==holder.lastChild||inside!==document.getElementById('inside')||inside.form!==form||
   inside.value!=='inside edited'||inside.defaultValue!=='seed'||inside.ownerDocument!==document)throw Error('replacement rollback');
"#);
    assert_eq!(check.errors, 0, "{}", page.report());
}

#[test]
fn inner_html_rejects_old_stylesheet_context_without_losing_its_generation() {
    let (mut doc, mut forms, mut page, outcome) = run("<html><head></head><body><div id=root></div></body></html>", "var root=document.getElementById('root'),sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/pending.css';root.appendChild(sheet);");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let root = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("root")).unwrap();
    let old = doc.nodes[root].children[0];
    let pending = page.take_dynamic_stylesheets();
    assert_eq!(pending.len(), 1);
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.set_inner_html(root, "<p>replacement</p>").unwrap_err().contains("stylesheet/frame"));
    assert_eq!(bridge.doc.nodes[root].children, [old]);
    assert_eq!(bridge.state.tree.parents[old], Some(root));
    assert!(bridge.state.dom.connected[old]);
    assert!(bridge.state.stylesheets.removed.contains(&old)); // Original insertion notification only.
    assert!(bridge.state.dom.fragment_scripts.is_empty());
    drop(bridge);
    let mut styles = HashMap::new();
    let outcome = page.complete_dynamic_stylesheet(&mut doc, &mut forms, &mut styles, &pending[0], Ok("#root{color:red}".into()));
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(styles.get(&old).map(String::as_str), Some("#root{color:red}"));
}
