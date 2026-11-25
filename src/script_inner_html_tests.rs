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
fn markup_serialization_exact_units_escape_pair_join_and_live_template_records() {
    let (_, _, page, result) = run("<html><body><div id=root></div><template id=t><b>old</b></template></body></html>", r#"
var root=document.getElementById('root'), raw='\ud800', tail='\udfff', ns='http://www.w3.org/1999/xhtml';
var local='n'+raw, node=document.createElementNS(ns,'p:'+local);
node.setAttribute('data-mark',raw+'&"<>'+tail);
node.appendChild(document.createTextNode(raw+'&<>\u00a0'+tail));
node.appendChild(document.createComment(raw+'<>'));
root.appendChild(node);
var expected='<'+local+' data-mark="'+raw+'&amp;&quot;&lt;&gt;'+tail+'">'+raw+'&amp;&lt;&gt;&nbsp;'+tail+'<!--'+raw+'<>--></'+local+'>';
if(node.outerHTML!==expected||root.innerHTML!==expected||root.innerHTML.charCodeAt(2)!==0xd800)throw 'exact native markup';
var pair=document.createElement('div');
pair.appendChild(document.createTextNode('\ud83d'));pair.appendChild(document.createTextNode('\ude00'));
if(pair.innerHTML!=='\ud83d\ude00'||pair.innerHTML.length!==2)throw 'joined nodes repair pair';
var t=document.getElementById('t'),b=t.content.firstChild;
b.textContent=raw+'&';b.setAttribute('data-test',tail+'<');
if(t.innerHTML!=='<b data-test="'+tail+'&lt;">'+raw+'&amp;</b>')throw 'live template records';
var attrName='a'+raw;b.setAttributeNS('urn:mark','p:'+attrName,'v');
if(t.innerHTML!=='<b data-test="'+tail+'&lt;" p:'+attrName+'="v">'+raw+'&amp;</b>')throw 'expanded Attr name';
var plain=b.ownerDocument.createElement('i');plain.appendChild(b.ownerDocument.createTextNode(tail));b.appendChild(plain);
if(t.innerHTML!=='<b data-test="'+tail+'&lt;" p:'+attrName+'="v">'+raw+'&amp;<i>'+tail+'</i></b>')throw 'factory template descendants';
var blank=document.createElement('br');blank.appendChild(document.createTextNode('unserialized'));if(blank.innerHTML!==''||blank.outerHTML!=='<br>')throw 'void children';
var literal=document.createElement('style');literal.textContent=raw+'<>&';if(literal.innerHTML!==raw+'<>&')throw 'raw text context';
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
}

#[test]
fn markup_serialization_raw_size_and_copy_refusal_recovers_without_tree_changes() {
    let (mut doc, mut forms, mut page, initial) = run("<div id=n></div>",
        r"document.getElementById('n').textContent='a\ud800&';");
    assert_eq!(initial.errors, 0, "{}", page.report());
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n,"id")==Some("n")).unwrap();
    let children = doc.nodes[node].children.clone();
    let retained = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let expected = [97, 0xd800, 38, 97, 109, 112, 59];
    bridge.state.limits.host.string_bytes = expected.len() * 2 - 1;
    assert!(bridge.serialize_html(node, false).unwrap_err().contains("string limit"));
    bridge.state.limits.host.string_bytes += 1;
    let budget = bridge.state.copy_work;
    bridge.state.copy_work = 1;
    assert!(bridge.serialize_html(node, false).unwrap_err().contains("work limit"));
    assert_eq!(bridge.doc.nodes[node].children, children);
    assert_eq!(bridge.state.retained, retained);
    bridge.state.copy_work = budget;
    assert!(matches!(bridge.serialize_html(node, false), Ok(Value::StringUnits(u)) if u == expected));
}

