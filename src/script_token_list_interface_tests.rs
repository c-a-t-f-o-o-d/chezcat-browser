//! Original DOMTokenList canonical binding and indexed protocol controls.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn token_list_interface_canonical_family_descriptors_and_illegal_constructor(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
if(typeof DOMTokenList!=='function')throw 'native token interface';
var P=DOMTokenList.prototype,a=document.getElementById('target').classList,b=document.getElementById('frame').sandbox;
if(Object.getPrototypeOf(P)!==Object.prototype||P.constructor!==DOMTokenList||Object.getPrototypeOf(a)!==P||Object.getPrototypeOf(b)!==P||!(a instanceof DOMTokenList)||a instanceof Node||Object.prototype.toString.call(a)!=='[object DOMTokenList]')throw 'canonical family';
var ld=Object.getOwnPropertyDescriptor(P,'length'),vd=Object.getOwnPropertyDescriptor(P,'value');
if(!ld.enumerable||!ld.configurable||ld.set!==undefined||ld.get.name!=='get length'||ld.get.length!==0||ld.get.call(a)!==2||!vd.enumerable||!vd.configurable||vd.get.name!=='get value'||vd.set.name!=='set value'||vd.set.length!==1)throw 'canonical attributes';
var methods=['item','contains','add','remove','supports','toString'],lengths=[1,1,0,0,1,0];
for(var i=0;i<methods.length;i++){var name=methods[i],d=Object.getOwnPropertyDescriptor(P,name);if(!d.enumerable||!d.configurable||!d.writable||d.value!==a[name]||d.value!==b[name]||d.value.name!==name||d.value.length!==lengths[i])throw 'canonical method';}
var rejected=0;try{new DOMTokenList();}catch(e){if(!(e instanceof TypeError))throw e;rejected++;}try{DOMTokenList();}catch(e){if(!(e instanceof TypeError))throw e;rejected++;}if(rejected!==2)throw 'illegal native constructor';
if(Object.prototype.hasOwnProperty.call(a,'length')||Object.prototype.hasOwnProperty.call(a,'contains')||Object.keys(a).join(',')!=='0,1')throw 'native own slots';
"#);
}
#[test]
fn token_list_interface_ordinary_shadows_deletion_saved_methods_and_conversion(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var P=DOMTokenList.prototype,a=document.getElementById('target').classList,b=document.getElementById('frame').sandbox,contains=P.contains,vd=Object.getOwnPropertyDescriptor(P,'value'),calls=0;
delete P.contains;if(a.contains!==undefined||'contains' in a||!contains.call(a,'one'))throw 'deleted method resurrection';
P.contains=function(token){if(this!==a)throw 'ordinary receiver';calls++;return token==='shadow';};if(!a.contains('shadow')||calls!==1)throw 'ordinary prototype replacement';
a.contains=17;if(a.contains!==17||P.contains===17||Object.keys(a).join(',')!=='0,1,contains')throw 'own method shadow';delete a.contains;if(typeof a.contains!=='function')throw 'own deletion';P.contains=contains;
Object.defineProperty(a,'value',{value:'own',configurable:true});if(a.value!=='own'||vd.get.call(a)!=='one two one')throw 'own accessor shadow';vd.set.call(a,'A�z');if(vd.get.call(a)!=='A�z'||a.value!=='own')throw 'canonical setter bypass';delete a.value;
var order='',probe={toString:function(){order+='s';vd.set.call(a,'later');return 'later';}};if(!contains.call(a,probe)||order!=='s')throw 'conversion observes live record';
if(contains.call(a,'\ud800')||!contains.call(a,'later')||contains.call(b,'later')||P.supports.call(b,'allow-scripts'))throw 'exact lookup and sandbox authority';
var marker={};try{contains.call(a,{toString:function(){throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
var rejected=0;for(var i=0;i<3;i++){try{if(i===0)contains.call({},probe);else if(i===1)vd.set.call({},probe);else Object.getOwnPropertyDescriptor(P,'length').get.call({});}catch(e){if(!(e instanceof TypeError))throw e;rejected++;}}if(rejected!==3||order!=='s')throw 'brand before conversion';
P.add.call(a,'first','second');P.remove.call(a,'later');if(vd.get.call(a)!=='first second'||!contains.call(a,'first'))throw 'canonical existing mutators';
"#);
}
#[test]
fn token_list_interface_indexed_protocol_live_ordinary_length_and_result_order(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var a=document.getElementById('target').classList,P=DOMTokenList.prototype,iterator=P.values,ld=Object.getOwnPropertyDescriptor(P,'length');
if(P.values!==Array.prototype.values||P.keys!==Array.prototype.keys||P.entries!==Array.prototype.entries||P[Symbol.iterator]!==P.values)throw 'realm iterator intrinsics';
if(Array.from(a).join(',')!=='one,two'||[...a].join(',')!=='one,two'||Array.from(a.keys()).join(',')!=='0,1'||Array.from(a.entries())[1].join(',')!=='1,two')throw 'actual indexed protocol';
var it=a.values();if(it.next().value!=='one')throw 'first token';a.value='changed other last';if(it.next().value!=='other')throw 'live token read';Object.defineProperty(a,'length',{get:function(){return 1;},configurable:true});if(!it.next().done)throw 'ordinary length';delete a.length;if(!it.next().done||ld.get.call(a)!==3)throw 'completed iterator stays complete';
var saved=a[Symbol.iterator];a[Symbol.iterator]=function(){var done=false;return {next:function(){if(done)return {done:true};done=true;return {done:false,value:'author'};}};};if(Array.from(a).join(',')!=='author')throw 'ordinary iterator override';delete a[Symbol.iterator];if(a[Symbol.iterator]!==saved||Array.from(a).join(',')!=='changed,other,last')throw 'iterator delete recovery';
"#);
}
#[test]
fn token_list_interface_native_owner_expiry_preserves_attribute_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var list=document.getElementById('target').classList,saved=DOMTokenList.prototype.contains,iterator=list.values();saved.call(list,'one');iterator.next();");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();
    let kind=std::mem::replace(&mut doc.nodes[node].kind,NodeKind::Text("temporary".into()));
    let commits=page.state.dom_commits;
    for source in ["saved.call(list,'one')","iterator.next()","Object.keys(list)"]{
        page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("native")||error.contains("Native")||error.contains("receiver")||error.contains("owner")||error.contains("Host-object"),"{error}");assert_eq!(page.state.dom_commits,commits);
    }
    doc.nodes[node].kind=kind;page.begin_job();run(&mut doc,&mut forms,&mut page,"if(!saved.call(list,'one')||list.value!=='one two one')throw 'fresh owner recovery';");
}

#[test]
fn token_list_interface_observation_and_protected_index_keys_use_actual_record(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var e=document.getElementById('target'),list=e.classList,P=DOMTokenList.prototype,vd=Object.getOwnPropertyDescriptor(P,'value'),records=[],observer=new MutationObserver(function(batch){records=records.concat(batch);});
observer.observe(e,{attributes:true,attributeOldValue:true,attributeFilter:['class']});
P.add.call(list,{toString:function(){return 'three';}});P.remove.call(list,'one');vd.set.call(list,'A�z other');
var pending=observer.takeRecords();if(pending.length!==3||pending[0].oldValue!=='one two one'||pending[1].oldValue!=='one two three'||pending[2].oldValue!=='two three'||e.className!=='A�z other')throw 'canonical token attribute transaction';
var marker=Symbol('own'),token={kept:true};list[marker]=token;list.note='ordinary';Object.defineProperty(list,'hidden',{value:'hidden',configurable:true});
var own=Object.getOwnPropertyNames(list),symbols=Object.getOwnPropertySymbols(list),copy=Object.assign({},list),d=Object.getOwnPropertyDescriptor(list,'0');
if(own.join(',')!=='0,1,note,hidden'||Object.keys(list).join(',')!=='0,1,note'||symbols.length!==1||symbols[0]!==marker||copy[marker]!==token||copy[0]!=='A�z'||copy.note!=='ordinary'||d.writable||!d.enumerable||d.value!=='A�z')throw 'actual indexed own inspection';
list[0]='forged';if(list[0]!=='A�z'||vd.get.call(list)!=='A�z other')throw 'protected native indexed slot';
Object.defineProperty(P,'later',{value:token,enumerable:true,configurable:true});var names=[],zeroes=0;for(var key in list){names.push(key);if(key==='0')zeroes++;}if(names[0]!=='0'||names[1]!=='1'||zeroes!==1||names.indexOf('note')<0||names.indexOf('later')<0)throw 'merged inherited enumeration';delete P.later;
observer.disconnect();
"#);
}
#[test]
fn token_list_interface_indexed_length_limit_refuses_without_dom_commit_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();let maximum=page.state.limits.heap.array_length;
    run(&mut doc,&mut forms,&mut page,&format!("var list=document.getElementById('target').classList,iterator=list.values();Object.defineProperty(list,'length',{{value:{},configurable:true}});",maximum+1));
    let commits=page.state.dom_commits;page.begin_job();
    let error=page.runtime.execute("iterator.next()",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
    assert!(error.contains("iterator length limit"),"{error}");assert_eq!(page.state.dom_commits,commits);
    page.begin_job();run(&mut doc,&mut forms,&mut page,"delete list.length;if(iterator.next().value!=='one'||iterator.next().value!=='two'||!iterator.next().done)throw 'indexed limit fresh recovery';");
}

#[test]
fn token_list_interface_native_raw_record_iterates_exact_units_and_keeps_clone_independent(){
    let(mut doc,mut forms,mut page)=fixture();
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();
    let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind else{unreachable!()};
    attrs.iter_mut().find(|a|a.name.html_is("class")).unwrap().value=DomString::from_units(&[65,0xd800,122,32,65,0xdfff,122,32,65,0xd800,122],128).unwrap();page.state.rebuild(&doc).unwrap();
    // Native record input qualifies the read/protocol boundary; public raw
    // class mutation remains gated pending exact CSS integration.
    run(&mut doc,&mut forms,&mut page,r#"
var e=document.getElementById('target'),list=e.classList,values=Array.from(list),P=DOMTokenList.prototype;
if(values.length!==2||values[0]!=='A\ud800z'||values[1]!=='A\udfffz'||Array.from(list.entries())[1][1]!==values[1]||!P.contains.call(list,values[0])||P.contains.call(list,'A�z'))throw 'native exact indexed protocol';
var clone=e.cloneNode(false);e.className='scalar';if(Array.from(clone.classList)[0]!==values[0]||Array.from(list)[0]!=='scalar'||Object.getPrototypeOf(clone.classList)!==P)throw 'native exact clone and live protocol';
"#);
}

#[test]
fn token_list_interface_child_intrinsics_saved_methods_recheck_actual_origin(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,list=document.getElementById('target').classList;
var childP=w.DOMTokenList.prototype,contains=childP.contains,get=Object.getOwnPropertyDescriptor(childP,'length').get;
if(childP===DOMTokenList.prototype||childP.values!==w.Array.prototype.values||childP[Symbol.iterator]!==childP.values||!contains.call(list,'one')||get.call(list)!==2||Array.from(childP.values.call(list)).join(',')!=='one,two')throw 'actual child intrinsic family and native borrowing';
"#);
    let commits=page.state.dom_commits;
    page.set_location(&crate::navigation::address("https://foreign-token.test/source",&crate::navigation::Location::home()).unwrap());
    for source in ["contains.call(list,'one')","get.call(list)"]{
        page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(page.state.dom_commits,commits);
    }
}
