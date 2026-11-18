//! Genuine main NodeList protocol, ordinary lookup and bounded native stepping.
use super::*;

fn fixture_with(limits:crate::javascript::limits::Limits)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<!doctype html><title>NodeList protocol</title><body><div id=root><span id=a class=item name=group>A</span><span id=b class=item name=group>B</span><span id=c class=item name=group>C</span></div><form id=f><input id=neighbor name=keep value=original></form>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::with_limits(limits);
    page.set_location(&location("https://node-list-iteration.test/protocol"));
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    (doc,forms,page)
}
fn fixture()->(Document,Forms,Page) {fixture_with(crate::javascript::limits::CAPACITY_REGRESSION)}
fn location(source:&str)->crate::navigation::Location {
    crate::navigation::Location {url:Some(crate::url::parse(source).unwrap().0),path:None,root:None,fragment:String::new()}
}
fn no_resources(page:&mut Page) {
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{source}\n{}",page.report());no_resources(page);
}
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(name)).unwrap()}
fn edited(doc:&Document,forms:&mut Forms)->(usize,(String,usize,usize,usize,bool,Option<usize>)) {
    let node=id(doc,"neighbor");forms.script_set_value(node,"edited neighbor").unwrap();
    let c=forms.controls.get_mut(&node).unwrap();c.value.cursor=5;c.value.anchor=2;c.value.offset=1;
    (node,(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner))
}
fn kept_editor(doc:&Document,forms:&Forms,node:usize,kept:&(String,usize,usize,usize,bool,Option<usize>)) {
    let c=&forms.controls[&node];assert_eq!(&(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner),kept);
    assert_eq!(forms.text_default_value(node).unwrap(),"original");assert_eq!(doc.attr(node,"value"),Some("original"));
}

