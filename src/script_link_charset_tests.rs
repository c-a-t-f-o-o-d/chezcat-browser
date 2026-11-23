//! Exact LINK metadata uses the existing native backing and output admission.
use super::*;

fn fixture()->(Document,Forms,Page,usize,usize) {
    let mut doc=crate::html::parse("<html><head><link id=sheet charset=seed></head><body><div id=other></div></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let sheet=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("sheet")).unwrap();
    let other=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("other")).unwrap();
    (doc,forms,page,sheet,other)
}

#[test]
fn link_charset_domstring_native_backing_copy_and_result_refusal_recover() {
    let (mut doc,mut forms,mut page,sheet,_)=fixture();
    let old_capacity=doc.dom_attr(sheet,"charset").unwrap().capacity();
    let value=Value::StringUnits(vec![0xd800,0xfffd,0xdc00]);
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let before=(b.state.retained,b.state.dom_commits);let limit=b.state.retained_limit;
    b.state.retained_limit=b.state.retained;
    assert!(b.stylesheet_set(sheet,"charset",&value).unwrap_err().contains("backing allowance"));
    assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.doc.attr(sheet,"charset"),Some("seed"));
    b.state.retained_limit=limit;b.state.copy_work=0;
    assert!(b.stylesheet_set(sheet,"charset",&value).unwrap_err().contains("string work limit"));
    assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.doc.attr(sheet,"charset"),Some("seed"));
    b.state.copy_work=b.state.limits.copy_bytes;
    assert!(b.stylesheet_set(sheet,"charset",&value).unwrap());
    let new_capacity=b.doc.dom_attr(sheet,"charset").unwrap().capacity();
    assert_eq!(b.doc.dom_attr(sheet,"charset").unwrap().raw_units(),Some(&[0xd800,0xfffd,0xdc00][..]));
    assert_eq!(b.state.retained,before.0-old_capacity+new_capacity);assert_eq!(b.state.dom_commits,before.1.map(|n|n+1));
    let bytes=b.state.limits.host.string_bytes;b.state.limits.host.string_bytes=5;
    assert!(b.stylesheet_get(sheet,"charset").unwrap_err().contains("allowance"));
    b.state.limits.host.string_bytes=bytes;b.state.copy_work=0;
    assert!(b.stylesheet_get(sheet,"charset").unwrap_err().contains("string work limit"));
    b.state.copy_work=b.state.limits.copy_bytes;
    assert!(matches!(b.stylesheet_get(sheet,"charset").unwrap(),Some(Value::StringUnits(units)) if units==[0xd800,0xfffd,0xdc00]));
    assert_eq!(b.state.retained,before.0-old_capacity+new_capacity);
}

#[test]
fn link_charset_domstring_native_policy_is_exact_and_fetch_queue_is_unchanged() {
    let (mut doc,mut forms,mut page,sheet,other)=fixture();
    let raw=DomString::from_units(&[0xd800],page.state.retained_limit).unwrap();
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for (node,name) in [(sheet,"href"),(sheet,"CHARSET"),(other,"charset")] {
        let before=(b.state.retained,b.state.dom_commits);
        assert!(b.public_dom_attribute(node,crate::html::AttributeName::unqualified(name.into()),Some(raw.try_clone().unwrap()),script_attr::InputPreparation::Pending).is_err());
        assert_eq!((b.state.retained,b.state.dom_commits),before);
    }
    b.public_dom_attribute(sheet,crate::html::AttributeName::unqualified("charset".into()),Some(raw),script_attr::InputPreparation::Pending).unwrap();
    assert!(b.state.stylesheets.pending.is_empty());assert_eq!(b.state.stylesheets.in_flight,0);
    assert_eq!(b.doc.dom_attr(sheet,"charset").unwrap().raw_units(),Some(&[0xd800][..]));
    b.public_dom_attribute(sheet,crate::html::AttributeName::unqualified("charset".into()),None,script_attr::InputPreparation::Pending).unwrap();
    assert!(b.doc.dom_attr(sheet,"charset").is_none());assert!(b.state.stylesheets.pending.is_empty());
}
