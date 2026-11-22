//! CDATA has a genuine native kind and preserves exclusive-Text boundaries.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse_loaded_document("<!doctype html><html><body><div id=mount></div></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}
#[test]
fn native_cdata_xml_factory_brand_conversion_ancestry_units_and_delimiter(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createDocument(null,'Root'),calls=0,make=Document.prototype.createCDATASection,c=make.call(d,{toString:function(){calls++;return '\ud800A\udfff';}});
if(calls!==1||c.nodeType!==4||c.nodeName!=='#cdata-section'||c.ownerDocument!==d||c.parentNode!==null||c.data.length!==3||c.data.charCodeAt(0)!==0xd800||c.data.charCodeAt(2)!==0xdfff||c.nodeValue!==c.data||c.textContent!==c.data||c.length!==3||!(c instanceof CDATASection)||!(c instanceof Text)||!(c instanceof CharacterData)||!(c instanceof Node)||c instanceof Comment||Object.getPrototypeOf(c)!==CDATASection.prototype||Object.getPrototypeOf(CDATASection.prototype)!==Text.prototype)throw 'actual native CDATA';
var caught=false;try{make.call(document,{toString:function(){calls++;return ']]>';}});}catch(e){caught=e.name==='NotSupportedError';}if(!caught||calls!==2)throw 'HTML rejection after conversion before delimiter';
caught=false;try{make.call(d,']]>');}catch(e){caught=e.name==='InvalidCharacterError';}if(!caught)throw 'XML delimiter';
caught=false;try{make.call({}, {toString:function(){calls++;return 'x';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls!==2)throw 'brand before conversion';
caught=false;try{new CDATASection('x');}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'illegal constructor';
d.documentElement.appendChild(c);if(d.documentElement.textContent!==c.data||d.documentElement.firstChild!==c||c.getRootNode()!==d)throw 'native owner tree consumption';
var copy=c.cloneNode(true);if(copy===c||copy.nodeType!==4||!(copy instanceof CDATASection)||copy.ownerDocument!==d||copy.data!==c.data)throw 'actual inert clone';
"#);
}
#[test]
fn native_cdata_import_mutation_old_units_range_and_exclusive_text_normalization(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createDocument(null,'Root'),original=d.createCDATASection('\ud800A\udfff'),c=document.importNode(original,true),mount=document.getElementById('mount');
if(c===original||c.nodeType!==4||!(c instanceof CDATASection)||c.ownerDocument!==document||c.data!==original.data)throw 'actual imported CDATA';
var left=document.createTextNode('left'),right=document.createTextNode('right'),empty=document.importNode(d.createCDATASection(''),true);mount.appendChild(left);mount.appendChild(c);mount.appendChild(right);mount.appendChild(empty);mount.appendChild(document.createTextNode('tail'));mount.normalize();
if(mount.childNodes.length!==5||mount.childNodes[1]!==c||mount.childNodes[3]!==empty||empty.parentNode!==mount||left.data!=='left'||right.data!=='right'||mount.textContent!=='left\ud800A\udfffrighttail')throw 'exclusive Text normalization boundaries';
var range=new Range();range.setStart(c,1);range.setEnd(c,3);if(range.toString()!=='A\udfff'||range.startContainer!==c||range.endOffset!==3)throw 'UTF16 CDATA Range';
var observer=new MutationObserver(function(){});observer.observe(c,{characterData:true,characterDataOldValue:true});c.data='\udfffB\ud800';var records=observer.takeRecords();if(records.length!==1||records[0].target!==c||records[0].oldValue!=='\ud800A\udfff'||c.nodeValue!==c.data||c.textContent!==c.data)throw 'actual observed data';observer.disconnect();
var inert=d.createCDATASection('old'),watch=new MutationObserver(function(){});watch.observe(inert,{characterData:true,characterDataOldValue:true});inert.textContent='\udfff';records=watch.takeRecords();if(records.length!==1||records[0].oldValue!=='old'||inert.nodeType!==4||inert.data.charCodeAt(0)!==0xdfff)throw 'inert mutation preserves kind';
var clone=c.cloneNode(false);if(clone.nodeType!==4||clone.data!==c.data||clone.ownerDocument!==document)throw 'native clone kind';
"#);
 assert!(doc.nodes.iter().any(|node|matches!(&node.kind,NodeKind::Cdata(data) if data.utf16_len()==3)));
 assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty()&&page.take_dynamic_stylesheets().is_empty());
}
#[test]
fn native_cdata_factory_retained_refusal_does_not_publish_and_recovers(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var xml=document.implementation.createDocument(null,'Root');");let before=(doc.nodes.len(),page.state.retained,page.state.dom_commits);let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;
 assert_eq!(page.console_eval(&mut doc,&mut forms,"xml.createCDATASection('refused');").errors,1);assert_eq!((doc.nodes.len(),page.state.retained,page.state.dom_commits),before);page.state.retained_limit=limit;
 run(&mut doc,&mut forms,&mut page,"var recovered=xml.createCDATASection('recovered');if(recovered.nodeType!==4||recovered.ownerDocument!==xml||recovered.data!=='recovered')throw 'CDATA recovery';");
}
#[test]
fn native_cdata_saved_inert_identity_author_fields_and_observer_survive_promotion(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var xml=document.implementation.createDocument(null,''),fragment=xml.createDocumentFragment(),saved=xml.createCDATASection('stable CDATA'),mount=document.getElementById('mount');saved.authorField={value:17};var key=saved.authorField,watch=new MutationObserver(function(){});watch.observe(saved,{characterData:true,characterDataOldValue:true});fragment.appendChild(saved);mount.appendChild(fragment);
if(mount.firstChild!==saved||saved.ownerDocument!==document||saved.nodeType!==4||!(saved instanceof CDATASection)||saved.authorField!==key||key.value!==17||fragment.firstChild!==null)throw 'stable promoted identity';saved.nodeValue='updated';var records=watch.takeRecords();if(records.length!==1||records[0].target!==saved||records[0].oldValue!=='stable CDATA'||saved.textContent!=='updated'||mount.textContent!=='updated')throw 'saved promoted observer and data';
"#);
}