#[test]
fn node_list_iteration_canonical_prototype_intrinsics_brands_and_existing_native_calls() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),list=root.querySelectorAll('.item');
if(Object.getPrototypeOf(list)!==NodeList.prototype||Object.getPrototypeOf(NodeList.prototype)!==Object.prototype||NodeList.prototype.constructor!==NodeList||list.constructor!==NodeList||!(list instanceof NodeList)||list instanceof Node||Array.isArray(list))throw 'genuine NodeList prototype family';
if(list[Symbol.iterator]!==Array.prototype.values||list.values!==Array.prototype.values||list.keys!==Array.prototype.keys||list.entries!==Array.prototype.entries||list.values!==list[Symbol.iterator])throw 'owner Array iterator intrinsic identity';
var iteratorDescriptor=Object.getOwnPropertyDescriptor(NodeList.prototype,Symbol.iterator),lengthDescriptor=Object.getOwnPropertyDescriptor(NodeList.prototype,'length');
if(iteratorDescriptor.value!==Array.prototype.values||iteratorDescriptor.enumerable||!iteratorDescriptor.writable||!iteratorDescriptor.configurable)throw 'default iterator descriptor';
if(typeof lengthDescriptor.get!=='function'||lengthDescriptor.get.name!=='get length'||lengthDescriptor.get.length!==0||lengthDescriptor.set!==undefined||!lengthDescriptor.enumerable||!lengthDescriptor.configurable||lengthDescriptor.get.call(list)!==3)throw 'branded readonly length descriptor';
var rejected=0;for(var bad of [{},document,root,window])try{lengthDescriptor.get.call(bad);throw 'wrong length brand accepted';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
try{new NodeList();throw 'NodeList factory exposed';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
try{NodeList();throw 'NodeList callable factory exposed';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
if(rejected!==6)throw 'NodeList native brand/constructor boundary';
var values=list.values(),borrowed=Array.prototype.values.call(list),keys=list.keys(),entries=list.entries();
if(values[Symbol.iterator]()!==values||Object.prototype.toString.call(values)!=='[object Array Iterator]'||values.next().value!==a||borrowed.next().value!==a||keys.next().value!==0||keys.next().value!==1)throw 'actual Array iterator result/prototype';
var entry=entries.next().value;if(!Array.isArray(entry)||entry.length!==2||entry[0]!==0||entry[1]!==a)throw 'entries use Array iterator pairs';
var item=list.item,seen=[],receiver={original:true};list.forEach(function(value,index,source){if(this!==receiver||source!==list)throw 'retained native forEach receiver';seen.push(value);},receiver);
if(item!==list.item||list.item(0)!==a||list.item(3)!==null||seen.length!==3||seen[0]!==a||seen[1]!==b||seen[2]!==c)throw 'existing item/forEach changed';
"#);
}

#[test]
fn node_list_iteration_symbol_lookup_observes_own_prototype_errors_deletion_and_close() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),list=root.querySelectorAll('.item'),reads=0,calls=0;
var nativeDescriptor=Object.getOwnPropertyDescriptor(NodeList.prototype,Symbol.iterator);
Object.defineProperty(list,Symbol.iterator,{get:function(){reads++;if(this!==list)throw 'own iterator getter receiver';return function(){calls++;if(this!==list)throw 'own iterator method receiver';return [b,a].values();};},configurable:true});
var expanded=[...list];if(reads!==1||calls!==1||expanded.length!==2||expanded[0]!==b||expanded[1]!==a)throw 'own iterator getter/method order';
var rejected=0;for(var method of [null,undefined,17]){Object.defineProperty(list,Symbol.iterator,{value:method,writable:true,configurable:true});try{[...list];throw 'missing/noncallable own iterator bypassed';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}}
var sentinel={original:true};Object.defineProperty(list,Symbol.iterator,{get:function(){throw sentinel;},configurable:true});
try{[...list];throw 'iterator getter exception lost';}catch(error){if(error!==sentinel)throw error;rejected++;}
delete list[Symbol.iterator];if(list[Symbol.iterator]!==nativeDescriptor.value||[...list][0]!==a)throw 'own symbol deletion did not restore real default';
Object.defineProperty(NodeList.prototype,Symbol.iterator,{get:function(){reads++;if(this!==list)throw 'prototype iterator getter receiver';return function(){return [b].values();};},configurable:true});
if([...list][0]!==b||reads!==2)throw 'live prototype iterator getter';
delete NodeList.prototype[Symbol.iterator];try{[...list];throw 'deleted prototype iterator has ghost fallback';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
Object.defineProperty(NodeList.prototype,Symbol.iterator,nativeDescriptor);
var closed=0,steps=0;list[Symbol.iterator]=function(){return {next:function(){steps++;return {value:a,done:false};},return:function(){closed++;return {done:true};}};};
for(var value of list){if(value!==a)throw 'custom iterator value';break;}
if(closed!==1||steps!==1||rejected!==5)throw 'normal abrupt IteratorClose over real NodeList source';
delete list[Symbol.iterator];if([...list].length!==3)throw 'default restoration after custom close';
"#);
}

#[test]
fn node_list_iteration_lazy_live_children_names_and_retained_static_snapshot() {
    let(mut doc,mut forms,mut page)=fixture();let(node,editor)=edited(&doc,&mut forms);
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
var snapshot=root.querySelectorAll('.item'),children=root.childNodes,names=document.getElementsByName('group');
var saved=snapshot.values(),live=children.values(),named=names.values();
if(saved.next().value!==a||live.next().value!==a||named.next().value!==a)throw 'first actual identities';
root.removeChild(b);if(b.parentNode!==null||b.isConnected)throw 'real detached named node';
var completedName=names.values();if(completedName.next().value!==a||completedName.next().value!==c||!completedName.next().done)throw 'name iterator after actual removal';
root.appendChild(b);
if(saved.next().value!==b||saved.next().value!==c||!saved.next().done||b.parentNode!==root||snapshot.length!==3)throw 'static iterator changed after physical mutation';
if(live.next().value!==c||live.next().value!==b||!live.next().done||children.length!==3)throw 'live child iterator eagerly captured old list';
if(named.next().value!==c||named.next().value!==b||!named.next().done||names.length!==3||!completedName.next().done)throw 'live name iteration or completed source restarted after parsed-node reinsertion';
var e=document.createElement('span');root.appendChild(e);if(!live.next().done||!named.next().done)throw 'completed Array iterator restarted after append';
if(document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='original')throw 'iteration reset native input';
var ordinary=(function(){var kept={check:function(){return 'retained ordinary source';}};return [kept].values();})();
var listRoot=root.querySelectorAll('.item');listRoot[Symbol.iterator]=(function(kept){return function(){return [kept].values();};})(a);
"#);
    run(&mut doc,&mut forms,&mut page,"if(ordinary.next().value.check()!=='retained ordinary source'||[...listRoot][0]!==a)throw 'later-job ordinary iterator/native backing closure reachability';");
    kept_editor(&doc,&forms,node,&editor);
}

#[test]
fn node_list_iteration_effective_length_conversion_and_absent_index_prototype_fallback() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),list=root.querySelectorAll('.item');
var nativeLength=Object.getOwnPropertyDescriptor(NodeList.prototype,'length').get,reads=0,converted=0;
Object.defineProperty(list,'length',{get:function(){reads++;return {valueOf:function(){converted++;return 2.9;}};},configurable:true});
var expanded=[...list];if(expanded.length!==2||expanded[0]!==a||expanded[1]!==b||reads!==3||converted!==3||nativeLength.call(list)!==3)throw 'iterator did not read/convert ordinary effective length each step';
var token={prototypeValue:true},present=0;Object.defineProperty(NodeList.prototype,'0',{get:function(){present++;return token;},configurable:true});
Object.defineProperty(NodeList.prototype,'3',{get:function(){if(this!==list)throw 'absent index getter receiver';return token;},configurable:true});
Object.defineProperty(list,'length',{value:5,writable:true,configurable:true});
expanded=[...list];if(expanded.length!==5||expanded[0]!==a||expanded[1]!==b||expanded[2]!==c||expanded[3]!==token||expanded[4]!==undefined||present||list.item(3)!==null)throw 'present native slots or absent prototype fallback';
list[0]=token;if(list[0]!==a||Object.getOwnPropertyDescriptor(list,'0').writable)throw 'native index readonly boundary';
for(var length of [0,-2,NaN]){list.length=length;if([...list].length!==0)throw 'ToLength zero/negative/NaN';}
list.length=Symbol('length');try{[...list];throw 'Symbol length accepted';}catch(error){if(error.name!=='TypeError')throw error;}
delete list.length;delete NodeList.prototype[0];delete NodeList.prototype[3];
if(list.length!==3||[...list].length!==3||nativeLength.call(list)!==3)throw 'ordinary length/index restoration';
"#);
}

