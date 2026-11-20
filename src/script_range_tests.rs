//! Original connected live-range cases over real DOM transactions.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse_loaded_document("<html><body><div id='root'><b id='a'>A😀B</b><i id='b'>CD</i><em id='c'>EF</em></div><p id='neighbor'>keep</p></body></html>",&||Ok(())).unwrap();
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    let location = crate::navigation::address(
        "https://range.test/start",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    run(&mut doc,&mut forms,&mut page,"var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');");
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc, forms, source);
    assert_eq!(result.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn dom_range_canonical_constructors_native_boundaries_utf16_and_clones() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var r=new Range(),s=document.createRange();
if(r===s||!(r instanceof Range)||!(r instanceof NodeRange)||!(r instanceof AbstractRange)||Object.getPrototypeOf(r)!==Range.prototype||Object.getPrototypeOf(Range)!==NodeRange||Object.getPrototypeOf(NodeRange)!==AbstractRange||r.constructor!==Range||Object.prototype.toString.call(r)!=='[object Range]')throw 'canonical range identity';
if(r.startContainer!==document||r.endContainer!==document||r.startOffset||r.endOffset||!r.collapsed||r.commonAncestorContainer!==document)throw 'initial document boundary';
var get=Object.getOwnPropertyDescriptor(NodeRange.prototype,'startContainer').get;
for(var fake of [{},document,a]){var caught=false;try{get.call(fake);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'getter brand';}
for(var C of [AbstractRange,NodeRange]){var caught=false;try{new C();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'illegal constructor';}
r.selectNodeContents(a);if(r.startContainer!==a||r.endOffset!==1||r.collapsed)throw 'child boundary';
r.setStart(a.firstChild,1);r.setEnd(a.firstChild,3);if(r.startOffset!==1||r.endOffset!==3||r.commonAncestorContainer!==a.firstChild)throw 'UTF16 offsets';
if(r.toString()!=='😀')throw 'UTF16 range string';r.setEnd(a.firstChild,2);if(r.toString()!=='\uD83D')throw 'exact unpaired UTF16 slice';r.setEnd(a.firstChild,3);
s=r.cloneRange();r.collapse();if(!r.collapsed||r.startOffset!==3||s.collapsed||s.startOffset!==1)throw 'independent clone and collapse';
r.setStartAfter(c);r.setEndBefore(a);if(!r.collapsed||r.startContainer!==root||r.startOffset!==0)throw 'crossed boundaries collapse';
r.selectNode(b);if(r.startContainer!==root||r.startOffset!==1||r.endOffset!==2)throw 'node selection';
var saved=Range;delete window.Range;if(!(r instanceof saved)||new saved().startContainer!==document)throw 'canonical saved constructor';
"#,
    );
}
#[test]
fn dom_range_live_insert_remove_move_replace_and_fragment_splice() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var r=new Range(),inside=new Range();r.setStart(root,1);r.setEnd(root,3);inside.selectNodeContents(b.firstChild);
var text=document.createTextNode('new');root.insertBefore(text,b);
if(r.startOffset!==1||r.endOffset!==4)throw 'strict insertion offset';
root.removeChild(b);if(inside.startContainer!==root||inside.startOffset!==2||!inside.collapsed||r.endOffset!==3)throw 'removed subtree retarget';
root.appendChild(a);if(r.startOffset!==0||r.endOffset!==2)throw 'same parent move order';
var into=new Range();into.selectNodeContents(c.firstChild);var replacement=document.createElement('span');root.replaceChild(replacement,c);
if(into.startContainer!==root||into.startOffset!==1||!into.collapsed||r.endOffset!==1)throw 'replacement retarget';
var fragment=document.createDocumentFragment(),x=document.createTextNode('X'),y=document.createTextNode('Y');fragment.appendChild(x);fragment.appendChild(y);var donor=new Range();donor.selectNodeContents(x);
root.insertBefore(fragment,replacement);if(donor.startContainer!==fragment||donor.startOffset!==0||!donor.collapsed||r.endOffset!==1)throw 'fragment splice live boundaries';
"#,
    );
}
#[test]
fn dom_range_live_character_data_inner_html_text_content_and_normalize() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var r=new Range(),old=a.firstChild;r.setStart(old,1);r.setEnd(old,4);old.data='changed';if(r.startOffset||r.endOffset||r.startContainer!==old)throw 'character replacement';
r.selectNodeContents(b.firstChild);b.textContent='new';if(r.startContainer!==b||r.startOffset||!r.collapsed)throw 'textContent subtree retirement';
r.selectNodeContents(c.firstChild);root.innerHTML='<span>replacement</span>';if(r.startContainer!==root||r.startOffset||!r.collapsed)throw 'innerHTML subtree retirement';
var parent=document.createElement('div'),empty=document.createTextNode(''),first=document.createTextNode('😀'),second=document.createTextNode('AB'),tail=document.createTextNode('');parent.appendChild(empty);parent.appendChild(first);parent.appendChild(second);parent.appendChild(tail);document.body.appendChild(parent);
var merged=new Range(),between=new Range(),removed=new Range();merged.setStart(second,1);merged.setEnd(second,2);between.setStart(parent,2);between.collapse(true);removed.selectNodeContents(empty);
parent.normalize();if(parent.childNodes.length!==1||parent.firstChild!==first||first.data!=='😀AB'||merged.startContainer!==first||merged.startOffset!==3||merged.endOffset!==4||between.startContainer!==first||between.startOffset!==2||removed.startContainer!==parent||removed.startOffset!==0)throw 'normalization boundaries';
"#,
    );
}
#[test]
fn dom_range_failed_mutation_and_quota_preserve_boundaries_and_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var r=new Range();r.selectNodeContents(a.firstChild);var savedStart=r.startContainer,savedEnd=r.endOffset;");
    let limit = page.state.limits.host_work;
    page.state.limits.host_work = 0;
    let result = page.console_eval(&mut doc, &mut forms, "root.removeChild(a);");
    assert_eq!(result.errors, 1);
    page.state.limits.host_work = limit;
    run(&mut doc,&mut forms,&mut page,"if(a.parentNode!==root||r.startContainer!==savedStart||r.endOffset!==savedEnd)throw 'failed mutation changed range';");
    let retained = page.state.retained;
    let old_limit = page.state.retained_limit;
    page.state.retained_limit = retained;
    // Fill already paid spare slots, then require actual backing growth.
    while page.state.ranges.len() < page.state.ranges.capacity() {
        let record = page.state.ranges[0];
        page.state.ranges.push(record);
    }
    let count = page.state.ranges.len();
    let result = page.console_eval(&mut doc, &mut forms, "new Range();");
    assert_eq!(result.errors, 1, "{}", page.report());
    assert_eq!(page.state.ranges.len(), count);
    assert_eq!(page.state.retained, retained);
    page.state.retained_limit = old_limit;
    run(&mut doc,&mut forms,&mut page,"var after=new Range();root.removeChild(a);if(r.startContainer!==root||r.startOffset!==0||!r.collapsed)throw 'recovered mutation';");
}

