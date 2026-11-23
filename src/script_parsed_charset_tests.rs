//! Parsed SCRIPT charset metadata does not grant dynamic execution authority.
use super::*;

fn fixture()->(Document,Forms,Page,usize,usize) {
    let mut doc=crate::html::parse("<html><head><script id=sheet type=application/x-inert charset=seed></script></head><body><div id=other></div></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let sheet=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("sheet")).unwrap();
    let other=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("other")).unwrap();
    (doc,forms,page,sheet,other)
}

#[test]
fn parsed_script_charset_native_backing_copy_and_result_refusal_recover() {
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
fn parsed_script_charset_native_policy_is_exact_and_fetch_queue_is_unchanged() {
    let (mut doc,mut forms,mut page,sheet,other)=fixture();
    assert!(!page.state.dom.created.contains(&sheet));
    let raw=DomString::from_units(&[0xd800],page.state.retained_limit).unwrap();
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(b.resource_charset_element(sheet));assert!(!b.dom_script(sheet));assert!(!b.resource_charset_element(usize::MAX));
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
fn parsed_script_charset_native_foreign_and_unknown_namespace_do_not_gain_metadata() {
    let (mut doc,mut forms,mut page,script,_)=fixture();
    for namespace in [crate::html::Namespace::Svg,crate::html::Namespace::Other,crate::html::Namespace::UnknownForeign] {
        doc.nodes[script].namespace=namespace;
        let raw=DomString::from_units(&[0xd800],page.state.retained_limit).unwrap();
        let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        assert!(!b.resource_charset_element(script));
        let before=(b.state.retained,b.state.dom_commits);
        assert!(b.public_dom_attribute(script,crate::html::AttributeName::unqualified("charset".into()),Some(raw),script_attr::InputPreparation::Pending).is_err());
        assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.doc.attr(script,"charset"),Some("seed"));
        assert!(b.state.dynamic.pending.is_empty());
    }
}
