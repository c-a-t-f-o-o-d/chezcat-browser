//! Original custom HTML data through the shared attribute transaction.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document("<html><body><div id=target panel-mode=seed><span>child</span></div><form id=f><input id=neighbor value=original></form><svg id=vector></svg></body></html>",&||Ok(())).unwrap();
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){
    assert_eq!(page.console_eval(doc,forms,source).errors,0,"{}",page.report());
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}
#[test]
fn custom_html_attributes_share_real_aliases_observation_clone_serialization_and_selectors(){
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var target=document.getElementById('target'),map=target.attributes,mode=map.getNamedItem('panel-mode'),oldNames=target.getAttributeNames();
var observer=new MutationObserver(function(){throw 'undrained custom records';});observer.observe(target,{attributes:true,attributeOldValue:true});
target.setAttribute('PANEL-MODE','ready');target.setAttribute('panel-ready','');target.removeAttribute('panel-mode');
var records=observer.takeRecords();observer.disconnect();
if(records.length!==3||records[0].attributeName!=='panel-mode'||records[0].oldValue!=='seed'||records[1].attributeName!=='panel-ready'||records[1].oldValue!==null||records[2].oldValue!=='ready')throw 'custom native observation/old values';
for(var record of records){if(record.target!==target||record.attributeNamespace!==null||record.type!=='attributes')throw 'custom record owner';}
if(target.attributes!==map||map.getNamedItem('panel-mode')!==null||mode.ownerElement!==null||mode.ownerDocument!==document||mode.value!=='ready'||oldNames.join(',')!=='id,panel-mode'||target.getAttributeNames().join(',')!=='id,panel-ready')throw 'real custom aliases and order';
target.setAttributeNode(mode);mode.value='active';
if(map.getNamedItem('panel-mode')!==mode||mode.ownerElement!==target||target.getAttribute('panel-mode')!=='active'||target.getAttributeNames().join(',')!=='id,panel-ready,panel-mode')throw 'custom Attr write/reattach';
var clone=target.cloneNode(true);mode.value='changed';
if(clone.getAttribute('panel-mode')!=='active'||clone.getAttributeNode('panel-mode')===mode||clone.children.length!==1||clone.outerHTML.indexOf('panel-mode="active"')<0)throw 'custom clone and serialization';
if(!target.matches('[panel-ready][panel-mode="changed"]')||document.querySelector('[panel-mode="changed"]')!==target||target.firstChild.closest('[panel-ready]')!==target)throw 'custom selector state';
var detached=document.createElement('span');detached.setAttribute('component-key','raw');detached.setAttribute('on-click','ordinary');detached.setAttribute('resource-href','https://never.test/metadata');
if(detached.getAttribute('on-click')!=='ordinary'||detached.getAttribute('resource-href')!=='https://never.test/metadata'||detached.isConnected)throw 'custom data received native authority';
document.body.setAttribute('layout-mode','wide');if(document.body.getAttribute('layout-mode')!=='wide')throw 'body custom data';
var order='';target.setAttribute({toString:function(){order+='n';return 'ANOTHER-MODE';}},{toString:function(){order+='v';return 'value';}});
if(order!=='nv'||target.getAttribute('another-mode')!=='value')throw 'custom conversion order';
"#);
}
#[test]
fn custom_html_attributes_keep_native_guards_and_failure_atomicity(){
    let(mut doc,mut forms,mut page)=fixture();
    let target=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="target")).unwrap();
    let vector=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="vector")).unwrap();
    let neighbor=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..}if id=="neighbor")).unwrap();
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let e=&mut forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
    let e=&forms.controls[&neighbor];let editor=(e.value.text(),e.value.cursor,e.value.anchor,e.value.offset,e.dirty_value,e.owner);
    let snapshot=format!("{:?}",doc.nodes);let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    for (node,name) in [(target,"src"),(target,"onclick"),(target,"is"),(target,"http-equiv"),(target,"accept-charset"),(vector,"panel-mode")]{
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        assert!(bridge.public_attribute(node,AttributeName::unqualified(name.into()),Some("refused".into())).is_err(),"{name}");
        assert_eq!(format!("{:?}",bridge.doc.nodes),snapshot);assert_eq!(bridge.state.retained,retained);assert_eq!(bridge.state.dom_commits,commits);
    }
    let value="q".repeat(80);let old=doc.attr(target,"panel-mode").unwrap().len();
    page.state.retained_limit=retained+value.capacity()-old-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(target,AttributeName::unqualified("panel-mode".into()),Some(value.clone())).is_err());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
    page.state.retained_limit=limit;page.begin_job();
    page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(target,AttributeName::unqualified("panel-extra".into()),Some("new".into())).is_err());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
    page.begin_job();run(&mut doc,&mut forms,&mut page,"document.getElementById('target').setAttribute('panel-mode','recovered');");
    let e=&forms.controls[&neighbor];assert_eq!((e.value.text(),e.value.cursor,e.value.anchor,e.value.offset,e.dirty_value,e.owner),editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");assert_eq!(doc.attr(target,"panel-mode"),Some("recovered"));
}