#[test]
fn dom_range_conversion_order_dom_errors_and_real_receiver_refusals() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var r=new Range(),offsetCalls=0;
try{r.setStart({}, {valueOf:function(){offsetCalls++;return 0;}});throw 'missing bad Node';}catch(e){if(!(e instanceof TypeError))throw e;}
if(offsetCalls)throw 'node conversion must precede numeric conversion';
r.selectNodeContents(a.firstChild);
try{r.setEnd(a.firstChild,99);throw 'missing offset refusal';}catch(e){if(!(e instanceof DOMException)||e.name!=='IndexSizeError')throw e;}
if(r.endOffset!==4)throw 'offset error mutated range';
r.setStart(a.firstChild,{valueOf:function(){r.setEnd(a.firstChild,2);return 1;}});
if(r.startOffset!==1||r.endOffset!==2)throw 'reentrant conversion clobbered native state';
r.setStart(a.firstChild,4294967296);if(r.startOffset!==0)throw 'unsigned long conversion';
var descriptor=Object.getOwnPropertyDescriptor(AbstractRange.prototype,'startOffset');
Object.defineProperty(r,'startOffset',{value:37,configurable:true});if(r.startOffset!==37||descriptor.get.call(r)!==0)throw 'ordinary property versus native slot';delete r.startOffset;
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow,read=w.Function('value','return value.startOffset;');
if(read(r)!==0||w.Range===Range||r instanceof w.Range)throw 'real owner prototypes';
try{new w.Range();throw 'missing unsupported child boundary';}catch(e){if(!(e instanceof w.DOMException)||e.name!=='NotSupportedError')throw e;}
"#,
    );
    let other = crate::navigation::address(
        "https://other-range.test/changed",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&other);
    let result = page.console_eval(&mut doc, &mut forms, "read(r);");
    assert_eq!(result.errors, 1, "{}", page.report());
    assert!(page.report().contains("origin"), "{}", page.report());
}

