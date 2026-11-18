//! Existing child boundaries and bounded selector failure publication.
use super::*;

fn location(source:&str)->crate::navigation::Location {
    crate::navigation::Location {url:Some(crate::url::parse(source).unwrap().0),path:None,root:None,fragment:String::new()}
}
fn fixture()->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<!doctype html><title>Selector security</title><body><div id=root><span id=a class=item>A</span><span id=b class=item>B</span></div><form id=f><input id=neighbor name=keep value=original></form>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&location("https://element-selector.test/security"));
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    (doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{source}\n{}",page.report());
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}

#[test]
fn element_selector_prototype_child_owner_borrowing_keeps_explicit_query_boundary_before_conversion() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),one=Element.prototype.querySelector,all=Element.prototype.querySelectorAll;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,childRoot=w.document.documentElement,childOne=w.Element.prototype.querySelector,childAll=w.Element.prototype.querySelectorAll;
var read=w.Function('node','selector','return node.querySelector(selector);'),borrow=w.Function('fn','node','selector','return fn.call(node,selector);'),own=w.Function('return window===self&&document.body.nodeType===1;');
if(childOne===one||childAll===all||childOne.call(root,'.item')!==a||childAll.call(root,'.item').length!==2||read(root,'.item')!==a||borrow(one,root,'.item')!==a||!own())throw 'same-origin native function owner/receiver';
var touched=0,request={toString:function(){touched++;return 'body';}},refused=0;
for(var fn of [one,all,childOne,childAll]){
 try{fn.call(childRoot,request);throw 'unsupported child query fabricated';}catch(error){if(error.name!=='NotSupportedError')throw error;refused++;}
 try{fn.call(w.document,request);throw 'child Document accepted as Element';}catch(error){if(error.name!=='TypeError')throw error;refused++;}
}
if(touched||refused!==8||one.call(root,'.item')!==a||!own())throw 'child boundary coercion or native owner recovery';
"#);
    page.set_location(&location("https://different-selector.test/changed"));
    for source in ["one.call(childRoot,request);","all.call(childRoot,request);","childOne.call(childRoot,request);","read(root,request);","borrow(one,root,request);"] {
        page.clear_console();let allocations=page.runtime.window_allocation_state_for_test();
        let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}\n{}",page.report());
        assert!(page.report().contains("same-origin"),"{source}\n{}",page.report());
        assert_eq!(page.runtime.window_allocation_state_for_test(),allocations,"{source}");
        run(&mut doc,&mut forms,&mut page,"if(touched||one.call(root,'.item')!==a||!own())throw 'origin refusal converted selector or damaged later owner context';");
    }
}

#[test]
fn element_selector_prototype_snapshot_quota_refusal_does_not_publish_partial_lists_or_reset_editor() {
    let(mut doc,mut forms,mut page)=fixture();
    let neighbor=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("neighbor")).unwrap();
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let e=&mut forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
    let c=&forms.controls[&neighbor];let editor=(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner);
    run(&mut doc,&mut forms,&mut page,"var root=document.getElementById('root'),a=document.getElementById('a'),all=Element.prototype.querySelectorAll,one=Element.prototype.querySelector,kept=all.call(root,'.item');if(kept.length!==2||kept[0]!==a||one.call(root,'.item')!==a)throw 'selector quota prefix';");
    let snapshot=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let count=page.state.node_lists.len();let references=page.state.node_list_references;
    let list_limit=page.state.limits.host.node_lists;let reference_limit=page.state.limits.host.node_list_references;
    for reference_cap in [false,true] {
        if reference_cap {page.state.limits.host.node_list_references=references;}
        else {page.state.limits.host.node_lists=count;}
        page.clear_console();let out=page.console_eval(&mut doc,&mut forms,"all.call(root,'.item');");
        assert_eq!(out.errors,1,"{}",page.report());assert!(page.report().contains("NodeList"),"{}",page.report());
        assert_eq!(page.state.node_lists.len(),count);assert_eq!(page.state.node_list_references,references);
        assert_eq!(format!("{:?}",doc.nodes),snapshot);assert_eq!(page.state.dom_commits,commits);
        let c=&forms.controls[&neighbor];assert_eq!((c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner),editor);
        assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
        page.state.limits.host.node_lists=list_limit;page.state.limits.host.node_list_references=reference_limit;
        run(&mut doc,&mut forms,&mut page,"if(kept.length!==2||kept[0]!==a||one.call(root,'.item')!==a||document.getElementById('neighbor').value!=='edited neighbor')throw 'quota refusal damaged saved selector/list/native state';");
    }
    run(&mut doc,&mut forms,&mut page,"if(all.call(root,'.item').length!==2)throw 'selector snapshot recovery';");
}

#[test]
fn element_selector_prototype_host_work_failure_stays_fatal_and_clears_native_call_context() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var root=document.getElementById('root'),a=document.getElementById('a'),one=Element.prototype.querySelector,all=Element.prototype.querySelectorAll,caught=0,finalized=0,after=0;if(one.call(root,'.item')!==a)throw 'selector work prefix';");
    let snapshot=format!("{:?}",doc.nodes);let lists=page.state.node_lists.len();let references=page.state.node_list_references;
    let allowance=page.state.limits.host_work;page.state.limits.host_work=0;page.clear_console();
    let out=page.console_eval(&mut doc,&mut forms,"try{all.call(root,'.item');after++;}catch(error){caught++;}finally{finalized++;}after++;");
    assert_eq!(out.errors,1,"{}",page.report());assert!(page.runtime.invocation_failed_fatally());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);assert_eq!(page.state.node_lists.len(),lists);assert_eq!(page.state.node_list_references,references);
    page.state.limits.host_work=allowance;
    run(&mut doc,&mut forms,&mut page,"if(caught||finalized||after||one.call(root,'.item')!==a||all.call(root,'.item').length!==2||window!==self)throw 'fatal selector work or next-job native context';");
}
