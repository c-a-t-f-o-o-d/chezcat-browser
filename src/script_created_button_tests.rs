//! Original created BUTTON source-order, native state and atomicity controls.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><form id=f action='/result'><input id=neighbor name=neighbor value=seed><div id=host></div><button id=parsed type=button name=parsed value=before>Parsed caption</button></form><form id=g action='/other'><div id=destination></div></form><svg id=foreign></svg><template id=inert></template></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&current());
    page.set_resource_base(&current());
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn current() -> crate::navigation::Location {
    crate::navigation::address("https://created-button.test/source", &crate::navigation::Location::home()).unwrap()
}
fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node,"id") == Some(name)).unwrap()
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc,forms,source);
    assert_eq!(result.errors,0,"{source}\n{}",page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}
fn controls(forms: &Forms) -> Vec<(usize, Kind, String, String, String, Option<usize>, bool, (usize,usize,usize), bool)> {
    forms.controls.iter().map(|(&node,c)| (node,c.kind,c.name.clone(),c.value.text(),c.caption.clone(),c.owner,c.disabled,
        (c.value.cursor,c.value.anchor,c.value.offset),c.dirty_value)).collect()
}
fn query(doc: &Document, forms: &Forms, owner: &str, submitter: Option<usize>) -> Option<String> {
    forms.submit_request(doc,id(doc,owner),submitter,&current(),&current()).unwrap().location.url.unwrap().query
}

