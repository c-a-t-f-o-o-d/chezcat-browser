//! Actual native XML factory records, nullable roots and owner-specific names.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse_loaded_document("<!doctype html><html><head></head><body><span id=mount></span></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();page.set_location(&crate::navigation::address("https://xml-records.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}
fn no_effects(page:&mut Page){assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty()&&page.take_dynamic_stylesheets().is_empty());let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());}
#[test]
fn native_xml_document_factory_kind_optional_roots_mime_names_and_owner_are_real(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var make=DOMImplementation.prototype.createDocument;if(make.length!==2||make.name!=='createDocument'||Object.getPrototypeOf(XMLDocument.prototype)!==Document.prototype||Object.getPrototypeOf(XMLDocument)!==Document)throw 'actual XMLDocument interface';
var d=make.call(document.implementation,'urn:original','p:Mixed',null),root=d.documentElement;
if(!(d instanceof XMLDocument)||!(d instanceof Document)||!(d instanceof Node)||d instanceof Element||Object.getPrototypeOf(d)!==XMLDocument.prototype||d.nodeType!==9||d.nodeName!=='#document'||d.ownerDocument!==null||d.parentNode!==null||d.defaultView!==null||d.contentType!=='application/xml'||d.compatMode!=='CSS1Compat'||d.URL!=='about:blank'||d.characterSet!=='UTF-8'||d.head!==null||d.body!==null||d.doctype!==null||d.childNodes.length!==1||d.firstChild!==root||d.lastChild!==root)throw 'real XML document records';
if(root.ownerDocument!==d||root.parentNode!==d||root.namespaceURI!=='urn:original'||root.prefix!=='p'||root.localName!=='Mixed'||root.tagName!=='p:Mixed'||root.nodeName!=='p:Mixed'||!(root instanceof Element)||root instanceof HTMLElement||root.getRootNode()!==d)throw 'real XML root records';
var empty=document.implementation.createDocument(null,'',null);if(empty.documentElement!==null||empty.head!==null||empty.body!==null||empty.doctype!==null||empty.childNodes.length||empty.firstChild!==null||empty.contentType!=='application/xml'||!(empty instanceof XMLDocument))throw 'genuine empty XML document';
var html=document.implementation.createDocument('http://www.w3.org/1999/xhtml','h:DiV',null),svg=document.implementation.createDocument('http://www.w3.org/2000/svg','s:svg',null);if(html.contentType!=='application/xhtml+xml'||html.documentElement.tagName!=='h:DiV'||html.documentElement.localName!=='DiV'||html.documentElement.ownerDocument!==html||svg.contentType!=='image/svg+xml'||svg.documentElement.prefix!=='s'||svg.documentElement.ownerDocument!==svg)throw 'MIME and document-specific case';
var nested=d.implementation.createDocument('urn:second','q:Second');if(nested===d||nested.documentElement.ownerDocument!==nested||nested.documentElement.namespaceURI!=='urn:second')throw 'real associated implementation factory';
"#);no_effects(&mut page);
}
#[test]
fn native_xml_document_common_factories_keep_null_namespaces_case_and_character_units(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createDocument('urn:original','p:Root'),root=d.documentElement,local=d.createElement('p:MiXeD'),text=d.createTextNode('\ud800'),comment=d.createComment('\udfff'),fragment=d.createDocumentFragment();
if(local.namespaceURI!==null||local.prefix!==null||local.localName!=='p:MiXeD'||local.tagName!=='p:MiXeD'||local.ownerDocument!==d||text.ownerDocument!==d||text.data.charCodeAt(0)!==0xd800||comment.ownerDocument!==d||comment.data.charCodeAt(0)!==0xdfff||fragment.ownerDocument!==d)throw 'actual XML owner factories';
fragment.appendChild(local);local.appendChild(text);local.appendChild(comment);root.appendChild(fragment);if(fragment.childNodes.length||root.firstChild!==local||text.parentNode!==local||local.getRootNode()!==d||local.textContent.charCodeAt(0)!==0xd800)throw 'genuine XML child topology';
var copy=root.cloneNode(true);if(copy===root||copy.ownerDocument!==d||copy.namespaceURI!==root.namespaceURI||copy.prefix!=='p'||copy.localName!=='Root'||copy.firstChild===local||copy.firstChild.localName!=='p:MiXeD'||copy.firstChild.firstChild.data.charCodeAt(0)!==0xd800)throw 'actual XML root subtree copy';
var name=d.createElementNS('urn:other','q:MiXeD');if(name.ownerDocument!==d||name.namespaceURI!=='urn:other'||name.prefix!=='q'||name.localName!=='MiXeD'||name.nodeName!=='q:MiXeD')throw 'genuine XML namespaced factory';
"#);no_effects(&mut page);
}
#[test]
fn native_xml_document_conversion_namespace_validation_and_unsupported_adoption_are_atomic(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var order='',make=DOMImplementation.prototype.createDocument,d=make.call(document.implementation,{toString:function(){order+='n';return '\ud800';}},{toString:function(){order+='q';return 'p:Mixed';}},null);if(order!=='nq'||d.documentElement.namespaceURI.charCodeAt(0)!==0xd800||d.documentElement.ownerDocument!==d)throw 'ordered exact factory URI';
var calls=0,caught=false;try{make.call({}, {toString:function(){calls++;return 'urn:original';}},'Mixed');}catch(e){caught=e instanceof TypeError;}if(!caught||calls)throw 'brand before conversion';
caught=false;try{document.implementation.createDocument(null,'p:Mixed');}catch(e){caught=e.name==='NamespaceError';}if(!caught)throw 'prefix requires namespace';
caught=false;try{document.implementation.createDocument('urn:original','Mixed',document.body);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'doctype argument brand';
var actual=document.doctype;caught=false;try{document.implementation.createDocument('urn:original','Mixed',actual);}catch(e){caught=e.name==='NotSupportedError';}if(!caught||document.doctype!==actual||actual.parentNode!==document||actual.ownerDocument!==document)throw 'unimplemented adoption has no prefix';
"#);no_effects(&mut page);
}
#[test]
fn native_xml_document_factory_retained_refusal_has_no_published_identity_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();let before=(doc.nodes.len(),page.state.retained,page.state.dom_commits);let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.implementation.createDocument('urn:original','p:Mixed');").errors,1);assert_eq!((doc.nodes.len(),page.state.retained,page.state.dom_commits),before);page.state.retained_limit=limit;
    run(&mut doc,&mut forms,&mut page,"var made=document.implementation.createDocument('urn:original','p:Mixed');if(!(made instanceof XMLDocument)||made.documentElement.ownerDocument!==made||made.documentElement.namespaceURI!=='urn:original'||made.documentElement.localName!=='Mixed'||made.body!==null)throw 'fresh job XML recovery';");no_effects(&mut page);
}

