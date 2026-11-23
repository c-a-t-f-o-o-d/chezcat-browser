//! Exact charset metadata follows genuine inert owners without activation.
use super::*;

fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><head></head><body></body>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(&crate::navigation::address("https://inert-charset.test/",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{}",page.report());}

#[test]
fn inert_charset_factory_and_parsed_owners_keep_exact_attrs_clones_and_records(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var html=document.implementation.createHTMLDocument('metadata'),xml=document.implementation.createDocument(null,'Root'),parsed=new DOMParser().parseFromString('<script id=s charset=seed></script><link id=l charset=link-seed>','text/html');
var nodes=[html.createElement('script'),html.createElement('link'),xml.createElementNS('http://www.w3.org/1999/xhtml','script'),xml.createElementNS('http://www.w3.org/1999/xhtml','link'),parsed.getElementById('s'),parsed.getElementById('l')];
for(var node of nodes){
var owner=node.ownerDocument,connected=node.isConnected,proto=node.localName==='script'?HTMLScriptElement.prototype:HTMLLinkElement.prototype,saved=Object.getOwnPropertyDescriptor(proto,'charset');
saved.set.call(node,'seed');var attr=node.getAttributeNode('charset'),observer=new MutationObserver(function(){});observer.observe(node,{attributes:true,attributeOldValue:true,attributeFilter:['charset']});
var calls=0;node.charset={toString:function(){calls++;node.setAttribute('charset','during');return '\ud800\ufffd\udfff';}};var records=observer.takeRecords();
if(calls!==1||node.charset!=='\ud800\ufffd\udfff'||attr.value!==node.charset||records.length!==2||records[0].oldValue!=='seed'||records[1].oldValue!=='during'||node.ownerDocument!==owner||node.isConnected!==connected)throw 'inert exact owner conversion';
var clone=node.cloneNode(false);if(clone.ownerDocument!==owner||clone.charset!==node.charset||clone===node||clone.getAttributeNode('charset')===attr)throw 'inert clone identity';clone.charset='copy';if(node.charset===clone.charset)throw 'independent clone metadata';
attr.value='\udfff';var replacement=owner.createAttribute('charset');replacement.value='\ud800';node.setAttributeNode(replacement);records=observer.takeRecords();
if(records.length!==2||records[0].oldValue!=='\ud800\ufffd\udfff'||records[1].oldValue!=='\udfff'||attr.ownerElement!==null||replacement.ownerElement!==node||node.charset!=='\ud800')throw 'inert Attr replacement';
node.removeAttributeNode(replacement);records=observer.takeRecords();if(node.charset!==''||replacement.ownerElement!==null||records.length!==1||records[0].oldValue!=='\ud800')throw 'inert metadata removal';observer.disconnect();
}
var caseNode=nodes[2];caseNode.setAttribute('CHARSET','upper');caseNode.charset='lower';if(caseNode.getAttribute('CHARSET')!=='upper'||caseNode.getAttribute('charset')!=='lower')throw 'XML owner exact attribute case';
"#);
    assert!(page.state.dynamic.pending.is_empty());assert!(page.state.stylesheets.pending.is_empty());
}

#[test]
fn inert_charset_saved_child_caller_checks_origin_before_conversion(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var owner=document.implementation.createHTMLDocument('owner'),s=owner.createElement('script'),l=owner.createElement('link'),calls=0;
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var child=f.contentWindow;
var putScript=child.Function('node','value',"Object.getOwnPropertyDescriptor(HTMLScriptElement.prototype,'charset').set.call(node,value);"),putLink=child.Function('node','value',"Object.getOwnPropertyDescriptor(HTMLLinkElement.prototype,'charset').set.call(node,value);");
var value={toString:function(){calls++;return '\ud800';}};putScript(s,value);putLink(l,value);if(calls!==2||s.charset!=='\ud800'||l.charset!=='\ud800')throw 'genuine inert owner accessor';
"#);
    page.set_location(&crate::navigation::address("https://other-inert-charset.test/",&crate::navigation::Location::home()).unwrap());
    for source in ["putScript(s,value);","putLink(l,value);"]{assert_eq!(page.console_eval(&mut doc,&mut forms,source).errors,1);assert!(page.report().contains("same-origin"),"{}",page.report());}
    run(&mut doc,&mut forms,&mut page,"if(calls!==2||s.charset!=='\\ud800'||l.charset!=='\\ud800'||s.ownerDocument!==owner||l.ownerDocument!==owner)throw 'inert denied conversion changed owner';");
}


#[test]
fn inert_charset_native_backing_output_refusal_and_recovery_preserve_actual_snapshot(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var owner=document.implementation.createHTMLDocument('owner'),node=owner.createElement('script');node.setAttribute('charset','seed');var held=node.getAttributeNode('charset'),clone=node.cloneNode(false);");
    let Value::Host(object)=page.runtime.execute("node",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap()else{panic!("genuine node")};
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let before=(b.state.retained,b.state.inert_documents.backing_bytes(),b.state.inert_documents.generated.len(),b.state.attributes.len(),b.state.dom_commits);
    let attached=b.state.attached_attributes.clone();let(limit,copies)=(b.state.retained_limit,b.state.copy_work);let raw=Value::StringUnits(vec![0xd800,0xfffd,0xdc00]);
    for storage in [true,false]{
        if storage{b.state.retained_limit=before.0;}else{b.state.copy_work=0;}
        assert!(b.charset_reflection_set(object,&raw).is_err());
        assert_eq!((b.state.retained,b.state.inert_documents.backing_bytes(),b.state.inert_documents.generated.len(),b.state.attributes.len(),b.state.dom_commits),before);
        assert_eq!(b.state.attached_attributes,attached);assert_eq!(b.inert_element_attributes(object).unwrap().iter().find(|attr|attr.name.html_is("charset")).unwrap().value.scalar().unwrap(),"seed");
        b.state.retained_limit=limit;b.state.copy_work=copies;
    }
    b.charset_reflection_set(object,&raw).unwrap();
    let snapshot=&b.state.inert_documents.generated[before.2].node;
    let NodeKind::Element{tag,href,..}=&snapshot.kind else{panic!("real source")};
    let payload=node_bytes(snapshot)+tag.capacity()+href.as_ref().map_or(0,String::capacity)+size_of::<Node>()+2*size_of::<usize>();
    assert_eq!(b.state.retained,before.0+payload+b.state.inert_documents.backing_bytes()-before.1);
    let after=(b.state.retained,b.state.dom_commits,b.state.inert_documents.generated.len());let bytes=b.state.limits.host.string_bytes;
    b.state.limits.host.string_bytes=5;assert!(b.charset_reflection_get(object).unwrap_err().contains("allowance"));b.state.limits.host.string_bytes=bytes;
    b.state.copy_work=0;assert!(b.charset_reflection_get(object).unwrap_err().contains("string work limit"));b.state.copy_work=copies;
    assert!(matches!(b.charset_reflection_get(object).unwrap(),Value::StringUnits(units) if units==[0xd800,0xfffd,0xdc00]));
    assert_eq!((b.state.retained,b.state.dom_commits,b.state.inert_documents.generated.len()),after);assert_eq!(b.state.attached_attributes,attached);
    assert!(b.state.dynamic.pending.is_empty());assert!(b.state.stylesheets.pending.is_empty());drop(b);
    run(&mut doc,&mut forms,&mut page,r#"if(node.charset!==held.value||held.value!=='\ud800\ufffd\udc00'||clone.charset!=='seed'||node.ownerDocument!==owner||clone.ownerDocument!==owner)throw 'recovered actual inert snapshot';"#);
}