#[test]
fn dom_range_promoted_inert_fragment_updates_real_destination_offsets() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var r=new Range();r.setStart(root,1);r.setEnd(root,3);
var t=document.createElement('template');t.innerHTML='<span>promoted</span>';var fragment=document.importNode(t.content,true),child=fragment.firstChild;
root.insertBefore(fragment,b);if(r.startOffset!==1||r.endOffset!==4||child.parentNode!==root||fragment.childNodes.length!==0)throw 'promoted insertion offsets';
var selected=new Range();selected.selectNodeContents(child.firstChild);if(selected.toString()!=='promoted')throw 'actual promoted native text';
"#,
    );
}
#[test]
fn dom_range_update_work_refusal_and_backing_copy_refusal_are_atomic() {
    let (mut baseline_doc, mut baseline_forms, mut baseline) = fixture();
    baseline.begin_job();
    let root_node = baseline_doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="root"))
        .unwrap();
    let a_node = baseline_doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="a"))
        .unwrap();
    let before = baseline.state.host_work;
    Bridge {
        doc: &mut baseline_doc,
        forms: &mut baseline_forms,
        state: &mut baseline.state,
    }
    .call(
        node_handle(root_node),
        "removeChild",
        &[Value::Host(node_handle(a_node))],
    )
    .unwrap();
    let mutation_work = before - baseline.state.host_work;
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var r=new Range();r.selectNodeContents(a.firstChild);",
    );
    page.begin_job();
    page.state.host_work = mutation_work;
    let ranges = page
        .state
        .ranges
        .iter()
        .map(|range| (range.start, range.end))
        .collect::<Vec<_>>();
    let error = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .call(
        node_handle(root_node),
        "removeChild",
        &[Value::Host(node_handle(a_node))],
    )
    .unwrap_err();
    assert!(error.contains("work limit"), "{error}");
    assert_eq!(
        page.state
            .ranges
            .iter()
            .map(|range| (range.start, range.end))
            .collect::<Vec<_>>(),
        ranges
    );
    assert!(doc.nodes[root_node].children.contains(&a_node));
    page.begin_job();
    while page.state.ranges.len() < page.state.ranges.capacity() {
        let record = page.state.ranges[0];
        page.state.ranges.push(record);
    }
    let before = (
        page.state.ranges.len(),
        page.state.ranges.capacity(),
        page.state.retained,
    );
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .create_range(WINDOW, DOCUMENT)
    .is_err());
    assert_eq!(
        (
            page.state.ranges.len(),
            page.state.ranges.capacity(),
            page.state.retained
        ),
        before
    );
    run(&mut doc,&mut forms,&mut page,"var recovered=new Range();root.removeChild(a);if(!r.collapsed||r.startContainer!==root||r.startOffset!==0)throw 'live update recovery';");
}
