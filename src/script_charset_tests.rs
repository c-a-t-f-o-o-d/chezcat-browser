//! Created SCRIPT charset keeps native storage, output and authority boundaries.
use super::*;

fn fixture()->(Document,Forms,Page,usize,usize) {
    let mut doc=crate::html::parse("<html><head></head><body><div id=other></div></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    assert_eq!(page.console_eval(&mut doc,&mut forms,"var script=document.createElement('script');script.id='sheet';script.charset='seed';").errors,0,"{}",page.report());
    let sheet=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("sheet")).unwrap();
    let other=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("other")).unwrap();
    (doc,forms,page,sheet,other)
}

#[test]
fn script_charset_domstring_native_backing_copy_and_result_refusal_recover() {
    let (mut doc,mut forms,mut page,sheet,_)=fixture();
    let old_capacity=doc.dom_attr(sheet,"charset").unwrap().capacity();
    let value=Value::StringUnits(vec![0xd800,0xfffd,0xdc00]);
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let before=(b.state.retained,b.state.dom_commits);let limit=b.state.retained_limit;
    b.state.retained_limit=b.state.retained;
    assert!(b.resource_charset_set(sheet,&value).unwrap_err().contains("backing allowance"));
    assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.doc.attr(sheet,"charset"),Some("seed"));
    b.state.retained_limit=limit;b.state.copy_work=0;
    assert!(b.resource_charset_set(sheet,&value).unwrap_err().contains("string work limit"));
    assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.doc.attr(sheet,"charset"),Some("seed"));
    b.state.copy_work=b.state.limits.copy_bytes;
    b.resource_charset_set(sheet,&value).unwrap();
    let new_capacity=b.doc.dom_attr(sheet,"charset").unwrap().capacity();
    assert_eq!(b.doc.dom_attr(sheet,"charset").unwrap().raw_units(),Some(&[0xd800,0xfffd,0xdc00][..]));
    assert_eq!(b.state.retained,before.0-old_capacity+new_capacity);assert_eq!(b.state.dom_commits,before.1.map(|n|n+1));
    let bytes=b.state.limits.host.string_bytes;b.state.limits.host.string_bytes=5;
    assert!(b.resource_charset_get(sheet).unwrap_err().contains("allowance"));
    b.state.limits.host.string_bytes=bytes;b.state.copy_work=0;
    assert!(b.resource_charset_get(sheet).unwrap_err().contains("string work limit"));
    b.state.copy_work=b.state.limits.copy_bytes;
    assert!(matches!(b.resource_charset_get(sheet).unwrap(),Value::StringUnits(units) if units==[0xd800,0xfffd,0xdc00]));
    assert_eq!(b.state.retained,before.0-old_capacity+new_capacity);
}

#[test]
fn script_charset_domstring_native_policy_is_exact_and_fetch_queue_is_unchanged() {
    let (mut doc,mut forms,mut page,sheet,other)=fixture();
    let raw=DomString::from_units(&[0xd800],page.state.retained_limit).unwrap();
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for (node,name) in [(sheet,"src"),(sheet,"CHARSET"),(other,"charset")] {
        let before=(b.state.retained,b.state.dom_commits);
        assert!(b.public_dom_attribute(node,crate::html::AttributeName::unqualified(name.into()),Some(raw.try_clone().unwrap()),script_attr::InputPreparation::Pending).is_err());
        assert_eq!((b.state.retained,b.state.dom_commits),before);
    }
    b.public_dom_attribute(sheet,crate::html::AttributeName::unqualified("charset".into()),Some(raw),script_attr::InputPreparation::Pending).unwrap();
    assert!(b.state.dynamic.pending.is_empty());
    assert_eq!(b.doc.dom_attr(sheet,"charset").unwrap().raw_units(),Some(&[0xd800][..]));
    b.public_dom_attribute(sheet,crate::html::AttributeName::unqualified("charset".into()),None,script_attr::InputPreparation::Pending).unwrap();
    assert!(b.doc.dom_attr(sheet,"charset").is_none());assert!(b.state.dynamic.pending.is_empty());
}

#[test]
fn script_charset_domstring_native_active_attr_transfer_still_refuses() {
    let (mut doc,mut forms,mut page,script,_)=fixture();
    for name in ["src","type","nonce","integrity","crossorigin","referrerpolicy","fetchpriority","async","defer","nomodule"] {
        let setup=format!("var setting=document.createAttribute('{name}');setting.value='held';");
        assert_eq!(page.console_eval(&mut doc,&mut forms,&setup).errors,0,"{}",page.report());
        let before=(page.state.retained,page.state.dom_commits);
        assert_eq!(page.console_eval(&mut doc,&mut forms,"script.setAttributeNode(setting);").errors,1);
        assert!(page.report().contains("active script/stylesheet settings"),"{}",page.report());
        assert_eq!((page.state.retained,page.state.dom_commits),before);
        assert!(doc.dom_attr(script,name).is_none());
        assert!(page.state.dynamic.pending.is_empty());
        assert_eq!(page.console_eval(&mut doc,&mut forms,"if(setting.ownerElement!==null||setting.value!=='held')throw 'refused Attr changed';").errors,0,"{}",page.report());
    }
}
