//! Exact native CharacterData units are independent of scalar display text.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<html><body><div id=mount></div><input id=field value=seed></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    page.set_location(&crate::navigation::address("https://native-units.test/source",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){
    assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());
}

#[test]
fn native_node_value_descriptor_reads_current_owners_and_preserves_nondata_children() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var desc=Object.getOwnPropertyDescriptor(Node.prototype,'nodeValue'),reads=0;
if(!desc.enumerable||!desc.configurable||desc.get.length||desc.set.length!==1||!('nodeValue' in document))throw 'native Node descriptor';
var el=document.createElement('div'),child=document.createTextNode('child'),frag=document.createDocumentFragment();
el.appendChild(child);frag.appendChild(el);
var xml=document.implementation.createDocument(null,'Root'), dt=document.implementation.createDocumentType('example','','');
for(var n of [document,xml,xml.documentElement,dt,el,frag]){
 if(n.nodeValue!==null||desc.get.call(n)!==null||Object.hasOwn(n,'nodeValue'))throw 'null native value';
 desc.set.call(n,{toString:function(){reads++;return 'unused';}});
}
if(reads!==6||el.firstChild!==child||child.data!=='child'||frag.firstChild!==el)throw 'Web IDL conversion before no-op';
var wrong=false;try{desc.set.call({}, {toString:function(){reads++;return 'bad';}});}catch(e){wrong=e instanceof TypeError;}
if(!wrong||reads!==6)throw 'wrong brand conversion';
var attr=el.ownerDocument.createAttribute('data-node'),text=xml.createTextNode('s'),cdata=xml.createCDATASection('c'),comment=document.createComment('c'),pi=xml.createProcessingInstruction('p','d');
el.setAttributeNode(attr);
for(var v of [attr,child,text,cdata,comment,pi]){desc.set.call(v,'\ud800x');if(desc.get.call(v)!=='\ud800x'||v.nodeValue!=='\ud800x')throw 'raw native data';v.nodeValue=null;if(desc.get.call(v)!=='')throw 'nullable data';}
if(el.getAttribute('data-node')!==''||attr.ownerElement!==el)throw 'native attached Attr';
Object.defineProperty(Node.prototype,'nodeValue',{value:42,configurable:true});
if(document.nodeValue!==42||desc.get.call(document)!==null||desc.get.call(child)!=='')throw 'overridden descriptor';
Object.defineProperty(Node.prototype,'nodeValue',desc);
"#);
}

