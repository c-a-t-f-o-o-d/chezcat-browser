//! Same-identity DocumentType adoption across actual document producers.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse_loaded_document("<!doctype html><html><body><span id=mount></span></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}
#[test]
fn xml_doctype_adoption_moves_loaded_main_identity_live_lists_observation_and_ranges(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var saved=document.doctype,html=document.documentElement,list=document.childNodes,watch=new MutationObserver(function(){}),range=new Range();watch.observe(document,{childList:true});range.setStart(document,1);range.setEnd(document,2);
var moved=document.implementation.createDocument('urn:original','p:Root',saved),records=watch.takeRecords();
if(document.doctype!==null||document.documentElement!==html||document.compatMode!=='CSS1Compat'||list.length!==1||list[0]!==html||moved.doctype!==saved||saved.ownerDocument!==moved||saved.parentNode!==moved||moved.childNodes.length!==2||moved.firstChild!==saved||moved.lastChild!==moved.documentElement||saved.nextSibling!==moved.documentElement||saved.name!=='html'||!(saved instanceof DocumentType)||moved.documentElement.previousSibling!==saved)throw 'same native loaded doctype';
if(records.length!==1||records[0].target!==document||records[0].removedNodes.length!==1||records[0].removedNodes[0]!==saved||records[0].addedNodes.length||records[0].previousSibling!==null||records[0].nextSibling!==html||range.startContainer!==document||range.startOffset!==0||range.endOffset!==1)throw 'real source removal records and live Range';
var second=moved.implementation.createDocument(null,'',saved);if(second.doctype!==saved||second.documentElement!==null||second.childNodes.length!==1||saved.parentNode!==second||saved.ownerDocument!==second||moved.doctype!==null||moved.childNodes.length!==1||moved.firstChild!==moved.documentElement||saved.nextSibling!==null)throw 'move existing adopted native identity again';
"#);assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty()&&page.take_dynamic_stylesheets().is_empty());
}
#[test]
fn xml_doctype_adoption_preserves_exact_detached_payload_and_inert_factory_parser_owners(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var owners=[document.implementation.createHTMLDocument('source'),new DOMParser().parseFromString('<!doctype html PUBLIC "original" "source"><html><body></body></html>','text/html')];
for(var old of owners){var saved=old.doctype,list=old.childNodes,root=old.documentElement,watch=new MutationObserver(function(){});watch.observe(old,{childList:true});var next=document.implementation.createDocument(null,'Root',saved),records=watch.takeRecords();if(old.doctype!==null||old.documentElement!==root||list[0]!==root||next.doctype!==saved||next.firstChild!==saved||saved.ownerDocument!==next||saved.parentNode!==next||records.length!==1||records[0].target!==old||records[0].removedNodes[0]!==saved||records[0].nextSibling!==root)throw 'actual inert source records';}
var detached=document.implementation.createDocumentType('Root','\ud800','\udfff'),saved=detached,empty=document.implementation.createDocument(null,'',detached);if(empty.doctype!==saved||empty.documentElement!==null||saved.ownerDocument!==empty||saved.publicId.charCodeAt(0)!==0xd800||saved.systemId.charCodeAt(0)!==0xdfff||saved.nodeName!=='Root')throw 'exact detached payload';
var inert=empty.implementation.createDocumentType('Next','a','b'),last=empty.implementation.createDocument('urn:next','q:Next',inert);if(inert.ownerDocument!==last||inert.parentNode!==last||last.doctype!==inert||empty.doctype!==saved||saved.ownerDocument!==empty)throw 'genuine detached inert adoption';
"#);
}
#[test]
fn xml_doctype_adoption_validation_and_storage_refusal_keep_source_and_records_then_recover(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var saved=document.doctype,watch=new MutationObserver(function(){});watch.observe(document,{childList:true});var order='',caught=false;try{document.implementation.createDocument({toString:function(){order+='n';return ''; }},{toString:function(){order+='q';return 'p:Root';}},saved);}catch(e){caught=e.name==='NamespaceError';}if(!caught||order!=='nq'||document.doctype!==saved||saved.ownerDocument!==document||watch.takeRecords().length)throw 'name validation precedes adoption';
"#);
 let before=(doc.nodes.len(),page.state.retained,page.state.dom_commits,doc.nodes[0].children.clone());let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;
 assert_eq!(page.console_eval(&mut doc,&mut forms,"document.implementation.createDocument(null,'Root',saved);").errors,1);assert_eq!((doc.nodes.len(),page.state.retained,page.state.dom_commits,doc.nodes[0].children.clone()),before);page.state.retained_limit=limit;
 run(&mut doc,&mut forms,&mut page,"if(document.doctype!==saved||saved.ownerDocument!==document||saved.parentNode!==document||watch.takeRecords().length)throw 'refused source has no prefix';var recovered=document.implementation.createDocument(null,'Root',saved);if(recovered.doctype!==saved||saved.ownerDocument!==recovered||document.doctype!==null||watch.takeRecords().length!==1)throw 'fresh job adoption recovery';");
}