#[test]
fn node_list_iteration_index_enumeration_inspection_assign_and_for_in_survive_backing() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),list=root.querySelectorAll('.item');
function checkOwn(){
 if(Object.keys(list).join(',')!=='0,1,2'||Object.getOwnPropertyNames(list).join(',')!=='0,1,2')throw 'native own numeric keys hidden by backing';
 var values=Object.values(list),entries=Object.entries(list),descriptors=Object.getOwnPropertyDescriptors(list),copy=Object.assign({},list);
 if(values.length!==3||values[0]!==a||values[1]!==b||values[2]!==c)throw 'native own Object.values';
 if(entries.length!==3||entries[0][0]!=='0'||entries[0][1]!==a)throw 'native own Object.entries';
 if(descriptors[0].value!==a||descriptors[0].writable||!descriptors[0].enumerable)throw 'native own Object.getOwnPropertyDescriptors';
 if(copy[0]!==a||copy[1]!==b||copy[2]!==c)throw 'native own Object.assign';
}
checkOwn();if([...list].length!==3)throw 'actual iteration before repeated inspection';checkOwn();
var marker=Symbol('own'),token={original:true};list.note='ordinary';list[marker]=token;Object.defineProperty(list,'hiddenNote',{value:'hidden',configurable:true});
if(Object.keys(list).join(',')!=='0,1,2,note'||Object.getOwnPropertyNames(list).join(',')!=='0,1,2,note,hiddenNote'||Object.assign({},list)[marker]!==token||Object.assign({},list).note!=='ordinary')throw 'merged own ordinary/symbol properties';
Object.defineProperty(NodeList.prototype,'0',{value:token,enumerable:true,configurable:true});
Object.defineProperty(NodeList.prototype,'later',{value:token,enumerable:true,configurable:true});
var names=[],zeroes=0;for(var key in list){names.push(key);if(key==='0')zeroes++;}
if(names[0]!=='0'||names[1]!=='1'||names[2]!=='2'||zeroes!==1||names.indexOf('note')===-1||names.indexOf('later')===-1||list.later!==token||list[0]!==a)throw 'numeric-first native keys, dedupe or inherited ordinary lookup';
delete NodeList.prototype[0];delete NodeList.prototype.later;
delete list.note;delete list.hiddenNote;delete list[marker];checkOwn();
"#);
}