#[test]
fn native_dom_string_character_factories_aliases_observer_old_values_and_range_are_exact(){
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var mount=document.getElementById('mount'),get=Object.getOwnPropertyDescriptor(Node.prototype,'textContent').get,set=Object.getOwnPropertyDescriptor(Node.prototype,'textContent').set,calls=0;
var nodes=[document.createTextNode('\ud800A\udfff'),document.createComment('\ud800A\udfff'),document.createProcessingInstruction('original','\ud800A\udfff'),new Text('\ud800A\udfff'),new Comment('\ud800A\udfff')];
for(var node of nodes){if(node.data.length!==3||node.data.charCodeAt(0)!==0xd800||node.data.charCodeAt(2)!==0xdfff||node.nodeValue!==node.data||get.call(node)!==node.data||node.length!==3)throw 'actual constructor units';var observer=new MutationObserver(function(){});observer.observe(node,{characterData:true,characterDataOldValue:true});node.data={toString:function(){calls++;return '\udfffB\ud800';}};var records=observer.takeRecords();if(records.length!==1||records[0].target!==node||records[0].oldValue.charCodeAt(0)!==0xd800||node.data.charCodeAt(0)!==0xdfff||node.nodeValue.charCodeAt(2)!==0xd800)throw 'exact old and new data';set.call(node,'\ud800');if(node.data.length!==1||node.data.charCodeAt(0)!==0xd800)throw 'borrowed native setter';observer.disconnect();var copy=node.cloneNode(true);if(copy===node||copy.data!==node.data||copy.ownerDocument!==document)throw 'clone code units';}
if(calls!==nodes.length)throw 'one conversion';var text=nodes[0];mount.appendChild(text);var range=new Range();range.setStart(text,1);range.setEnd(text,1);text.nodeValue='\udfff';if(text.data.charCodeAt(0)!==0xdfff||range.startContainer!==text||range.startOffset||range.endOffset)throw 'actual replacement Range';var snapshot=text.data;text.data=null;if(text.data!=='null')throw 'nonnullable data';text.nodeValue=null;if(text.data!=='')throw 'nullable nodeValue';text.textContent=snapshot;if(text.data.charCodeAt(0)!==0xdfff)throw 'textContent alias';
"#);
}
#[test]
fn native_dom_string_inert_created_cloned_imported_promoted_and_normalized_keep_units(){
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template');t.innerHTML='<unit-card>seed</unit-card>';var root=t.content.firstChild,owner=root.ownerDocument;root.textContent='\ud800';var saved=root.firstChild,copy=saved.cloneNode(true),made=owner.createTextNode('\udfff'),imported=document.importNode(root,true);if(saved.data.charCodeAt(0)!==0xd800||copy.data!==saved.data||copy.ownerDocument!==owner||made.data.charCodeAt(0)!==0xdfff||imported.firstChild.data!==saved.data||imported.ownerDocument!==document)throw 'actual inert copies';root.appendChild(made);document.getElementById('mount').appendChild(t.content);if(root.firstChild!==saved||saved.ownerDocument!==document||made.ownerDocument!==document||root.textContent.charCodeAt(0)!==0xd800||root.textContent.charCodeAt(1)!==0xdfff)throw 'promoted aliases';root.normalize();if(root.firstChild!==saved||saved.data.length!==2||saved.data.charCodeAt(0)!==0xd800||saved.data.charCodeAt(1)!==0xdfff||made.parentNode!==null||made.data.charCodeAt(0)!==0xdfff)throw 'normalized code units and retained removed data';
var parent=document.createElement('span'),high=document.createTextNode('\ud83d'),low=document.createTextNode('\ude00');parent.appendChild(high);parent.appendChild(low);document.body.appendChild(parent);var range=new Range();range.setStart(low,1);range.setEnd(low,1);if(parent.textContent!=='😀')throw 'joined getter pair';parent.normalize();if(parent.firstChild!==high||parent.childNodes.length!==1||high.data!=='😀'||range.startContainer!==high||range.startOffset!==2||low.data.charCodeAt(0)!==0xde00)throw 'pair normalization and live Range';
"#);
}
#[test]
fn native_dom_string_storage_refusal_has_no_native_or_observer_prefix_and_recovers(){
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var text=document.createTextNode('x'),observer=new MutationObserver(function(){});observer.observe(text,{characterData:true,characterDataOldValue:true});");
    let before=(doc.nodes.len(),page.state.dom_commits,page.state.retained);
    let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained+3;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"text.data='\\ud800';").errors,1);
    assert_eq!((doc.nodes.len(),page.state.dom_commits,page.state.retained),before);
    page.state.retained_limit=limit;
    run(&mut doc,&mut forms,&mut page,"if(text.data!=='x'||observer.takeRecords().length)throw 'failed code-unit publication';text.data='\\ud800';var record=observer.takeRecords();if(text.data.charCodeAt(0)!==0xd800||record.length!==1||record[0].oldValue!=='x')throw 'fresh job code units';text.data='z';record=observer.takeRecords();if(record[0].oldValue.charCodeAt(0)!==0xd800||text.data!=='z')throw 'shrinking raw storage';");
    assert_eq!(page.state.retained,before.2);
}

