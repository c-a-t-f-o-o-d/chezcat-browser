//! Original live collection identity, filter backing and enumeration acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse(markup);doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
#[test]
fn namespace_collection_exact_uri_local_name_wildcards_and_nullable_arguments() {
    let (mut doc, mut forms, mut page) = page("<!doctype html><html><body></body></html>");
    execute(&mut page, &mut doc, &mut forms, r#"
var xml=document.implementation.createDocument(null,'Root'),root=xml.documentElement;
var a=xml.createElementNS('urn:A','p:Mixed'),b=xml.createElementNS(null,'Mixed');
var raw=xml.createElementNS('urn:\ud800','Mixed'),replacement=xml.createElementNS('urn:\ufffd','Mixed');
var spelled=xml.createElementNS(null,'Q\ufffd');
for(var node of [a,b,raw,replacement,spelled])root.appendChild(node);
var named=Document.prototype.getElementsByTagNameNS,scoped=Element.prototype.getElementsByTagNameNS;
if(named===scoped||named.name!=='getElementsByTagNameNS'||named.length!==2||scoped.length!==2)throw 'IDL methods';
if(xml.getElementsByTagNameNS('*','*').length!==6||root.getElementsByTagNameNS('*','*').length!==5)throw 'descendant scope';
if(named.call(xml,'urn:A','Mixed')[0]!==a||scoped.call(root,'urn:A','mixed').length||
   scoped.call(root,'urn:A','p:Mixed').length||xml.getElementsByTagNameNS('urn:a','*').length)throw 'local versus qualified spelling';
if(root.getElementsByTagNameNS(null,'Mixed')[0]!==b||root.getElementsByTagNameNS('','Mixed')[0]!==b||
   root.getElementsByTagNameNS(undefined,'Mixed')[0]!==b||xml.getElementsByTagNameNS(null,'Root')[0]!==root)throw 'null namespace';
if(root.getElementsByTagNameNS('*','Mixed').length!==4||root.getElementsByTagNameNS('urn:A','*').length!==1||
   root.getElementsByTagNameNS('urn:\ud800','Mixed')[0]!==raw||root.getElementsByTagNameNS('urn:\ufffd','Mixed')[0]!==replacement||
   root.getElementsByTagNameNS(null,'Q\ud800').length||root.getElementsByTagNameNS(null,'Q\ufffd')[0]!==spelled)throw 'raw units';
var effects='',ns={toString:function(){effects+='n';return 'urn:A';}},local={toString:function(){effects+='l';return 'Mixed';}};
var bad=0;try{named.call(root,ns,local);}catch(e){if(e instanceof TypeError)bad++;}
try{scoped.call(xml,ns,local);}catch(e){if(e instanceof TypeError)bad++;}
try{scoped.call(root,ns);}catch(e){if(e instanceof TypeError)bad++;}
if(bad!==3||effects!==''||scoped.call(root,ns,local)[0]!==a||effects!=='nl')throw 'brand, arity or conversion order';
var failed=false;
try{scoped.call(root,ns,Symbol());}catch(e){failed=e instanceof TypeError;}
if(!failed||effects!=='nln')throw 'second conversion refusal and earlier effect';
var desc=Object.getOwnPropertyDescriptor(Document.prototype,'getElementsByTagNameNS');
if(!desc.enumerable||!desc.writable||!desc.configurable||Object.hasOwn(xml,'getElementsByTagNameNS'))throw 'method descriptor';
Object.defineProperty(Document.prototype,'getElementsByTagNameNS',{value:function(){return 17;},configurable:true});
if(xml.getElementsByTagNameNS()!==17||named.call(xml,'urn:A','Mixed')[0]!==a)throw 'author override versus saved native query';
delete Document.prototype.getElementsByTagNameNS;
if(typeof xml.getElementsByTagNameNS!=='undefined')throw 'deleted prototype method resurrected';
Object.defineProperty(Document.prototype,'getElementsByTagNameNS',desc);
"#);
}

#[test]
fn namespace_collection_live_order_adoption_template_separation_and_named_controls() {
    let (mut doc, mut forms, mut page) = page("<!doctype html><html><body><form id='f'><input id='field' name='getElementsByTagNameNS'></form><template><i id='hidden'></i></template></body></html>");
    execute(&mut page, &mut doc, &mut forms, r#"
var html='http://www.w3.org/1999/xhtml',form=document.getElementById('f'),field=document.getElementById('field');
if(form.getElementsByTagNameNS!==field||Element.prototype.getElementsByTagNameNS.call(form,html,'input')[0]!==field||
   document.getElementsByTagNameNS(html,'i').length||document.getElementsByTagNameNS(html,'INPUT').length)throw 'named form and HTML casing';
var x=document.implementation.createDocument('urn:r','r:root'),y=document.implementation.createDocument(null,'second');
var section=x.createElementNS(null,'section');x.documentElement.appendChild(section);
var one=x.createElementNS('urn:a','p:item'),two=x.createElementNS('urn:a','q:item');
one.setAttribute('id','one');var items=section.getElementsByTagNameNS('urn:a','item');
section.appendChild(one);section.insertBefore(two,one);
if(items.length!==2||items.item(0)!==two||items[1]!==one||items.namedItem('one')!==one||items.item(2)!==null)throw 'live order';
y.documentElement.appendChild(y.adoptNode(section));
if(x.getElementsByTagNameNS('urn:a','item').length||y.getElementsByTagNameNS('urn:a','item')[0]!==two||
   section.ownerDocument!==y||items[1]!==one)throw 'owner transfer';
section.removeChild(two);if(items.length!==1||items[0]!==one)throw 'live removal';
"#);
}

#[test]
fn namespace_collection_storage_refusal_keeps_registry_and_recovers() {
    let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
    let retained = page.state.retained;
    let limit = page.state.retained_limit;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let args = [Value::String("urn:test".into()), Value::String("local".into())];
    let string_limit = bridge.state.limits.host.string_bytes;
    bridge.state.limits.host.string_bytes = 4;
    assert!(bridge.create_namespace_collection(DOCUMENT, &args).is_err());
    bridge.state.limits.host.string_bytes = string_limit;
    bridge.state.retained_limit = retained + 3;
    assert!(bridge.create_namespace_collection(DOCUMENT, &args).is_err());
    assert!(bridge.state.dom.collections.is_empty());
    assert_eq!(bridge.state.retained, retained);
    bridge.state.retained_limit = limit;
    let Value::Host(handle) = bridge.create_namespace_collection(DOCUMENT, &args).unwrap() else { unreachable!() };
    let records = &bridge.state.dom.collections;
    assert_eq!(bridge.state.retained, retained + records.capacity()*size_of::<TagCollection>() + records[0].filter.owned_bytes());
    assert_eq!(bridge.tag_collection_get(handle,"length").unwrap().to_number(), 0.0);
}

#[test]
fn class_collection_document_modes_fold_only_ascii_and_preserve_raw_tokens() {
    for (doctype, fold) in [
        ("", true),
        ("<!doctype html>", false),
        ("<!doctype html PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN' 'original'>", false),
    ] {
        let (mut doc, mut forms, mut page) = page(&format!("{doctype}<html><body><div id='root'><i id='a' class='seed'></i><i id='b'></i></div></body></html>"));
        // Supply the native record directly. Public mutation of raw class
        // values on a main-document element still has separate CSS consumers.
        let target = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("a")).unwrap();
        let NodeKind::Element { attrs, .. } = &mut doc.nodes[target].kind else { unreachable!() };
        let mut units: Vec<_> = "UP\u{a0}X\tA".encode_utf16().collect();
        units.push(0xd800);
        units.extend("z É".encode_utf16());
        attrs.iter_mut().find(|attr| attr.name.html_is("class")).unwrap().value = DomString::from_units(&units, 128).unwrap();
        let script = r#"
var root=document.getElementById('root'), a=document.getElementById('a'), b=document.getElementById('b');
b.setAttribute('class','UP\u00a0X A\ufffdz é');
var raw=root.getElementsByClassName('A\ud800z UP\u00a0X A\ud800z');
var lossy=document.getElementsByClassName('A\ufffdz');
if(raw.length!==1 || raw[0]!==a || lossy.length!==1 || lossy[0]!==b) throw 'exact code units';
if(root.getElementsByClassName('UP X').length || root.getElementsByClassName('é')[0]!==b) throw 'ASCII boundaries';
var folded=root.getElementsByClassName('a\ud800Z');
if(folded.length!==(FOLD?1:0)) throw 'node document mode';
var copy=a.cloneNode(false);root.insertBefore(copy,a);b.setAttribute('class','changed');
if(raw.length!==2 || raw[0]!==copy || raw[1]!==a || lossy.length!==0) throw 'live order and mutation';
"#.replace("FOLD", if fold { "true" } else { "false" });
        execute(&mut page, &mut doc, &mut forms, &script);
    }
}

#[test]
fn class_collection_inert_owners_and_adoption_follow_the_current_document_mode() {
    let (mut doc, mut forms, mut page) = page("<!doctype html><html><body></body></html>");
    execute(&mut page, &mut doc, &mut forms, r#"
var parsed=new DOMParser().parseFromString('<html><body><section><b></b></section></body></html>', 'text/html');
var root=parsed.body.firstChild, child=root.firstChild;
child.setAttribute('class','A\udfff'); var view=root.getElementsByClassName('a\udfff');
if(parsed.compatMode!=='BackCompat' || view.length!==1 || view[0]!==child) throw 'inert quirks root';
var html=document.implementation.createHTMLDocument('standards');
html.body.appendChild(html.adoptNode(root));
if(root.ownerDocument!==html || child.ownerDocument!==html || view.length!==0) throw 'saved collection after adoption';
child.setAttribute('class','a\udfff');
if(view.length!==1 || view[0]!==child || html.getElementsByClassName('a\ufffd').length) throw 'adopted raw match';
"#);
}

#[test]
fn browser_live_tag_and_class_collections_exceed_old_identity_range_and_keep_actual_live_behavior() {
    let(mut doc,mut forms,mut page)=page("<html><body><div id=one class='red blue'></div><div id=two class=red></div></body></html>");
    let baseline=page.state.retained;
    execute(&mut page,&mut doc,&mut forms,"var tags,classes;for(var i=0;i<70000;i++)tags=document.getElementsByTagName('DIV');for(var i=0;i<70000;i++)classes=document.getElementsByClassName('red red\\tblue');if(tags.length!==2||tags.item(1)!==document.getElementById('two')||tags.namedItem('one')!==classes[0]||classes.length!==1)throw 'large live registries';");
    assert_eq!(page.state.dom.collections.len(),140000);
    let payload=page.state.dom.collections.iter().map(|collection|collection.filter.owned_bytes()).sum::<usize>();
    assert_eq!(page.state.retained,baseline+payload+page.state.dom.collections.capacity()*size_of::<TagCollection>());
    execute(&mut page,&mut doc,&mut forms,"var node=document.getElementById('two');node.className='blue red';if(classes.length!==2||classes[1]!==node)throw 'live class update';document.body.removeChild(node);if(classes.length!==1||tags.length!==1)throw 'live topology update';");
    for handle in [256,512,NODE_BASE,NODE_LIST_BASE,COLLECTION_BASE-1,COLLECTION_END] {assert!(collection_handle_index(handle).is_none());}
    assert_eq!(collection_handle_index(COLLECTION_BASE+139999),Some(139999));
    assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_live_collection_registry_and_filter_joint_admission_are_atomic_and_exact() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p></body></html>");
    let baseline=page.state.retained;let limit=page.state.retained_limit;let slot=size_of::<TagCollection>();
    let value=Value::String("p".into());page.state.retained_limit=baseline+slot;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).is_err());
    assert_eq!(page.state.dom.collections.capacity(),0);assert_eq!(page.state.retained,baseline);
    page.state.retained_limit+=1;page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).unwrap();
    assert_eq!(page.state.dom.collections.capacity(),1);assert_eq!(page.state.retained,baseline+slot+1);
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work=slot;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.dom.collections.len(),1);assert_eq!(page.state.dom.collections.capacity(),1);assert_eq!(page.state.retained,baseline+slot+1);
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).unwrap();
    assert_eq!(page.state.dom.collections.len(),2);
}
#[test]
fn browser_class_filter_staging_refusal_preserves_registry_and_recovers_with_deduplicated_owned_names() {
    let(mut doc,mut forms,mut page)=page("<html><body><p class='a b'></p></body></html>");
    let baseline=page.state.retained;let limit=page.state.retained_limit;let value=Value::String("b a b".into());
    page.state.retained_limit=baseline+4;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_class_collection(DOCUMENT,&value).unwrap_err().contains("class-filter scratch"));
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.dom.collections.capacity(),0);
    page.state.retained_limit=limit;page.begin_job();
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_class_collection(DOCUMENT,&value).unwrap() else{panic!("Expected collection")};
    let Filter::Classes(names)=&page.state.dom.collections[0].filter else{panic!("Expected class filter")};assert_eq!(names, &[vec![0x61], vec![0x62]]);
    let payload=names.capacity()*size_of::<Vec<u16>>()+names.iter().map(|name| name.capacity()*size_of::<u16>()).sum::<usize>();
    assert_eq!(page.state.retained,baseline+payload+page.state.dom.collections.capacity()*size_of::<TagCollection>());
    assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.tag_collection_get(handle,"length").unwrap().to_number(),1.0);
}
#[test]
fn browser_live_collection_enumerates_six_thousand_indices_and_refuses_key_storage_atomically() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<p></p>".repeat(6000)));
    execute(&mut page,&mut doc,&mut forms,"var collection=document.getElementsByTagName('p'),keys=Object.keys(collection);if(keys.length!==6000||keys[0]!=='0'||keys[5999]!=='5999')throw 'real index enumeration';");
    let baseline=page.state.retained;let limit=page.state.retained_limit;page.state.retained_limit=baseline;
    page.begin_job();assert!(page.state.index_keys(6000).unwrap_err().contains("collection key allowance"));assert_eq!(page.state.retained,baseline);
    page.state.retained_limit=limit;page.begin_job();let keys=page.state.index_keys(6000).unwrap();assert_eq!(keys.len(),6000);assert_eq!(keys[5999],"5999");
    assert_eq!(page.state.retained,baseline);
}
