//! Genuine document records, native doctype identities and original negatives.
use super::*;
fn fixture(source:&str)->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document(source,&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    page.set_location(&crate::navigation::address("https://document-records.test/source",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}

#[test]
fn native_document_records_loaded_doctype_has_real_native_identity_getters_and_copies(){
    let(mut doc,mut forms,mut page)=fixture(" \n<!--before--><!dOcTyPe HtMl PUBLIC 'original-public' 'original-system'><html><head></head><body><p id=mount>kept</p></body></html>");
    assert_eq!(doc.kind,crate::html::DocumentKind::Html);assert_eq!(doc.mode,crate::html::DocumentMode::NoQuirks);
    run(&mut doc,&mut forms,&mut page,r#"
var dt=document.doctype,read=Object.getOwnPropertyDescriptor(Document.prototype,'doctype').get,name=Object.getOwnPropertyDescriptor(DocumentType.prototype,'name').get;
if(!(dt instanceof DocumentType)||!(dt instanceof Node)||Object.getPrototypeOf(dt)!==DocumentType.prototype||Object.getPrototypeOf(DocumentType.prototype)!==Node.prototype||read.call(document)!==dt||document.childNodes[1]!==dt||dt.previousSibling!==document.firstChild||dt.nextSibling!==document.documentElement||dt.parentNode!==document||dt.ownerDocument!==document||dt.nodeType!==10||dt.nodeName!=='html'||dt.name!=='html'||dt.publicId!=='original-public'||dt.systemId!=='original-system'||dt.firstChild!==null||dt.childNodes.length||dt.textContent!==null||dt.nodeValue!==null||!dt.isConnected)throw 'real loaded doctype';
var copy=dt.cloneNode(true),imported=document.importNode(dt,true);if(copy===dt||copy.parentNode!==null||copy.ownerDocument!==document||copy.isConnected||copy.name!==dt.name||copy.publicId!==dt.publicId||copy.systemId!==dt.systemId||imported===copy||imported.systemId!==dt.systemId)throw 'independent native copies';
var caught=false;try{name.call(document.getElementById('mount'));}catch(e){caught=e instanceof TypeError;}if(!caught||dt.name!=='html')throw 'native doctype getter brand';
copy.nodeValue='ignored';copy.textContent='ignored';copy.normalize();if(copy.nodeValue!==null||copy.textContent!==null||copy.childNodes.length)throw 'doctype is not CharacterData';
var parent=document.createElement('span'),fragment=document.createDocumentFragment();for(var destination of [parent,fragment]){caught=false;try{destination.appendChild(copy);}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||destination.childNodes.length||copy.parentNode!==null)throw 'doctype needs Document parent';}
"#);
}

#[test]
fn native_document_records_parser_modes_and_inert_doctype_payloads_are_real(){
    for (declaration,expected) in [
        ("","BackCompat"),("<!doctype html>","CSS1Compat"),
        ("<!doctype potato>","BackCompat"),
        ("<!doctype html PUBLIC '-//W3C//DTD XHTML 1.0 Transitional//EN'>","CSS1Compat"),
        ("<!doctype html PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN' ''>","BackCompat"),
        ("<!doctype html PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN' 'original-system'>","CSS1Compat"),
    ] {
        let markup=format!("{declaration}<html><head></head><body></body></html>");let(mut doc,mut forms,mut page)=fixture(&markup);
        run(&mut doc,&mut forms,&mut page,&format!("if(document.compatMode!=='{expected}')throw 'actual mode';"));
    }
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><html><head></head><body></body></html>");
    run(&mut doc,&mut forms,&mut page,r#"
var parser=new DOMParser(),d=parser.parseFromString("<!--before--><!doctype html PUBLIC '-//W3C//DTD XHTML 1.0 Transitional//EN' 'original-system'><html><head></head><body>kept</body></html>",'text/html'),dt=d.doctype;
if(d.compatMode!=='CSS1Compat'||dt.name!=='html'||dt.publicId!=='-//W3C//DTD XHTML 1.0 Transitional//EN'||dt.systemId!=='original-system'||dt.parentNode!==d||dt.ownerDocument!==d||dt.previousSibling!==d.firstChild||dt.nextSibling!==d.documentElement||d.querySelector('body')!==d.body||d.body.textContent!=='kept')throw 'native parsed inert document';
var copy=dt.cloneNode(true),imported=document.importNode(dt,true);if(copy===dt||copy.ownerDocument!==d||copy.publicId!==dt.publicId||imported.ownerDocument!==document||imported.publicId!==dt.publicId||imported.parentNode!==null)throw 'inert payload copies';
var factory=document.implementation.createHTMLDocument('original');if(factory.compatMode!=='CSS1Compat'||factory.doctype.name!=='html'||factory.doctype.publicId!==''||factory.doctype.systemId!==''||factory.doctype.ownerDocument!==factory||factory.doctype.cloneNode().ownerDocument!==factory)throw 'actual factory doctype';
"#);
}

#[test]
fn native_document_records_factory_preserves_domstrings_and_validates_current_name_rules(){
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><html><head></head><body></body></html>");
    run(&mut doc,&mut forms,&mut page,r#"
var make=DOMImplementation.prototype.createDocumentType,impl=document.implementation,log='';if(make.length!==3)throw 'required signature';
var dt=make.call(impl,{toString:function(){log+='n';return 'MiXeD\ud800';}},{toString:function(){log+='p';return '\udfff';}},{toString:function(){log+='s';return 'https://never-fetch.test/\ud800';}});
if(log!=='nps'||dt.name.length!==6||dt.name.charCodeAt(5)!==0xd800||dt.publicId.charCodeAt(0)!==0xdfff||dt.systemId.charCodeAt(dt.systemId.length-1)!==0xd800||dt.ownerDocument!==document||dt.parentNode!==null||dt.isConnected||dt.getRootNode()!==dt)throw 'actual native factory payload';
var empty=impl.createDocumentType('','','');if(empty.name!==''||empty.publicId!==''||empty.nodeName!=='')throw 'empty doctype name is valid';
log='';var caught=false;try{impl.createDocumentType({toString:function(){log+='n';return 'invalid name';}},{toString:function(){log+='p';return '';}},{toString:function(){log+='s';return '';}});}catch(e){caught=e.name==='InvalidCharacterError';}if(!caught||log!=='nps')throw 'ordered conversions before name validation';
log='';caught=false;try{make.call({}, {toString:function(){log+='bad';return '';}},'','');}catch(e){caught=e instanceof TypeError;}if(!caught||log!=='')throw 'brand before conversion';
var owner=impl.createHTMLDocument('owner'),inert=owner.implementation.createDocumentType('Original','\ud800','\udfff'),copy=inert.cloneNode(),imported=document.importNode(inert,true);if(inert.ownerDocument!==owner||copy.ownerDocument!==owner||copy.publicId.charCodeAt(0)!==0xd800||imported.ownerDocument!==document||imported.systemId.charCodeAt(0)!==0xdfff)throw 'associated inert implementation owner';
var element=document.createElement('span'),fragment=document.createDocumentFragment(),inertFragment=owner.createDocumentFragment();
for(var destination of [element,fragment,owner.body,inertFragment]){caught=false;try{destination.appendChild(inert);}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||inert.parentNode!==null)throw 'doctype hierarchy precedes adoption';}
for(var parent of [dt,inert]){caught=false;try{Node.prototype.appendChild.call(parent,element);}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||element.parentNode!==null||parent.childNodes.length)throw 'doctype cannot own children';}

"#);
    assert!(!page.has_navigation_request());let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
    assert!(page.take_dynamic_scripts().is_empty()&&page.take_dynamic_stylesheets().is_empty());
}

#[test]
fn native_document_records_factory_storage_refusal_has_no_identity_or_mutation_prefix_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><html><head></head><body></body></html>");
    let before=(doc.nodes.len(),page.state.retained,page.state.dom_commits);let limit=page.state.retained_limit;
    page.state.retained_limit=page.state.retained;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.implementation.createDocumentType('native','\\ud800','\\udfff');").errors,1);
    assert_eq!((doc.nodes.len(),page.state.retained,page.state.dom_commits),before);page.state.retained_limit=limit;
    run(&mut doc,&mut forms,&mut page,"var made=document.implementation.createDocumentType('native','\\ud800','\\udfff');if(made.name!=='native'||made.publicId.charCodeAt(0)!==0xd800||made.systemId.charCodeAt(0)!==0xdfff||made.ownerDocument!==document||made.parentNode!==null||document.doctype.name!=='html')throw 'fresh job recovery';");
}