#[test]
fn native_dom_string_unreserved_inert_import_charges_all_owned_code_units_and_projection(){
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template'),owner=t.content.ownerDocument;");
    let limit=page.state.node_limit;page.state.node_limit=doc.nodes.len();
    run(&mut doc,&mut forms,&mut page,"var unreserved=owner.createTextNode('\\ud800');");
    page.state.node_limit=limit;
    let Value::Host(source)=page.runtime.execute("unreserved;",&mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap() else {panic!("real source")};
    let backing=|bridge:&Bridge<'_>|bridge.doc.nodes.capacity()*size_of::<Node>()+bridge.state.node_state_bytes()+bridge.state.node_sets_storage_bytes();
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let before=bridge.state.retained;let previous=backing(&bridge);
    let Value::Host(copy)=bridge.import_node(&[Value::Host(source),Value::Bool(true)],None).unwrap() else {panic!("real imported text")};
    let node=node_index(copy).unwrap();let NodeKind::Text(data)=&bridge.doc.nodes[node].kind else {panic!("native text")};
    assert_eq!(data.raw_units(),Some(&[0xd800][..]));
    assert_eq!(bridge.state.retained-before,data.capacity()+backing(&bridge)-previous);
    drop(bridge);
    run(&mut doc,&mut forms,&mut page,"var instruction=owner.createProcessingInstruction('original','\\udfff'),copy=document.importNode(instruction,true);if(copy.data!==instruction.data||copy.data.charCodeAt(0)!==0xdfff||copy.target!=='original'||copy.ownerDocument!==document||instruction.ownerDocument!==owner)throw 'real instruction copy';");
}

#[test]
fn native_dom_string_attributes_alias_observe_detach_transfer_clone_import_and_promote(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var mount=document.getElementById('mount'),root=document.createElement('unit-card'),calls=0;
root.setAttribute('data-original',{toString:function(){calls++;return '\ud800A';}});
var attr=root.getAttributeNode('data-original'),list=root.attributes,observer=new MutationObserver(function(){});
observer.observe(root,{attributes:true,attributeOldValue:true});
if(calls!==1||attr.value.charCodeAt(0)!==0xd800||root.getAttribute('data-original')!==attr.value||root.getAttributeNS(null,'data-original')!==attr.value||root.dataset.original!==attr.value)throw 'actual Attr DOMString';if(root.matches('[data-original=\"�A\"]')||!root.matches('[data-original]')||!root.matches('[data-original$=A]')||!root.matches('[data-original*=A]'))throw 'selectors compare exact native units';
attr.nodeValue='\udfffB';var records=observer.takeRecords();if(records.length!==1||records[0].oldValue.charCodeAt(0)!==0xd800||records[0].target!==root||list[0]!==attr||attr.textContent.charCodeAt(0)!==0xdfff)throw 'exact attached alias and observation';
var copy=root.cloneNode(true),imported=document.importNode(root,true);if(copy.getAttribute('data-original')!==attr.value||imported.getAttributeNode('data-original').value!==attr.value)throw 'actual attribute copies';
root.removeAttribute('data-original');records=observer.takeRecords();if(attr.ownerElement!==null||attr.value.charCodeAt(0)!==0xdfff||records.length!==1||records[0].oldValue!==attr.value)throw 'retained detached value';attr.textContent='\ud800';root.setAttributeNode(attr);if(attr.ownerElement!==root||root.getAttribute('data-original').charCodeAt(0)!==0xd800||root.attributes[0]!==attr)throw 'real attachment';
var t=document.createElement('template'),owner=t.content.ownerDocument,inert=owner.createElement('unit-card');t.content.appendChild(inert);inert.setAttribute('data-original','\udfff');var saved=inert.getAttributeNode('data-original');var io=new MutationObserver(function(){});io.observe(inert,{attributes:true,attributeOldValue:true});saved.value='\ud800';records=io.takeRecords();if(saved.ownerDocument!==owner||inert.getAttributeNS(null,'data-original').charCodeAt(0)!==0xd800||records[0].oldValue.charCodeAt(0)!==0xdfff)throw 'inert exact aliases';var inertCopy=inert.cloneNode(true),mainCopy=document.importNode(inert,true);if(inertCopy.getAttribute('data-original')!==saved.value||mainCopy.getAttribute('data-original')!==saved.value)throw 'inert exact copies';mount.appendChild(t.content);if(saved.ownerDocument!==document||saved.ownerElement!==inert||inert.getAttribute('data-original').charCodeAt(0)!==0xd800)throw 'promoted actual Attr';
"#);
}
#[test]
fn native_dom_string_attribute_storage_refusal_and_active_consumer_refusal_are_atomic(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var root=document.createElement('span');root.setAttribute('data-original','x');var attr=root.getAttributeNode('data-original'),observer=new MutationObserver(function(){});observer.observe(root,{attributes:true,attributeOldValue:true});");
    let before=(page.state.retained,page.state.dom_commits);let limit=page.state.retained_limit;
    page.state.retained_limit=before.0+3;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"attr.value='\\ud800';").errors,1);
    assert_eq!((page.state.retained,page.state.dom_commits),before);page.state.retained_limit=limit;
    run(&mut doc,&mut forms,&mut page,"if(attr.value!=='x'||observer.takeRecords().length)throw 'no publication';attr.value='\\ud800';if(observer.takeRecords()[0].oldValue!=='x')throw 'recovery';");
    assert_eq!(page.console_eval(&mut doc,&mut forms,"root.setAttribute('style','\\udfff');").errors,1);
    run(&mut doc,&mut forms,&mut page,"if(root.getAttribute('style')!==null||observer.takeRecords().length||attr.value.charCodeAt(0)!==0xd800)throw 'active guard kept native values';");
}

#[test]
fn native_dom_string_attribute_raw_search_repeated_prefix_unicode_and_ascii_case_are_exact(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"var n=document.createElement('span'),repeated='a';for(var i=0;i<17;i++)repeated+=repeated;n.setAttribute('data-search','\ud800'+repeated+'B😀');if(!n.matches('[data-search]')||!n.matches('[data-search*="aaaaB😀"]')||!n.matches('[data-search$="b😀" i]')||n.matches('[data-search^="�"]')||n.matches('[data-search*="aaaaC😀"]'))throw 'raw linear predicate';document.title='foundation-ok';"#);
}