#[test]
fn node_list_iteration_realms_keep_owner_intrinsics_after_parent_array_methods_change() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),list=root.querySelectorAll('.item');
var values=Array.prototype.values,keys=Array.prototype.keys,entries=Array.prototype.entries;
Array.prototype.values=function(){throw 'altered parent values';};Array.prototype.keys=function(){throw 'altered parent keys';};Array.prototype.entries=function(){throw 'altered parent entries';};
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var childValues=w.Array.prototype.values,childKeys=w.Array.prototype.keys,childEntries=w.Array.prototype.entries;
if(NodeList.prototype[Symbol.iterator]!==values||NodeList.prototype.values!==values||NodeList.prototype.keys!==keys||NodeList.prototype.entries!==entries||[...list][0]!==a)throw 'parent NodeList intrinsic aliases followed author Array changes';
if(w.NodeList.prototype[Symbol.iterator]!==childValues||w.NodeList.prototype.values!==childValues||w.NodeList.prototype.keys!==childKeys||w.NodeList.prototype.entries!==childEntries||childValues===values||Object.getPrototypeOf(w.NodeList.prototype)!==w.Object.prototype)throw 'child NodeList factory used parent methods or wrong realm';
var read=w.Function('list','return list[Symbol.iterator];'),step=w.Function('iterator','return iterator.next();'),iterator=values.call(list);
if(read(list)!==values||step(iterator).value!==a||Object.getPrototypeOf(list)!==NodeList.prototype)throw 'same-origin list owner/prototype/intrinsic identity';
Array.prototype.values=values;Array.prototype.keys=keys;Array.prototype.entries=entries;
"#);
}

#[test]
fn node_list_iteration_cached_next_and_symbol_getter_revalidate_actual_reader_origin() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),list=root.querySelectorAll('.item'),gets=0;
var values=Array.prototype.values,iterator=values.call(list),cachedNext=iterator.next;
Object.defineProperty(list,Symbol.iterator,{get:function(){gets++;return values;},configurable:true});
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var read=w.Function('list','return list[Symbol.iterator];'),spread=w.Function('list','return [...list];'),step=w.Function('next','iterator','return next.call(iterator);'),own=w.Function('return window===self&&document.body.nodeType===1;');
if(read(list)!==values||gets!==1||step(cachedNext,iterator).value!==a||!own())throw 'same-origin callback/next prefix';gets=0;
"#);
    page.set_location(&location("https://different-node-list.test/changed"));
    for source in ["read(list);","spread(list);","step(cachedNext,iterator);"] {
        page.clear_console();let allocations=page.runtime.window_allocation_state_for_test();
        let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}\n{}",page.report());assert!(page.report().contains("same-origin"),"{source}\n{}",page.report());
        let after=page.runtime.window_allocation_state_for_test();
        // Array literal evaluation allocates its unexposed result before
        // GetIterator. This is separate from forbidden native realm creation.
        let array=usize::from(source=="spread(list);");
        assert_eq!(after,(allocations.0,allocations.1+array,allocations.2,allocations.3),"{source}");
        run(&mut doc,&mut forms,&mut page,"if(gets||!own())throw 'foreign reader reached getter or damaged owner context';");
    }
    run(&mut doc,&mut forms,&mut page,"if(cachedNext.call(iterator).value!==b||gets||list.length!==3)throw 'foreign next advanced cursor or broke main owner';");
}