#[test]
fn outer_html_serializes_the_element_and_its_current_tree() {
    let (_, _, page, outcome) = run("<html><body><input id=editor value=seed></body></html>", r#"
        var nav = document.createElement('nav'), copy = nav.cloneNode(true);
        if (!('outerHTML' in nav) || copy.outerHTML !== '<nav></nav>' || nav.innerHTML !== '')
            throw 'empty outer and inner';
        nav.setAttribute('title','A & B');
        nav.setAttributeNS('urn:example', 'm:word', '"');
        nav.appendChild(document.createComment('marker'));
        nav.appendChild(document.createTextNode('<雪>'));
        nav.appendChild(document.createElement('br'));
        var children = '<!--marker-->&lt;雪&gt;<br>';
        if (nav.outerHTML !== '<nav title="A &amp; B" m:word="&quot;">'+children+'</nav>' ||
            nav.innerHTML !== children || copy.outerHTML !== '<nav></nav>' ||
            'outerHTML' in nav.firstChild) throw 'detached serialization';
        document.body.appendChild(nav);
        var edit = document.getElementById('editor');
        edit.value = 'retained edit';
        if (edit.outerHTML !== '<input id="editor" value="seed">' || edit.value !== 'retained edit' ||
            nav.outerHTML !== nav.cloneNode(true).outerHTML) throw 'actual attributes and children';
    "#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn outer_html_accounts_for_root_bytes_and_preserves_failed_reads_and_writes() {
    let (mut doc, mut forms, mut page, initial) = run(
        "<html><body><div id=holder><span>read</span></div></body></html>", "");
    assert_eq!(initial.errors, 0);
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("holder")).unwrap();
    let child = doc.nodes[node].children[0];
    let count = doc.nodes.len();
    let retained = page.state.retained;
    let limit = page.state.limits.host.string_bytes;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.state.limits.host.string_bytes = "<span>read</span>".len();
    assert_eq!(bridge.serialize_html(node, false).unwrap().to_text(), "<span>read</span>");
    assert!(bridge.serialize_html(node, true).unwrap_err().contains("string limit"));
    bridge.state.limits.host.string_bytes = limit;
    assert!(bridge.dom_set(node_handle(node), "outerHTML", &Value::String("<em>new</em>".into()))
        .unwrap_err().contains("replacement"));
    assert_eq!(bridge.doc.nodes[node].children, [child]);
    assert_eq!(bridge.doc.nodes.len(), count);
    assert_eq!(bridge.state.retained, retained);
    assert_eq!(bridge.serialize_html(node, true).unwrap().to_text(), "<div id=\"holder\"><span>read</span></div>");
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
    let template = crate::html::parse_fragment("<b>x</b>", "template", true).unwrap();
    assert_eq!(template.nodes[0].children, [1]);
    assert!(matches!(&template.nodes[1].kind, NodeKind::Element { tag, .. } if tag == "b"));
    assert!(matches!(&template.nodes[template.nodes[1].children[0]].kind, NodeKind::Text(text) if text == "x"));
    for context in ["table", "tr", "select", "svg"] {
        assert!(crate::html::parse_fragment("<b>x</b>", context, true).is_err(), "{context}");
    }
    let (_,_,page,result)=run("<div id=root></div>",r#"
var root=document.getElementById('root');
root.innerHTML='<b><p>x</b>y';
if(root.innerHTML!=='<b></b><p><b>x</b>y</p>')throw 'adopted fragment block';
root.innerHTML='<p><b>x<div>y';
if(root.innerHTML!=='<p><b>x</b></p><div><b>y</b></div>')throw 'implied-close reconstruction';
"#);
    assert_eq!(result.errors,0,"{}",page.report());
    for source in ["<xmp><img src=x></xmp>","<plaintext><img src=x>","<picture><img src=x></picture>","<b><table><tr><td>held","a\0b"] {
        assert!(crate::html::parse_fragment(source, "div", true).is_err(), "{source}");
    }
    let recovered_p = crate::html::parse_fragment("</p>", "div", true).unwrap();
    assert_eq!(recovered_p.nodes[0].children, [1]);
    assert!(matches!(&recovered_p.nodes[1].kind, NodeKind::Element { tag, attrs, id: None, href: None, .. }
        if tag == "p" && attrs.is_empty()));
    assert!(recovered_p.nodes[1].children.is_empty());
    let recovered_br = crate::html::parse_fragment("a</br>b", "div", true).unwrap();
    assert_eq!(recovered_br.nodes[0].children, [1, 2, 3]);
    assert!(matches!(&recovered_br.nodes[1].kind, NodeKind::Text(text) if text == "a"));
    assert!(matches!(&recovered_br.nodes[2].kind, NodeKind::Element { tag, attrs, id: None, href: None, .. }
        if tag == "br" && attrs.is_empty()));
    assert!(recovered_br.nodes[2].children.is_empty());
    assert!(matches!(&recovered_br.nodes[3].kind, NodeKind::Text(text) if text == "b"));
    let headings = crate::html::parse_fragment("<h1>a<h2>b</h2>", "div", true).unwrap();
    assert_eq!(headings.nodes[0].children, [1, 3]);
    for (node, name, expected_text) in [(1, "h1", "a"), (3, "h2", "b")] {
        assert!(matches!(&headings.nodes[node].kind, NodeKind::Element { tag, .. } if tag == name));
        assert_eq!(headings.nodes[node].children, [node + 1]);
        assert!(matches!(&headings.nodes[node + 1].kind, NodeKind::Text(text) if text == expected_text));
    }
    crate::html::fragment_token_guard_tests::assert_supported_fragment_token_recovery();
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
  root.innerHTML='before<table id="leaves">\r\n &#9;<!--marker &amp; <img src=/unrequested>-->&#13;</table>after';
  table=root.firstChild.nextSibling;
  var leading=table.firstChild,marker=leading.nextSibling,trailing=marker.nextSibling;
  if(root.childNodes.length!==3||root.firstChild.textContent!=='before'||table.nextSibling.textContent!=='after'||
     table.nodeName!=='TABLE'||table.childNodes.length!==3||leading.nodeType!==3||leading.textContent!=='\n \t'||
     marker.nodeType!==8||marker.data!=='marker &amp; <img src=/unrequested>'||trailing.nodeType!==3||trailing.textContent!=='\r'||
     trailing.nextSibling!==null||leading.parentNode!==table||marker.parentNode!==table||trailing.parentNode!==table||
     leading.ownerDocument!==document||marker.ownerDocument!==document||trailing.ownerDocument!==document||
     table.isConnected!==root.isConnected)throw Error('table leaf ownership');
  root.innerHTML='<table><!--one--><!--two--></table><table>&#32;</table>';
  table=root.firstChild;
  if(root.childNodes.length!==2||table.childNodes.length!==2||table.firstChild.data!=='one'||table.lastChild.data!=='two'||
     table.nextSibling.childNodes.length!==1||table.nextSibling.firstChild.textContent!==' ')throw Error('table leaf replacement');
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
        "<table / ></table>", "<table>x</table>",
        "<table><b>text</b></table>", "<table><tr><td>x</td></tr></table>",
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

#[test]
fn fragment_comments_inner_html_template_import_clone_and_live_views_keep_real_mixed_nodes(){
    let(doc,forms,page,outcome)=run("<html><body><input id=editor value=seed><div id=mount></div></body></html>",r##"
var editor=document.getElementById('editor');editor.value='kept edit';
var target=document.createElement('div');target.innerHTML='before<!-- raw &amp; <img src="/never.ppm"> --><a href="#target" on-click="ordinary">word</a><!--end-->after';
var list=target.childNodes,view=target.children,comment=list[1];
if(list.length!==5||view.length!==1||comment.nodeType!==8||comment.data!==' raw &amp; <img src="/never.ppm"> '||comment.parentNode!==target||comment.ownerDocument!==document||target.textContent!=='beforewordafter')throw 'real parsed mixed children';
var cloned=target.cloneNode(true);if(cloned.childNodes[1]===comment||cloned.childNodes[1].data!==comment.data||cloned.children[0].getAttribute('on-click')!=='ordinary')throw 'real Comment clone';
var template=document.createElement('template');template.innerHTML=target.innerHTML;var source=template.content,sourceList=source.childNodes,sourceComment=sourceList[1];
if(sourceList.length!==5||source.children.length!==1||sourceComment.nodeType!==8||sourceComment.data!==comment.data||sourceComment.parentNode!==source||sourceComment.ownerDocument!==source.ownerDocument)throw 'real inert Comment';
var imported=document.importNode(source,true),donor=imported.childNodes,children=imported.children,copy=donor[1],link=children[0];
if(copy===sourceComment||copy.nodeType!==8||copy.data!==sourceComment.data||copy.ownerDocument!==document||copy.parentNode!==imported||copy.previousSibling!==donor[0])throw 'real Comment import';
document.getElementById('mount').appendChild(imported);
if(donor.length!==0||children.length!==0||copy.parentNode!==document.getElementById('mount')||!copy.isConnected||sourceComment.isConnected||sourceList[1]!==sourceComment||document.querySelectorAll('img').length!==0||editor.value!=='kept edit')throw 'mixed splice owns real nodes and editor';
document.title='real parsed Comments';
"##);
    assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"real parsed Comments");
    assert!(!page.state.images.has_changes());assert!(!doc.nodes.iter().any(|node|matches!(&node.kind,NodeKind::Element{tag,..}if matches!(tag.as_str(),"img"|"script"))));
    assert_eq!(forms.controls.len(),1);
}
#[test]
fn fragment_comments_failed_later_admission_keeps_tree_native_editor_and_resources_atomic(){
    let(mut doc,mut forms,mut page,outcome)=run("<html><body><input id=editor value=seed><div id=mount><span>old</span></div></body></html>","document.getElementById('editor').value='kept edit';");assert_eq!(outcome.errors,0);
    let mount=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("mount")).unwrap();let before=doc.nodes.len();let children=doc.nodes[mount].children.clone();let retained=page.state.retained;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for markup in ["<!--prefix--><p onclick='bad()'>no</p>","<!--prefix--><iframe></iframe>"]{
        assert!(bridge.set_inner_html(mount,markup).is_err(), "{markup}");assert_eq!(bridge.doc.nodes.len(),before);assert_eq!(bridge.doc.nodes[mount].children,children);assert_eq!(bridge.state.retained,retained);assert!(!bridge.state.images.has_changes());
    }
    bridge.state.retained_limit=retained;assert!(bridge.set_inner_html(mount,"<!--needs real storage-->").is_err());assert_eq!(bridge.doc.nodes.len(),before);assert_eq!(bridge.doc.nodes[mount].children,children);assert_eq!(bridge.state.retained,retained);
    drop(bridge);
    // Escaped script data is supported by the shared parser. Fragment scripts
    // remain inert even when their source and resource attributes later change.
    let (doc, forms, page, outcome) = run("<html><body><input id=editor value=seed><div id=mount></div></body></html>", r#"
var editor=document.getElementById('editor');editor.value='kept edit';
var mount=document.getElementById('mount');
mount.innerHTML='<!--prefix--><script><!--window.fragmentExecuted=true;--></script>';
var comment=mount.firstChild,script=mount.lastChild;
if(mount.childNodes.length!==2||comment.nodeType!==8||comment.data!=='prefix'||script.nodeName!=='SCRIPT'||script.childNodes.length!==1||script.firstChild.nodeType!==3||script.textContent!=='<!--window.fragmentExecuted=true;-->')throw 'real escaped script data';
script.src='/never-fetch.js';script.text='window.fragmentExecuted=true;';
document.body.appendChild(script);
if(window.fragmentExecuted!==undefined||editor.value!=='kept edit')throw 'fragment script inertness and native editor';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(forms.controls.values().next().unwrap().value.text(),"kept edit");
    let script=(0..doc.nodes.len()).find(|&node|tag(&doc,node)=="script").unwrap();
    assert!(page.state.dom.fragment_scripts.contains(&script));
    assert!(page.state.dynamic.pending.is_empty());
    assert!(!page.state.images.has_changes());
}

#[test]
fn fragment_comments_native_select_markers_do_not_become_option_values_or_break_editors(){
    let(doc,forms,page,outcome)=run("<html><body><form id=f><input id=editor value=seed><div id=mount></div></form></body></html>",r#"
var editor=document.getElementById('editor');editor.value='kept edit';var mount=document.getElementById('mount');
mount.innerHTML='<!--before--><select id="choice" name="choice"><!--inside--><option>one<!--not label-->two</option><!--after--></select>';
var select=document.getElementById('choice'),option=select.children[0];
if(select.childElementCount!==1||select.value!=='onetwo'||select.selectedIndex!==0||option.textContent!=='onetwo'||select.childNodes.length!==3||select.firstChild.nodeType!==8||option.childNodes.length!==3||option.childNodes[1].data!=='not label'||editor.value!=='kept edit')throw 'real native select mixed nodes';
document.title='native select comment markers';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"native select comment markers");assert_eq!(forms.controls.len(),2);assert!(!page.state.images.has_changes());
    let choice=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("choice")).unwrap();let options=&forms.controls[&choice].select.as_ref().unwrap().options;
    assert_eq!(options.len(),1);assert_eq!(options[0].label,"onetwo");assert_eq!(options[0].value,"onetwo");
}
#[test]
fn fragment_formatting_inner_html_uses_native_order_independent_attributes_and_template_owner(){
    let (_,_,page,out)=run("<html><body><div id=root></div><template id=t></template></body></html>",r##"
var root=document.getElementById('root'),tpl=document.getElementById('t'),calls=0;
var observer=new MutationObserver(function(rs){if(rs.length!==1||rs[0].target!==root||rs[0].addedNodes.length!==2||rs[0].addedNodes[1]!==root.lastChild)throw 'fragment record';calls++;});
observer.observe(root,{childList:true});
root.innerHTML='<a href="#x">1<p>2</a>3</p>';
if(root.innerHTML!=='<a href="#x">1</a><p><a href="#x">2</a>3</p>')throw 'fragment order '+root.innerHTML;
var first=root.firstChild,copy=root.lastChild.firstChild;if(first===copy||copy.ownerDocument!==document||copy.parentNode!==root.lastChild)throw 'fragment identity';
copy.setAttribute('data-copy','yes');if(first.hasAttribute('data-copy'))throw 'aliased attribute';
tpl.innerHTML='<b>one<div>two</b>three</div>';
var t=tpl.content,sub=t.lastChild.firstChild;if(t.firstChild===sub||sub.ownerDocument!==t.ownerDocument||sub.textContent!=='two'||t.lastChild.lastChild.data!=='three')throw 'template fragment identity';
if(tpl.innerHTML!=='<b>one</b><div><b>two</b>three</div>')throw 'template serialization';
Promise.resolve().then(function(){if(calls!==1)throw 'missing mutation delivery';});
"##);
    assert_eq!(out.errors,0,"{}",page.report());
}

#[test]
fn fragment_formatting_adopted_depth_refuses_before_publication_and_allows_recovery(){
    let source=format!("{}<div id=root><i>old</i></div>{}","<div>".repeat(122),"</div>".repeat(122));
    let(mut doc,mut forms,mut page,out)=run(&source,"");
    assert_eq!(out.errors,0,"{}",page.report());
    let root=(0..doc.nodes.len()).find(|&i|doc.attr(i,"id")==Some("root")).unwrap();
    let original=doc.nodes[root].children.clone();let count=doc.nodes.len();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let error=bridge.set_inner_html(root,"<a>prefix<p><span><b>deep</b></span></a></p>").unwrap_err();
    assert!(error.contains("depth limit"),"{error}");
    assert_eq!(bridge.doc.nodes.len(),count);assert_eq!(bridge.doc.nodes[root].children,original);
    assert_eq!(bridge.state.tree.parents[original[0]],Some(root));
    bridge.set_inner_html(root,"<i>ok</i>").unwrap();
    assert_ne!(bridge.doc.nodes[root].children,original);
}

#[test]
fn inner_html_list_starts_preserve_connected_and_detached_parent_and_owner_links() {
    let (_, _, page, outcome) = run("<div id=root></div>", r#"
function verify(target) {
  target.innerHTML='<dl><dt>A<div>B<dd>C';
  var list=target.firstChild,old=list.firstChild,next=old.nextSibling,wrapper=old.lastChild;
  if(target.innerHTML!=='<dl><dt>A<div>B</div></dt><dd>C</dd></dl>'||list.childNodes.length!==2||
     old.parentNode!==list||next.parentNode!==list||wrapper.parentNode!==old||next.ownerDocument!==document||
     wrapper.ownerDocument!==document||next.isConnected!==target.isConnected)throw Error('definition sibling ownership');
  target.innerHTML='<dl><dt>A<section>B<dd>C';
  list=target.firstChild;old=list.firstChild;wrapper=old.lastChild;next=wrapper.lastChild;
  if(target.innerHTML!=='<dl><dt>A<section>B<dd>C</dd></section></dt></dl>'||list.childNodes.length!==1||
     next.parentNode!==wrapper||next.ownerDocument!==document||wrapper.parentNode!==old)throw Error('definition special barrier');
  target.innerHTML='<ul><li>A<section><p>B<li>C';
  list=target.firstChild;old=list.firstChild;wrapper=old.lastChild;next=wrapper.lastChild;
  if(target.innerHTML!=='<ul><li>A<section><p>B</p><li>C</li></section></li></ul>'||
     list.childNodes.length!==1||next.parentNode!==wrapper||next.ownerDocument!==document)throw Error('list item special barrier');
  target.innerHTML='<dl><dt><b>A<dd>B</b><dt>C';
  if(target.innerHTML!=='<dl><dt><b>A</b></dt><dd><b>B</b></dd><dt>C</dt></dl>')throw Error('definition formatting');
}
verify(document.createElement('div'));verify(document.getElementById('root'));
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn inner_html_scoped_ends_preserve_connected_and_detached_tree_ownership() {
    let (_, _, page, outcome) = run("<div id=root></div>", r#"
function verify(target) {
  target.innerHTML='<ul><li>A<ul>B</li>C</ul>D';
  var list=target.firstChild,item=list.firstChild,nested=item.childNodes[1];
  if(target.innerHTML!=='<ul><li>A<ul>BC</ul>D</li></ul>'||nested.parentNode!==item||
     item.parentNode!==list||nested.firstChild.data!=='BC'||item.lastChild.data!=='D'||
     item.ownerDocument!==document||item.isConnected!==target.isConnected)throw Error('list scope ownership');
  target.innerHTML='<dl><dd>A<section><dt>B</dd>C';
  list=target.firstChild;item=list.firstChild;var section=item.lastChild,term=section.firstChild;
  if(target.innerHTML!=='<dl><dd>A<section><dt>B</dt></section></dd>C</dl>'||
     list.lastChild.data!=='C'||section.parentNode!==item||term.parentNode!==section||
     term.ownerDocument!==document)throw Error('exact definition end ownership');
  target.innerHTML='<h1>A<span><h4>B</h1>C';
  var outer=target.firstChild,span=outer.lastChild,nearest=span.firstChild;
  if(target.innerHTML!=='<h1>A<span><h4>B</h4>C</span></h1>'||
     nearest.parentNode!==span||span.parentNode!==outer||span.lastChild.data!=='C'||
     nearest.ownerDocument!==document)throw Error('nearest heading end ownership');
  target.innerHTML='<h2><b>A</h6>B</b>C';
  var first=target.firstChild.firstChild,copy=target.childNodes[1];
  if(target.innerHTML!=='<h2><b>A</b></h2><b>B</b>C'||first===copy||
     copy.parentNode!==target||copy.ownerDocument!==document)throw Error('end formatting reconstruction');
}
verify(document.createElement('div'));verify(document.getElementById('root'));
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn inner_html_ordinary_end_scope_preserves_connected_and_detached_owner_links() {
    let (mut doc, mut forms, mut page, outcome) = run("<div id=root></div>", r#"
function verify(target) {
  target.innerHTML='<section>A<marquee>B</section>C</marquee>D';
  var section=target.firstChild,boundary=section.childNodes[1];
  if(target.innerHTML!=='<section>A<marquee>BC</marquee>D</section>'||
     target.childNodes.length!==1||boundary.parentNode!==section||boundary.firstChild.data!=='BC'||
     section.lastChild.data!=='D'||section.ownerDocument!==document||
     section.isConnected!==target.isConnected)throw Error('ordinary scope ownership');
  target.innerHTML='<article>A<ul><li>B</article>C';
  var article=target.firstChild,list=article.lastChild,item=list.firstChild;
  if(target.innerHTML!=='<article>A<ul><li>B</li></ul></article>C'||
     item.parentNode!==list||list.parentNode!==article||target.lastChild.data!=='C'||
     item.ownerDocument!==document)throw Error('ordinary list traversal');
  target.innerHTML='<select><optgroup><option>A</select>B';
  var select=target.firstChild,group=select.firstChild,option=group.firstChild;
  if(target.innerHTML!=='<select><optgroup><option>A</option></optgroup></select>B'||
     option.parentNode!==group||group.parentNode!==select||target.lastChild.data!=='B'||
     option.ownerDocument!==document||select.value!=='A')throw Error('select target before scope boundary');
  target.innerHTML='<section><b>A</section>B</b>C';
  var first=target.firstChild.firstChild,copy=target.childNodes[1];
  if(target.innerHTML!=='<section><b>A</b></section><b>B</b>C'||first===copy||
     copy.parentNode!==target||copy.ownerDocument!==document)throw Error('ordinary end formatting');

}
var detached=document.createElement('div'),connected=document.getElementById('root');verify(detached);verify(connected);
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    // Unsupported resources use the existing native refusal boundary. Check
    // that boundary directly rather than expecting a catchable JS exception.
    for target in ["detached", "connected"] {
        page.begin_job();
        page.runtime.execute(&format!("var checked={target},retained=checked.firstChild,before=checked.innerHTML;"),
            &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
        for markup in ["<section><object>A</section>", "<button>A</button>"] {
            page.begin_job();let tree=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
            let error=page.runtime.execute(&format!("checked.innerHTML='{markup}';"),
                &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
            assert!(error.contains("unsupported resources")||error.contains("native controls/forms"),"{error}");
            assert_eq!(format!("{:?}",doc.nodes),tree);assert_eq!(page.state.dom_commits,commits);
            page.begin_job();page.runtime.execute("if(checked.firstChild!==retained||checked.innerHTML!==before)throw 'fragment refusal must preserve actual nodes';",
                &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
        }
    }
}

#[test]
fn inner_html_generic_end_barriers_preserve_connected_and_detached_owner_links() {
    let (mut doc, mut forms, mut page, outcome) = run("<div id=root></div>", r#"
function verify(target) {
  target.innerHTML='<span>A<section>B</span>C</section>D</span>E';
  var span=target.firstChild,section=span.childNodes[1];
  if(target.innerHTML!=='<span>A<section>BC</section>D</span>E'||
     target.childNodes.length!==2||section.parentNode!==span||section.firstChild.data!=='BC'||
     span.lastChild.data!=='D'||target.lastChild.data!=='E'||section.ownerDocument!==document||
     section.isConnected!==target.isConnected)throw Error('generic barrier ownership');
  target.innerHTML='<label>A<dialog>B</label>C';
  var label=target.firstChild,dialog=label.lastChild;
  if(target.innerHTML!=='<label>A<dialog>B</dialog></label>C'||dialog.parentNode!==label||
     target.lastChild.data!=='C'||dialog.ownerDocument!==document)throw Error('generic non-special traversal');
  target.innerHTML='<ruby><rt>A<section>B</rt>C</section>D</rt>E</ruby>F';
  var ruby=target.firstChild,rt=ruby.firstChild;
  if(target.innerHTML!=='<ruby><rt>A<section>BC</section>D</rt>E</ruby>F'||
     rt.childNodes[1].parentNode!==rt||rt.childNodes[1].firstChild.data!=='BC'||
     ruby.lastChild.data!=='E'||target.lastChild.data!=='F'||rt.ownerDocument!==document)throw Error('generic ruby target');
  target.innerHTML='<span><b data-note=kept>A<section>B</span>C</section>D</span>E';
  var first=target.firstChild.firstChild,copy=target.childNodes[1];
  if(target.innerHTML!=='<span><b data-note="kept">A<section>BC</section>D</b></span><b data-note="kept">E</b>'||
     first===copy||copy.parentNode!==target||copy.getAttribute('data-note')!=='kept'||
     copy.ownerDocument!==document)throw Error('generic formatting identity');
}
var detached=document.createElement('div'),connected=document.getElementById('root');
verify(detached);verify(connected);
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    // Unsupported fragment recovery returns a native error. Check each
    // refusal independently so both target identities remain observable.
    for target in ["detached", "connected"] {
        page.begin_job();
        page.runtime.execute(&format!("var checked={target},retained=checked.firstChild,before=checked.innerHTML;"),
            &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
        for (markup, expected) in [
            ("<select><option><span>A</span></option></select>", "HTML fragment nonconventional select recovery is not implemented."),
            ("<span>A<section>B</span>C</section>D<table><tr></tr></table>", "HTML fragment tables require ordinary flow, supported non-element content and an immediately matching end tag."),
        ] {
            page.begin_job();
            let tree=format!("{:?}",doc.nodes);
            let commits=page.state.dom_commits;
            let error=page.runtime.execute(&format!("checked.innerHTML='{markup}';"),
                &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert_eq!(format!("{:?}",doc.nodes),tree);
            assert_eq!(page.state.dom_commits,commits);
            page.begin_job();
            page.runtime.execute("if(checked.firstChild!==retained||checked.innerHTML!==before)throw 'fragment refusal must preserve actual nodes';",
                &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
        }
    }
}

#[test]
fn inner_html_ruby_annotation_starts_preserve_connected_and_detached_owner_links() {
    let (_, _, page, outcome) = run("<div id=root></div>", r#"
function verify(target) {
  target.innerHTML='<ruby><rb>A<rt>B<rp>C</ruby>';
  var ruby=target.firstChild,base=ruby.firstChild,annotation=base.nextSibling,fallback=annotation.nextSibling;
  if(target.innerHTML!=='<ruby><rb>A</rb><rt>B</rt><rp>C</rp></ruby>'||ruby.childNodes.length!==3||
     base.parentNode!==ruby||annotation.parentNode!==ruby||fallback.parentNode!==ruby||
     annotation.ownerDocument!==document||annotation.isConnected!==target.isConnected||
     base.firstChild.data!=='A'||annotation.firstChild.data!=='B'||fallback.firstChild.data!=='C')throw Error('ruby annotation sibling ownership');
  target.innerHTML='<ruby><rtc><rt>A<rp>B<rtc>C';
  var next=target.firstChild,firstGroup=next.firstChild,secondGroup=firstGroup.nextSibling;
  if(target.innerHTML!=='<ruby><rtc><rt>A</rt><rp>B</rp></rtc><rtc>C</rtc></ruby>'||
     firstGroup.childNodes.length!==2||secondGroup.parentNode!==next||secondGroup.ownerDocument!==document||
     ruby.parentNode!==null||ruby.isConnected||base.parentNode!==ruby||base.ownerDocument!==document)throw Error('ruby rtc ownership and retained old identities');
  target.innerHTML='<ruby><rt>A<span>B<rt>C</ruby>';
  ruby=target.firstChild;annotation=ruby.firstChild;var wrapper=annotation.lastChild,child=wrapper.lastChild;
  if(target.innerHTML!=='<ruby><rt>A<span>B<rt>C</rt></span></rt></ruby>'||
     child.parentNode!==wrapper||wrapper.parentNode!==annotation||child.ownerDocument!==document)throw Error('ruby current node barrier');
  target.innerHTML='<ruby><b><p>A<rt></rt>C';
  ruby=target.firstChild;var bold=ruby.firstChild,paragraph=bold.firstChild,empty=paragraph.nextSibling;
  if(target.innerHTML!=='<ruby><b><p>A</p><rt></rt>C</b></ruby>'||
     bold.childNodes.length!==3||empty.firstChild!==null||empty.parentNode!==bold||
     bold.lastChild.data!=='C'||empty.ownerDocument!==document)throw Error('ruby formatting identity');
}
verify(document.createElement('div'));verify(document.getElementById('root'));
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}
