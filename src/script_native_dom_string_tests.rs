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