#[test]
fn node_list_iteration_host_work_failure_and_spread_capacity_preserve_native_state() {
    for allowance in [0,1] {
        let(mut doc,mut forms,mut page)=fixture();let(node,editor)=edited(&doc,&mut forms);
        run(&mut doc,&mut forms,&mut page,"var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),list=root.querySelectorAll('.item'),iterator=list.values(),caught=0,finalized=0,after=0;");
        let snapshot=format!("{:?}",doc.nodes);let limit=page.state.limits.host_work;
        page.state.limits.host_work=allowance;page.clear_console();
        let out=page.console_eval(&mut doc,&mut forms,"try{iterator.next();after++;}catch(error){caught++;}finally{finalized++;}after++;");
        assert_eq!(out.errors,1,"allowance={allowance}\n{}",page.report());assert!(page.runtime.invocation_failed_fatally());
        assert_eq!(format!("{:?}",doc.nodes),snapshot);kept_editor(&doc,&forms,node,&editor);no_resources(&mut page);
        page.state.limits.host_work=limit;
        let expected=if allowance==0 {"a"} else {"b"};
        run(&mut doc,&mut forms,&mut page,&format!("if(caught||finalized||after||iterator.next().value!=={expected})throw 'per-step work failure cursor/context cleanup';"));
    }
    let mut limits=crate::javascript::limits::CAPACITY_REGRESSION;limits.capacity.arguments=2;
    let(mut doc,mut forms,mut page)=fixture_with(limits);let(node,editor)=edited(&doc,&mut forms);
    run(&mut doc,&mut forms,&mut page,"var list=document.getElementById('root').querySelectorAll('.item'),entered=0;function sink(){entered++;}");
    let snapshot=format!("{:?}",doc.nodes);let out=page.console_eval(&mut doc,&mut forms,"sink(...list);");
    assert_eq!(out.errors,1,"{}",page.report());assert!(page.runtime.invocation_failed_fatally());assert!(page.report().contains("argument limit"),"{}",page.report());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);kept_editor(&doc,&forms,node,&editor);no_resources(&mut page);
    run(&mut doc,&mut forms,&mut page,"if(entered||list.item(0)!==document.getElementById('a'))throw 'spread capacity entered sink or reset native state';");
    let mut limits=crate::javascript::limits::CAPACITY_REGRESSION;limits.heap.array_length=2;
    let(mut doc,mut forms,mut page)=fixture_with(limits);let(node,editor)=edited(&doc,&mut forms);
    run(&mut doc,&mut forms,&mut page,"document.getElementById('a').className='item two';document.getElementById('b').className='item two';var list=document.getElementById('root').querySelectorAll('.two');if(list.length!==2)throw 'array spread cap prefix';");
    let snapshot=format!("{:?}",doc.nodes);let out=page.console_eval(&mut doc,&mut forms,"[0,...list];");
    assert_eq!(out.errors,1,"{}",page.report());assert!(page.runtime.invocation_failed_fatally());assert!(page.report().contains("Array length limit"),"{}",page.report());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);kept_editor(&doc,&forms,node,&editor);no_resources(&mut page);
}

#[test]
fn node_list_iteration_iterator_allocation_limit_is_fatal_and_keeps_real_native_controls() {
    let mut limits=crate::javascript::limits::CAPACITY_REGRESSION;limits.heap.objects=2048;
    let(mut doc,mut forms,mut page)=fixture_with(limits);let(node,editor)=edited(&doc,&mut forms);
    run(&mut doc,&mut forms,&mut page,"var list=document.getElementById('root').querySelectorAll('.item'),retained=[],created=0,caught=0,finalized=0,after=0;");
    let snapshot=format!("{:?}",doc.nodes);
    let out=page.console_eval(&mut doc,&mut forms,"try{while(created<4096){retained.push(list.values());created++;}after++;}catch(error){caught++;}finally{finalized++;}after++;");
    assert_eq!(out.errors,1,"{}",page.report());assert!(page.runtime.invocation_failed_fatally());assert!(page.report().contains("Object"),"{}",page.report());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);kept_editor(&doc,&forms,node,&editor);no_resources(&mut page);
}

#[test]
fn node_list_iteration_other_native_handles_remain_unapproved_despite_inherited_symbols() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var root=document.getElementById('root'),a=document.getElementById('a'),list=root.querySelectorAll('.item'),touched=0;Object.prototype[Symbol.iterator]=function(){touched++;return [a].values();};");
    let snapshot=format!("{:?}",doc.nodes);let root=id(&doc,"root");
    {
        let bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        for handle in [WINDOW,DOCUMENT,LOCATION,CONSOLE,NAVIGATOR,node_handle(root),NODE_LIST_BASE+u32::try_from(bridge.state.node_lists.len()).unwrap()] {
            assert_eq!(bridge.indexed_iteration_owner(handle),None);
        }
    }
    for source in ["[...document];","Array.prototype.values.call(document);"] {
        page.clear_console();let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}\n{}",page.report());assert!(page.report().contains("Host-object iteration is not implemented"),"{}",page.report());
        assert_eq!(format!("{:?}",doc.nodes),snapshot);
        run(&mut doc,&mut forms,&mut page,"if(touched||[...list][0]!==a)throw 'unrelated Host reached inherited iterator or damaged genuine source';");
    }
    run(&mut doc,&mut forms,&mut page,"delete Object.prototype[Symbol.iterator];if([...list].length!==3)throw 'normal genuine iteration recovery';");
}
