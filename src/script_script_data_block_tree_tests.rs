//! Script data blocks retain exact DOM text without acquiring execution authority.
use super::*;

#[test]
fn script_data_block_tree_conversion_reentry_observer_refusal_and_empty_replacement_recover(){
    let(mut d,mut f,mut p)=fixture();
    run(&mut d,&mut f,&mut p,r#"var data=document.createElement('script');data.type='application/json';data.text='old';document.getElementById('target').appendChild(data);var old=data.firstChild,during,calls=0,watch=new MutationObserver(function(){});watch.observe(data,{childList:true});data.text={toString:function(){calls++;data.text='during\ud800';during=data.firstChild;return 'after\udfff';}};var after=data.firstChild,r=watch.takeRecords();if(calls!==1||data.text!=='after\udfff'||r.length!==2||r[0].removedNodes[0]!==old||r[0].addedNodes[0]!==during||r[1].removedNodes[0]!==during||r[1].addedNodes[0]!==after||during.data!=='during\ud800')throw 'script conversion before observer preflight';data.text=undefined;r=watch.takeRecords();if(data.text!=='undefined'||r.length!==1||r[0].removedNodes[0]!==after||r[0].addedNodes[0]!==data.firstChild)throw 'undefined script binding observation';old=data.firstChild;data.text=null;r=watch.takeRecords();if(data.text!=='null'||data.firstChild.data!=='null'||r.length!==1||r[0].removedNodes[0]!==old||r[0].addedNodes[0]!==data.firstChild)throw 'nonnullable text binding';old=data.firstChild;data.textContent=null;r=watch.takeRecords();if(data.firstChild!==null||data.text!==''||r.length!==1||r[0].removedNodes[0]!==old||r[0].addedNodes.length)throw 'nullable textContent replacement observation';data.text='stable\ud800';old=data.firstChild;watch.takeRecords();"#);
    let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let limit=p.state.retained_limit;
    p.begin_job();p.state.retained_limit=p.state.retained;
    let error=p.runtime.execute(r#"data.text='blocked\udfff';"#,&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
    assert!(error.contains("allowance"),"{error}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);
    p.state.retained_limit=limit;
    run(&mut d,&mut f,&mut p,r#"if(data.firstChild!==old||data.text!=='stable\ud800'||watch.takeRecords().length)throw 'observer refusal atomic';data.text='recovery\udfff';var r=watch.takeRecords();if(r.length!==1||r[0].removedNodes[0]!==old||r[0].addedNodes[0]!==data.firstChild||data.text!=='recovery\udfff')throw 'observed script recovery';"#);
    assert!(p.take_dynamic_scripts().is_empty());
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><p id=target></p><svg id=svg></svg></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let location=crate::navigation::address("https://dataset-inspection.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);p.set_resource_base(&location);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}

#[test]
fn script_data_block_tree_child_join_keeps_split_units_and_native_text_content(){
    let(mut d,mut f,mut p)=fixture();
    run(&mut d,&mut f,&mut p,r#"var data=document.createElement('script');data.type='application/json';var a=document.createTextNode('\ud83d'),b=document.createTextNode('\ude00'),c=document.createTextNode('\ud800');data.appendChild(a);data.appendChild(b);data.appendChild(c);document.getElementById('target').appendChild(data);if(data.text!=='😀\ud800'||data.textContent!=='😀\ud800'||a.data!=='\ud83d'||b.data!=='\ude00'||c.data!=='\ud800')throw 'exact native child join';data.textContent='replace\udfff';if(data.text!=='replace\udfff'||data.firstChild.data!=='replace\udfff'||a.parentNode!==null||b.parentNode!==null||c.parentNode!==null)throw 'exact textContent producer';"#);
    assert!(p.take_dynamic_scripts().is_empty());
}

#[test]
fn script_data_block_tree_raw_set_and_read_quota_refusals_are_atomic_and_recover(){
    for write in [false,true] { for mode in 0..5 {
        let(mut d,mut f,mut p)=fixture();
        run(&mut d,&mut f,&mut p,r#"var data=document.createElement('script');data.id='data';data.type='application/json';data.text='old\ud800';document.getElementById('target').appendChild(data);var old=data.firstChild,watch=new MutationObserver(function(){});watch.observe(data,{childList:true});"#);
        p.state.rebuild(&d).unwrap();let node=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("data")).unwrap();
        let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let retained=p.state.retained;let limit=p.state.retained_limit;let strings=p.state.limits.host.string_bytes;
        p.begin_job();match mode {0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,3=>p.state.limits.host.string_bytes=1,_=>p.state.execution_deadline=Some(Instant::now())};
        let mut bridge=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};
        let error=if write {bridge.dom_set(node_handle(node),"text",&Value::StringUnits(vec![0xdfff,110,101,119])).unwrap_err()}else{bridge.dom_get(node_handle(node),"text").unwrap_err()};
        let category=match mode{0=>"work",1=>"string work",2=>"storage allowance",3=>"string allowance",_=>"time limit"};assert!(error.contains(category),"write={write} mode={mode}: {error}");
        assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert_eq!(p.state.retained,retained);
        p.state.retained_limit=limit;p.state.limits.host.string_bytes=strings;p.state.execution_deadline=None;
        run(&mut d,&mut f,&mut p,r#"if(data.text!=='old\ud800'||data.firstChild!==old||watch.takeRecords().length)throw 'atomic raw text refusal';data.text='fresh\udfff';var r=watch.takeRecords();if(data.text!=='fresh\udfff'||data.firstChild.data!=='fresh\udfff'||r.length!==1||r[0].removedNodes[0]!==old||old.parentNode!==null)throw 'native raw text recovery';"#);
        assert!(p.take_dynamic_scripts().is_empty());
    }}
}

#[test]
fn script_data_block_tree_created_data_block_has_real_text_child_and_descendant_order_without_activation(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var root=document.getElementById('target'),data=document.createElement('script'),fired=0;root.appendChild(document.createTextNode('before'));data.type='application/json';data.text='window.dataBlockExecuted=17;';data.onload=function(){fired++;};data.onerror=function(){fired++;};root.appendChild(data);root.appendChild(document.createTextNode('after'));if(!data.isConnected||data.parentNode!==root||data.firstChild.nodeType!==3||data.firstChild.data!=='window.dataBlockExecuted=17;'||data.firstChild.parentNode!==data||data.text!==data.textContent||data.text!==data.firstChild.data)throw 'actual created script Text child';if(root.textContent!=='beforewindow.dataBlockExecuted=17;after'||Object.getOwnPropertyDescriptor(Node.prototype,'textContent').get.call(root)!==root.textContent||fired||window.dataBlockExecuted!==undefined)throw 'descendant tree order does not authorize execution';"#);assert!(p.take_dynamic_scripts().is_empty(),"data block must not prepare dynamic script requests");}

#[test]
fn script_data_block_tree_connected_data_text_replacement_preserves_exact_nodes_records_and_no_activation(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var root=document.getElementById('target'),data=document.createElement('script');data.type='application/json';data.text='old\ud800';root.appendChild(data);var old=data.firstChild,watch=new MutationObserver(function(){});watch.observe(data,{childList:true});data.text='next\udfff';var records=watch.takeRecords();if(records.length!==1||records[0].target!==data||records[0].removedNodes.length!==1||records[0].removedNodes[0]!==old||records[0].addedNodes.length!==1||records[0].addedNodes[0]!==data.firstChild||old.parentNode!==null||old.data!=='old\ud800'||data.firstChild.data!=='next\udfff'||root.textContent!=='next\udfff')throw 'native replacement retains exact old/new Text nodes';data.firstChild.data+='tail';if(root.textContent!=='next\udffftail'||data.text!=='next\udffftail'||watch.takeRecords().length)throw 'live descendant character data';root.removeChild(data);if(data.isConnected||data.parentNode!==null||root.textContent!==''||data.text!=='next\udffftail')throw 'detached data source retained';root.appendChild(data);if(root.textContent!=='next\udffftail'||window.dataBlockExecuted!==undefined)throw 'reinsert data remains nonexecutable';"#);assert!(p.take_dynamic_scripts().is_empty(),"data block must not prepare dynamic script requests");}