#[test]
fn native_xml_document_head_body_and_title_read_current_owned_nodes_after_mutation(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var ns='http://www.w3.org/1999/xhtml',d=document.implementation.createDocument(ns,'html'),root=d.documentElement,head=d.createElementNS(ns,'head'),body=d.createElementNS(ns,'body'),title=d.createElementNS(ns,'title'),text=d.createTextNode(' \t\ud800  label\n ');
if(d.head!==null||d.body!==null||d.title!=='')throw 'actual initially absent nodes';root.appendChild(head);root.appendChild(body);head.appendChild(title);title.appendChild(text);if(d.head!==head||d.body!==body||d.title.length!==7||d.title.charCodeAt(0)!==0xd800||d.title.charAt(1)!==' '||d.title.charAt(6)!=='l')throw 'current XML HTML metadata';
text.data=' updated   title ';if(d.title!=='updated title')throw 'current changed title';var ignored=d.createElementNS(ns,'span');ignored.appendChild(d.createTextNode('not direct'));title.appendChild(ignored);if(d.title!=='updated title')throw 'title uses child Text rather than nested text';
root.removeChild(head);root.removeChild(body);if(d.head!==null||d.body!==null||d.title!==''||head.parentNode!==null||title.ownerDocument!==d)throw 'current removal and retained owners';
var factory=document.implementation.createHTMLDocument('initial'),saved=factory.head.firstChild.firstChild;saved.data=' actual   changed ';if(factory.title!=='actual changed')throw 'HTML factory uses same native title source';
"#);no_effects(&mut page);
}
