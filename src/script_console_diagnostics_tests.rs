//! Real native console diagnostics preserve Error identity and avoid author code.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><div id=out>unchanged</div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://console-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
#[test]
fn console_native_error_origin_survives_author_getters_and_keeps_actual_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    let source="var saved,hits=0;\nfunction originalFailure(){return null.missing;}\ntry{originalFailure();}catch(e){saved=e;Object.defineProperty(e,'message',{get:function(){hits++;return 'private getter value';}});Object.defineProperty(e,'name',{get:function(){hits++;return 'fake';}});console.error('initialization',e);}\nif(hits!==0||!(saved instanceof TypeError))throw 'printer invoked author code or changed identity';";
    let out = page.console_eval(&mut doc, &mut forms, source);
    assert_eq!(out.errors, 0, "{}", page.report());
    let report = page.report();
    assert!(report.contains("console.error: initialization TypeError: Cannot access a property of null or undefined."),"{report}");
    assert!(
        report.contains("Original native error at line 2"),
        "{report}"
    );
    assert!(report.contains("null.missing"));
    assert!(!report.contains("console.error: initialization [script object]"));
    let out=page.console_eval(&mut doc,&mut forms,"try{throw saved;}catch(e){if(e!==saved||hits!==0)throw 'identity';}document.title='diagnostic complete';");
    assert_eq!(out.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "diagnostic complete");
}
#[test]
fn console_native_error_constructor_and_ordinary_proxy_objects_do_not_run_getters_or_coercions() {
    let (mut doc, mut forms, mut page) = fixture();
    let out=page.console_eval(&mut doc,&mut forms,"var hits=0,e=TypeError('constructed');var fake={name:'TypeError',message:'private object field',toString:function(){hits++;return 'coercion';}};Object.defineProperty(fake,'message',{get:function(){hits++;return 'getter';}});var proxy=new Proxy(fake,{get:function(){hits++;throw 'proxy read';}});console.warn(e);console.info(fake,proxy);Object.defineProperty(e,'message',{get:function(){hits++;throw 'message getter';}});console.log(e);if(hits!==0)throw 'printer invoked author code';");
    assert_eq!(out.errors, 0, "{}", page.report());
    let lines = page
        .state
        .logs
        .iter()
        .filter(|line| line.starts_with("console."))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        lines.contains("console.warn: TypeError: constructed"),
        "{lines}"
    );
    assert!(
        lines.contains("console.info: [script object] [script object]"),
        "{lines}"
    );
    assert!(lines.contains("console.log: TypeError"), "{lines}");
    assert!(!lines.contains("private object field"));
}

#[test]
fn console_native_error_child_realm_details_are_opaque_after_origin_change() {
    let (mut doc, mut forms, mut page) = fixture();
    let out=page.console_eval(&mut doc,&mut forms,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,childError=new w.TypeError('child-owned-message');console.error(childError);");
    assert_eq!(out.errors, 0, "{}", page.report());
    assert!(page
        .state
        .logs
        .iter()
        .any(|line| line.starts_with("console.error: TypeError: child-owned-message")));
    page.clear_console();
    page.set_location(
        &crate::navigation::address(
            "https://other-console.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let out = page.console_eval(&mut doc, &mut forms, "console.error(childError);");
    assert_eq!(out.errors, 0, "{}", page.report());
    let lines = page
        .state
        .logs
        .iter()
        .filter(|line| line.starts_with("console."))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(lines, "console.error: [script object]");
}