#[test]
fn created_button_vue_name_type_caption_then_insertion_has_one_native_model() {
    let (mut doc,mut forms,mut page) = fixture();
    let count = forms.controls.len();
    run(&mut doc,&mut forms,&mut page,r#"
var button=document.createElement('button');button.id='created';
button.setAttribute('name','increment');button.setAttribute('type','button');button.value='payload';
if(button.name!=='increment'||button.type!=='button'||button.value!=='payload'||button.form!==null)throw 'pending reflection';
button.appendChild(document.createTextNode('Increment original counter'));
var emphasis=document.createElement('span');emphasis.appendChild(document.createTextNode(' now'));button.appendChild(emphasis);
if(button.textContent!=='Increment original counter now'||button.parentNode!==null)throw 'ordinary pending caption';
"#);
    let node = id(&doc,"created");
    assert_eq!(forms.controls.len(),count,"Caption construction must not force early native admission");
    assert!(!forms.controls.contains_key(&node));
    run(&mut doc,&mut forms,&mut page,"document.getElementById('host').appendChild(button);if(button.form!==document.getElementById('f')||button.ownerDocument!==document||!button.isConnected)throw 'native owner';");
    let control = &forms.controls[&node];
    assert_eq!(control.kind,Kind::Button);
    assert_eq!(control.name,"increment");
    assert_eq!(control.value.text(),"payload");
    assert_eq!(control.caption,"Increment original counter now");
    assert_eq!(control.owner,Some(id(&doc,"f")));
    assert_eq!(query(&doc,&forms,"f",None).as_deref(),Some("neighbor=seed"));
    run(&mut doc,&mut forms,&mut page,"button.type='submit';");
    assert_eq!(forms.controls[&node].kind,Kind::Submit);
    assert_eq!(query(&doc,&forms,"f",Some(node)).as_deref(),Some("neighbor=seed&increment=payload"));
}

#[test]
fn created_button_reflection_attr_aliases_and_value_default_mode_agree() {
    let (mut doc,mut forms,mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var button=document.createElement('button');button.id='created';button.type='button';button.name='first';
var caption=document.createElement('span');caption.appendChild(document.createTextNode('Caption stays'));button.appendChild(caption);document.getElementById('host').appendChild(button);
var effects=0;
button.name={toString:function(){effects++;return 'renamed';}};
button.type={toString:function(){effects++;return 'ReSeT';}};
button.value={toString:function(){effects++;return 'a\r\nb';}};
if(effects!==3||button.name!=='renamed'||button.type!=='reset'||button.value!=='a\r\nb'||button.textContent!=='Caption stays')throw 'ordered native reflection';
var name=button.getAttributeNode('name'),value=button.getAttributeNode('value');
name.value='alias';value.value='latest';
if(button.name!=='alias'||button.value!=='latest'||button.getAttributeNode('name')!==name||document.getElementById('f').alias!==button)throw 'attached Attr aliases';
button.removeAttributeNode(name);if(button.name!==''||name.ownerElement!==null||name.value!=='alias')throw 'name removal';
button.setAttributeNode(name);button.value=null;
if(button.name!=='alias'||button.value!=='null'||button.getAttribute('value')!=='null')throw 'ordinary BUTTON DOMString';
button.type='invalid';if(button.type!=='submit')throw 'invalid type default';
button.removeAttribute('type');if(button.type!=='submit')throw 'missing type default';
button.type='button';button.value='a\r\nb';
"#);
    let node = id(&doc,"created");
    assert_eq!(forms.controls[&node].kind,Kind::Button);
    assert_eq!(forms.controls[&node].name,"alias");
    assert_eq!(forms.controls[&node].caption,"Caption stays");
    assert_eq!(forms.controls[&node].value.text(),"a\r\nb");
    forms.reset(id(&doc,"f"));
    assert_eq!(forms.controls[&node].value.text(),"a\r\nb","Native Reset leaves BUTTON's current value attribute mode intact");
    run(&mut doc,&mut forms,&mut page,"if(button.value!=='a\\r\\nb'||button.textContent!=='Caption stays')throw 'reset current button value';button.removeAttribute('value');if(button.value!=='')throw 'removed value';");
    assert_eq!(forms.controls[&node].value.text(),"");
    run(&mut doc,&mut forms,&mut page,"caption.textContent='Span caption';if(button.textContent!=='Span caption'||button.value!==''||button.form!==document.getElementById('f'))throw 'span caption synchronization';");
    assert_eq!(forms.controls[&node].caption,"Span caption");
    run(&mut doc,&mut forms,&mut page,"button.textContent='Whole caption';if(button.textContent!=='Whole caption'||button.value!==''||button.form!==document.getElementById('f'))throw 'whole caption synchronization';");
    assert_eq!(forms.controls[&node].caption,"Whole caption");
    assert_eq!(forms.controls[&node].value.text(),"");
    assert_eq!(forms.controls[&node].owner,Some(id(&doc,"f")));
}

#[test]
fn created_button_parsed_entity_value_keeps_raw_native_state_and_submission() {
    let mut doc=crate::html::parse("<html><body><form id=f action='/result'><button id=parsed type=button name=before value='a&#13;b&#10;c'>Parsed caption</button></form></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&current());page.set_resource_base(&current());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let node=id(&doc,"parsed");
    assert_eq!(doc.attr(node,"value"),Some("a\rb\nc"));
    assert_eq!(forms.controls[&node].value.text(),"a\rb\nc");
    run(&mut doc,&mut forms,&mut page,r#"
var parsed=document.getElementById('parsed');
if(parsed.value!=='a\rb\nc')throw 'raw parsed reflection';
parsed.name='line';parsed.type='submit';
if(parsed.value!=='a\rb\nc'||parsed.textContent!=='Parsed caption'||parsed.form!==document.getElementById('f'))throw 'metadata replaced parsed value';
"#);
    assert_eq!(forms.controls[&node].kind,Kind::Submit);
    assert_eq!(forms.controls[&node].name,"line");
    assert_eq!(forms.controls[&node].value.text(),"a\rb\nc");
    assert_eq!(query(&doc,&forms,"f",Some(node)).as_deref(),Some("line=a%0D%0Ab%0D%0Ac"));
    forms.reset(id(&doc,"f"));
    assert_eq!(forms.controls[&node].value.text(),"a\rb\nc");
    run(&mut doc,&mut forms,&mut page,r#"if(parsed.value!=='a\rb\nc'||parsed.textContent!=='Parsed caption')throw 'parsed Reset changed value mode';"#);
}

#[test]
fn created_button_fragment_moves_preserve_caption_value_owner_and_neighbor_editor() {
    let (mut doc,mut forms,mut page) = fixture();
    let neighbor = id(&doc,"neighbor");
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    let editor = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    editor.cursor=2;editor.anchor=5;editor.offset=1;
    let preserved = controls(&forms).into_iter().find(|row|row.0==neighbor).unwrap();
    run(&mut doc,&mut forms,&mut page,r#"
var button=document.createElement('button');button.id='created';button.name='move';button.type='submit';button.value='retained';
button.appendChild(document.createTextNode('Move caption'));
var donor=new DocumentFragment(),saved=button;donor.appendChild(button);
if(button.form!==null||button.getRootNode()!==donor)throw 'detached native button';
document.getElementById('host').appendChild(donor);
if(button.form!==document.getElementById('f')||donor.childNodes.length!==0)throw 'first owner';
document.getElementById('destination').appendChild(button);
if(button!==saved||button.form!==document.getElementById('g')||button.value!=='retained')throw 'second owner';
button.parentNode.removeChild(button);donor.appendChild(button);
if(button.form!==null||button.getRootNode()!==donor||button.textContent!=='Move caption')throw 'retained detached state';
document.getElementById('host').appendChild(donor);
"#);
    let node=id(&doc,"created");
    assert_eq!(forms.controls[&node].kind,Kind::Submit);
    assert_eq!(forms.controls[&node].owner,Some(id(&doc,"f")));
    assert_eq!(forms.controls[&node].caption,"Move caption");
    assert_eq!(forms.controls[&node].value.text(),"retained");
    assert_eq!(controls(&forms).into_iter().find(|row|row.0==neighbor),Some(preserved));
    assert_eq!(query(&doc,&forms,"f",Some(node)).as_deref(),Some("neighbor=edited+neighbor&move=retained"));
}

#[test]
fn created_button_context_and_native_interior_refusals_preserve_both_models() {
    let (mut doc,mut forms,mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var button=document.createElement('button');button.id='created';button.name='q';button.type='button';button.appendChild(document.createTextNode('Stable caption'));document.getElementById('host').appendChild(button);");
    let node=id(&doc,"created");let host=id(&doc,"host");let foreign=id(&doc,"foreign");let inert=id(&doc,"inert");
    for source in ["document.getElementById('foreign').appendChild(button);","document.getElementById('inert').appendChild(button);","button.innerHTML='<span>replacement</span>';","button.appendChild(document.createTextNode('extra'));","button.firstChild.data='replacement';"] {
        let original=controls(&forms);let children=[node,host,foreign,inert].map(|node|doc.nodes[node].children.clone());let retained=page.state.retained;
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}\n{}",page.report());
        assert_eq!(controls(&forms),original);assert_eq!([node,host,foreign,inert].map(|node|doc.nodes[node].children.clone()),children);
        // A failed append may have created its argument Text node first.
        assert!(page.state.retained>=retained);
        assert_eq!(forms.controls[&node].caption,"Stable caption");
    }
    run(&mut doc,&mut forms,&mut page,"var explicit=document.createElement('button');explicit.id='explicit';explicit.name='pending';explicit.type='button';explicit.appendChild(document.createTextNode('Pending'));");
    let explicit=id(&doc,"explicit");
    if let NodeKind::Element{attrs,..}=&mut doc.nodes[explicit].kind {attrs.push(crate::html::Attribute::unqualified("form".into(),"f".into()));}
    page.state.retained += "form".len()+"f".len();
    let before=controls(&forms);let children=doc.nodes[host].children.clone();
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('host').appendChild(explicit);");
    assert_eq!(outcome.errors,1,"{}",page.report());assert_eq!(controls(&forms),before);assert_eq!(doc.nodes[host].children,children);assert!(!forms.controls.contains_key(&explicit));
    run(&mut doc,&mut forms,&mut page,"var resource=document.createElement('button');resource.id='resource';var image=document.createElement('img');image.id='resource-image';image.src='/unrequested.png';");
    let resource=id(&doc,"resource");let image=id(&doc,"resource-image");
    // Internal caption topology tests admission's independent resource guard.
    doc.nodes[resource].children.push(image);page.state.tree.parents[image]=Some(resource);
    let before=controls(&forms);let children=doc.nodes[host].children.clone();
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('host').appendChild(resource);");
    assert_eq!(outcome.errors,1,"{}",page.report());assert_eq!(controls(&forms),before);assert_eq!(doc.nodes[host].children,children);assert!(!forms.controls.contains_key(&resource));
    assert!(page.take_image_changes().2.is_empty());
}

#[test]
fn created_button_attribute_and_admission_quotas_fail_before_native_publication() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var button=document.createElement('button');button.id='created';button.appendChild(document.createTextNode('Caption'));");
    page.state.rebuild(&doc).unwrap();
    let node=id(&doc,"created");let host=id(&doc,"host");let before=controls(&forms);let retained=page.state.retained;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for (key,text) in [("name","n".repeat(8193)),("value","🦀".repeat(2049))] {
        assert!(bridge.public_attribute(node,crate::html::AttributeName::unqualified(key.into()),Some(text)).is_err());
        assert_eq!(bridge.doc.attr(node,key),None);assert_eq!(controls(bridge.forms),before);assert_eq!(bridge.state.retained,retained);
    }
    bridge.state.retained_limit=retained;
    assert!(bridge.public_attribute(node,crate::html::AttributeName::unqualified("name".into()),Some("growth".into())).is_err());
    assert_eq!(bridge.doc.attr(node,"name"),None);assert_eq!(controls(bridge.forms),before);
    drop(bridge);
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('host').appendChild(button);");
    assert_eq!(outcome.errors,1,"Native caption storage must be charged before insertion: {}",page.report());
    assert!(doc.nodes[host].children.is_empty());assert!(!forms.controls.contains_key(&node));assert_eq!(controls(&forms),before);assert_eq!(page.state.retained,retained);

    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var button=document.createElement('button');button.id='boundary';button.type='button';
var boundaryName='n',boundaryValue='🦀';
for(var i=0;i<13;i++)boundaryName=boundaryName+boundaryName;
for(var j=0;j<11;j++)boundaryValue=boundaryValue+boundaryValue;
button.name=boundaryName;button.value=boundaryValue;button.appendChild(document.createTextNode('Boundary'));
document.getElementById('host').appendChild(button);
"#);
    let control=&forms.controls[&id(&doc,"boundary")];
    assert_eq!(control.name,"n".repeat(8192));
    assert_eq!(control.value.text(),"🦀".repeat(2048));
    assert_eq!(control.value.text().encode_utf16().count(),forms::MAX_VALUE_UNITS);
}

#[test]
fn created_button_control_and_reset_payload_limits_refuse_without_partial_admission() {
    for existing in ["<input type=hidden>".repeat(forms::MAX_CONTROLS),(0..32).map(|_|format!("<input value='{}'>","x".repeat(forms::MAX_VALUE_UNITS))).collect::<String>()] {
        let mut doc=crate::html::parse(&format!("<html><body><form id=f><div id=host></div>{existing}</form></body></html>"));doc.scripting_enabled=true;
        let mut forms=Forms::new(&doc);let text:Vec<_>=forms.controls.iter().filter(|(_,c)|c.kind==Kind::Text).map(|(&n,_)|n).collect();
        for node in text {forms.script_set_value(node,"").unwrap();}
        let mut page=Page::for_test();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
        run(&mut doc,&mut forms,&mut page,"var button=document.createElement('button');button.id='created';button.appendChild(document.createTextNode('Caption'));");
        let node=id(&doc,"created");let host=id(&doc,"host");
        // The pending attribute is a fixture so the admission boundary is
        // exercised independently of an earlier attribute payload check.
        if let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind {attrs.push(crate::html::Attribute::unqualified("name".into(),"q".into()));}
        page.state.retained += "name".len()+"q".len();
        page.state.rebuild(&doc).unwrap();
        let before=controls(&forms);let retained=page.state.retained;
        let result=page.console_eval(&mut doc,&mut forms,"document.getElementById('host').appendChild(button);");
        assert_eq!(result.errors,1,"{}",page.report());assert_eq!(controls(&forms),before);assert!(!forms.controls.contains_key(&node));assert!(doc.nodes[host].children.is_empty());assert_eq!(page.state.retained,retained);
    }
}

#[test]
fn created_button_parsed_select_parent_refuses_type_changes_atomically_but_reflects_name_value() {
    let mut doc=crate::html::parse("<html><body><form id=f><input id=neighbor name=neighbor value=seed><select id=select><button id=button type=button name=before value=seed>Caption</button></select></form></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&current());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    run(&mut doc,&mut forms,&mut page,r#"
var button=document.getElementById('button'),select=document.getElementById('select');
if(button.parentNode!==select||button.type!=='button')throw 'actual parsed SELECT parent';
button.name='updated';button.value='raw\r\nvalue';
var nameAlias=button.getAttributeNode('name');nameAlias.value='via-alias';
var typeAlias=button.getAttributeNode('type'),incoming=document.createAttribute('type');incoming.value='invalid';
if(button.name!=='via-alias'||button.value!=='raw\r\nvalue'||button.form!==document.getElementById('f'))throw 'preserved native reflection';
"#);
    let node=id(&doc,"button");let select=id(&doc,"select");
    assert_eq!(page.state.tree.parents[node],Some(select));
    assert_eq!(forms.controls[&node].kind,Kind::Button);
    assert_eq!(forms.controls[&node].name,"via-alias");
    assert_eq!(forms.controls[&node].value.text(),"raw\r\nvalue");
    for source in ["button.type='invalid';","button.type='button';","button.setAttribute('type','invalid');",
        "button.removeAttribute('type');","button.setAttributeNS(null,'type','');","button.removeAttributeNS(null,'type');",
        "typeAlias.value='invalid';","button.removeAttributeNode(typeAlias);","button.setAttributeNode(incoming);","button.setAttributeNodeNS(incoming);"] {
        let model=controls(&forms);let parents=page.state.tree.parents.clone();let connected=page.state.dom.connected.clone();
        let children=doc.nodes.iter().map(|entry|entry.children.clone()).collect::<Vec<_>>();
        let identities=page.state.attributes.iter().map(|attr|(attr.owner,attr.name.qualified.clone(),attr.value.scalar().unwrap().to_owned())).collect::<Vec<_>>();
        let retained=page.state.retained;let commits=page.state.dom_commits;
        let result=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(result.errors,1,"{source}\n{}",page.report());
        assert_eq!(controls(&forms),model,"{source}");assert_eq!(page.state.tree.parents,parents);assert_eq!(page.state.dom.connected,connected);
        assert_eq!(doc.nodes.iter().map(|entry|entry.children.clone()).collect::<Vec<_>>(),children);
        assert_eq!(page.state.attributes.iter().map(|attr|(attr.owner,attr.name.qualified.clone(),attr.value.scalar().unwrap().to_owned())).collect::<Vec<_>>(),identities);
        assert_eq!(doc.attr(node,"type"),Some("button"));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
        run(&mut doc,&mut forms,&mut page,"if(button.type!=='button'||button.getAttributeNode('type')!==typeAlias||typeAlias.ownerElement!==button||typeAlias.value!=='button'||incoming.ownerElement!==null||incoming.value!=='invalid')throw 'failed type transaction published state';");
    }
    run(&mut doc,&mut forms,&mut page,"button.name='still-name';button.value='still-value';if(button.name!=='still-name'||button.value!=='still-value'||button.type!=='button')throw 'name/value policy was narrowed';");
    assert_eq!(forms.controls[&node].kind,Kind::Button);
    assert_eq!(forms.controls[&node].name,"still-name");assert_eq!(forms.controls[&node].value.text(),"still-value");
}

#[test]
fn created_button_caption_ancestry_preserves_exact_tree_depth_boundary_and_recovery() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var boundaryRoot=document.createElement('div'),boundaryTail=boundaryRoot;
for(var depth=1;depth<128;depth++){
 var next=document.createElement('div');boundaryTail.appendChild(next);boundaryTail=next;
}
var beyond=document.createElement('span');
if(boundaryTail.getRootNode()!==boundaryRoot||boundaryRoot.isConnected||boundaryTail.parentNode===null)throw 'exact admitted depth';
"#);
    let root=doc.nodes.iter().enumerate().find_map(|(node,entry)|
        (page.state.dom.created.contains(&node) && tag(&doc,node)=="div"
            && page.state.tree.parents[node].is_none() && !entry.children.is_empty()).then_some(node)).unwrap();
    let mut tail=root;let mut depth=1;
    while let Some(&child)=doc.nodes[tail].children.first() {tail=child;depth+=1;}
    assert_eq!(depth,128,"The original maximum tree must remain admissible");
    let beyond=doc.nodes.iter().enumerate().find_map(|(node,_)|
        (page.state.dom.created.contains(&node) && tag(&doc,node)=="span" && page.state.tree.parents[node].is_none()).then_some(node)).unwrap();
    {
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        assert!(bridge.validate_button_children(tail,&[beyond]).is_ok(),"An exactly exhausted128-step ancestry is complete");
        // Internal cycle control: completing128steps is not sufficient when
        // ancestry is still present. Restore the synthetic link immediately.
        bridge.state.tree.parents[root]=Some(root);
        assert!(bridge.validate_button_children(tail,&[beyond]).unwrap_err().contains("BUTTON caption ancestry limit"));
        bridge.state.tree.parents[root]=None;
    }
    let model=controls(&forms);let parents=page.state.tree.parents.clone();let connected=page.state.dom.connected.clone();
    let children=doc.nodes.iter().map(|entry|entry.children.clone()).collect::<Vec<_>>();
    let retained=page.state.retained;let commits=page.state.dom_commits;
    let result=page.console_eval(&mut doc,&mut forms,"boundaryTail.appendChild(beyond);");
    assert_eq!(result.errors,1,"{}",page.report());
    assert!(page.report().contains("DOM tree depth limit reached"),"{}",page.report());
    assert_eq!(controls(&forms),model);assert_eq!(page.state.tree.parents,parents);assert_eq!(page.state.dom.connected,connected);
    assert_eq!(doc.nodes.iter().map(|entry|entry.children.clone()).collect::<Vec<_>>(),children);
    assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
    run(&mut doc,&mut forms,&mut page,"if(boundaryTail.getRootNode()!==boundaryRoot||beyond.parentNode!==null||boundaryRoot.isConnected)throw 'partial depth mutation';document.title='Exact depth recovered';");
    assert_eq!(doc.title,"Exact depth recovered");
}
