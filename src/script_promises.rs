//! Promise jobs belong to the displayed document and drain only when the host
//! has returned from an ordinary script or callback. A checkpoint never resets
//! execution, retained-value, copy, host-work or startup aggregate allowances.

use super::*;

impl Page {
    pub(super) fn microtask_checkpoint(&mut self, doc: &mut Document, forms: &mut Forms) {
        if !self.state.enabled || !doc.scripting_enabled {
            self.runtime.cancel_microtasks();
            return;
        }
        match self.runtime.microtask_checkpoint(&mut Bridge {
            doc,
            forms,
            state: &mut self.state,
        }) {
            Ok(diagnostics) => {
                for diagnostic in diagnostics {
                    self.state.error("Microtask", &diagnostic);
                }
            }
            Err(error) => self.state.error("Microtask checkpoint", &error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_runtime_diagnostic_report_reserves_space_with_full_logs_and_unicode_errors() {
        let (mut doc,mut forms,mut page,outcome)=make_page("<p></p>",&[]);
        assert_eq!(outcome.errors,0);
        page.runtime.define_global("diagnosticMessage",Value::String("\u{10ffff}".repeat(1000))).unwrap();
        let outcome=page.console_eval(&mut doc,&mut forms,"for(var i=0;i<12;i++){try{throw Error(diagnosticMessage);}catch(e){}}for(var j=0;j<6;j++)Promise.reject(Error(diagnosticMessage)).catch(function(){});");
        assert_eq!(outcome.errors,0,"{}",page.report());
        for _ in 0..MAX_LOG_LINES { page.state.log(&"x".repeat(MAX_LOG_LINE)); }
        let errors=page.error_count();
        let report=page.report();
        assert!(report.len()<=MAX_LOG_BYTES+1024,"{} bytes",report.len());
        assert!(report.contains("Further caught native Error samples omitted: 4"));
        assert!(report.contains("rejected=6 (handled=6, reported=0)"));
        assert_eq!(report,page.report());
        assert_eq!(page.error_count(),errors);
    }

    fn make_page(markup: &str, sources: &[&str]) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse(markup);
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let scripts: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(node, source)| Script {
                node,
                source: (*source).into(),
                label: format!("promise fixture {node}"),
            })
            .collect();
        let mut page = Page::for_test();
        let location = crate::navigation::address(
            "http://127.0.0.1:1/promise-fixture",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        page.set_resource_base(&location);
        let outcome = page.start(&mut doc, &mut forms, &scripts);
        (doc, forms, page, outcome)
    }

    #[test]
    fn promise_jobs_drain_before_the_next_classic_script_and_lifecycle_event() {
        let (doc, _, page, outcome) = make_page(
            "<p id=out></p>",
            &[
                r#"
var order='script';
if(window.Promise!==Promise||window.queueMicrotask!==queueMicrotask)throw 'global identity';
Promise.resolve(7).then(function(value){order+=':promise'+value;queueMicrotask(function(){order+=':nested';});});
queueMicrotask(function(){order+=':queued';});
document.addEventListener('DOMContentLoaded',function(){
    order+=':dom';Promise.resolve().then(function(){order+=':domjob';});
});
window.addEventListener('load',function(){
    if(order!=='script:promise7:queued:nested:second:dom:domjob')throw order;
    Promise.resolve('complete').then(function(value){document.title=value;});
});
"#,
                "if(order!=='script:promise7:queued:nested')throw order;order+=':second';",
            ],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "complete");
        assert!(!page.runtime.has_microtasks());
    }

    #[test]
    fn promise_jobs_run_after_catchable_script_errors_but_not_fatal_faults() {
        let (doc, _, page, outcome) = make_page(
            "<p></p>",
            &[
                "Promise.resolve().then(function(){document.title='kept';});throw Error('ordinary failure');",
                "if(document.title!=='kept')throw 'lost job';",
            ],
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "kept");
        let (doc, _, page, outcome) = make_page(
            "<p></p>",
            &[
                "Promise.resolve().then(function(){document.title='forbidden';});while(true){}",
                "if(document.title==='forbidden')throw 'revived fatal job';document.title='recovered';",
            ],
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "recovered");
        assert!(!page.runtime.has_microtasks());
    }

    #[test]
    fn promise_event_callbacks_checkpoint_before_next_listener_and_default_action() {
        let (mut doc, mut forms, mut page, outcome) = make_page(
            "<form id=f><button id=go type=submit>Go</button></form>",
            &[r#"
var button=document.getElementById('go'),order='';
button.addEventListener('click',function(event){
    order+='first';Promise.resolve().then(function(){order+=':job';event.preventDefault();});
});
button.addEventListener('click',function(event){
    if(order!=='first:job'||!event.defaultPrevented)throw 'listener ordering';
    document.title='event complete';
});
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let target = doc
            .nodes
            .iter()
            .enumerate()
            .find_map(|(id, _)| (doc.attr(id, "id") == Some("go")).then_some(id))
            .unwrap();
        page.begin_job();
        let outcome = page.dispatch(&mut doc, &mut forms, target, "click", true);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(outcome.prevented);
        assert_eq!(doc.title, "event complete");
    }

    #[test]
    fn promise_timer_and_animation_callbacks_checkpoint_individually() {
        let (mut doc, mut forms, mut page, outcome) = make_page(
            "<p></p>",
            &[r#"
var order='';
setTimeout(function(){
    order+='timer';Promise.resolve().then(function(){order+=':timerjob';document.title=order;});
},0);
requestAnimationFrame(function(){
    order+=':frame1';Promise.resolve().then(function(){order+=':framejob';});
});
requestAnimationFrame(function(){
    if(order!=='timer:timerjob:frame1:framejob')throw order;
    Promise.resolve().then(function(){document.title=order+':frame2job';});
});
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let now = Instant::now() + Duration::from_secs(1);
        let outcome = page.poll_timer_at(&mut doc, &mut forms, now).unwrap();
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "timer:timerjob");
        let outcome = page.poll_timer_at(&mut doc, &mut forms, now).unwrap();
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "timer:timerjob:frame1:framejob:frame2job");
    }

    #[test]
    fn promise_console_diagnostics_and_cancelled_document_jobs_are_bounded() {
        let (mut doc, mut forms, mut page, outcome) = make_page("<p></p>", &[]);
        assert_eq!(outcome.errors, 0);
        let outcome = page.console_eval(
            &mut doc,
            &mut forms,
            "Promise.resolve(3).then(function(value){document.title='console '+value;});",
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "console 3");
        let outcome = page.console_eval(&mut doc, &mut forms,
            "queueMicrotask(function(){throw Error('queued failure');});Promise.resolve().then(function(){document.title='continued';});");
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "continued");
        page.begin_job();
        page.runtime.execute("var finish;var held=new Promise(function(resolve){finish=resolve;});held.then(function(){document.title='stale';});",
            &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }).unwrap();
        page.stop_timers("Cancelled document callback work.");
        let outcome = page.console_eval(&mut doc, &mut forms, "finish('discarded');");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "continued");
        page.runtime
            .execute(
                "Promise.resolve().then(function(){document.title='disabled job';});",
                &mut Bridge {
                    doc: &mut doc,
                    forms: &mut forms,
                    state: &mut page.state,
                },
            )
            .unwrap();
        doc.scripting_enabled = false;
        page.microtask_checkpoint(&mut doc, &mut forms);
        doc.scripting_enabled = true;
        page.begin_job();
        page.microtask_checkpoint(&mut doc, &mut forms);
        assert_eq!(doc.title, "continued");
        assert!(!page.runtime.has_microtasks());
    }

    #[test]
    fn promise_dynamic_script_jobs_precede_element_and_window_load_handlers() {
        let (mut doc, mut forms, mut page, outcome) = make_page(
            "<html><head></head><body></body></html>",
            &[r#"
var order='created',child=document.createElement('script');child.src='/child.js';
child.onload=function(){
    if(order!=='created:scriptjob')throw order;
    Promise.resolve().then(function(){order+=':loadjob';});
};
window.addEventListener('load',function(){
    if(order!=='created:scriptjob:loadjob')throw order;
    Promise.resolve().then(function(){document.title=order+':windowjob';});
});
document.head.appendChild(child);
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let request = page.take_dynamic_scripts().pop_front().unwrap();
        assert!(request.error.is_none(), "{:?}", request.error);
        let outcome = page.complete_dynamic_script(
            &mut doc,
            &mut forms,
            &request,
            Ok(Script {
                node: request.node,
                source: "Promise.resolve().then(function(){order+=':scriptjob';});".into(),
                label: "original dynamic Promise fixture".into(),
            }),
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "created:scriptjob:loadjob:windowjob");
    }

    #[test]
    fn promise_reactions_keep_document_eval_policy_and_rejection_diagnostics() {
        let mut doc = crate::html::parse("<p></p>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let mut policy = crate::csp::PolicySet::default();
        policy.add_header("script-src 'nonce-original-fixture'");
        // As with the loader, these sources have already been selected for
        // execution. The runtime must still apply the document's eval policy.
        let scripts = [Script {
            node: 0,
            label: "CSP Promise fixture".into(),
            source: r#"
Promise.resolve().then(function(){return eval("document.title='forbidden'");})
.catch(function(error){if(error.name!=='EvalError')throw error;document.title='CSP retained';});
"#
            .into(),
        }];
        let outcome = page.start_with_policy(&mut doc, &mut forms, &scripts, &policy);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "CSP retained");
        let (mut doc, mut forms, mut page, outcome) = make_page("<p></p>", &[
            "Promise.reject('visible rejection');Promise.reject('handled').catch(function(){});"
        ]);
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert!(page.report().contains("visible rejection"));
        let outcome = page.console_eval(&mut doc, &mut forms, "'next job';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }

    #[test]
    fn promise_resolution_explicitly_accepts_current_document_nodes_and_globals() {
        let (doc, _, page, outcome) = make_page(
            "<p id=out></p>",
            &[r#"
var node=document.getElementById('out'),order='';
Promise.resolve(node).then(function(value){if(value!==node)throw 'node identity';order+='node;';});
Promise.resolve(document).then(function(value){if(value!==document)throw 'document identity';order+='document;';});
Promise.resolve(window).then(function(value){if(value!==window)throw 'window identity';order+='window;';});
queueMicrotask(function(){if(order!=='node;document;window;')throw order;document.title='native resolution passed';});
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "native resolution passed");
        let (doc, _, page, outcome) = make_page(
            "<p></p>",
            &[r#"
var called=0;window.then=function(resolve){
    if(this!==window)throw 'native then receiver';called++;resolve('native thenable passed');
};
Promise.resolve(window).then(function(value){
    if(called!==1)throw 'duplicate native call';document.title=value;
});
if(called!==0)throw 'synchronous native then call';
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "native thenable passed");
    }

    #[test]
    fn string_work_snapshot_survives_checkpoint_recovery_and_full_logs() {
        let (mut doc, mut forms, mut page, outcome) = make_page("<p></p>", &[]);
        assert_eq!(outcome.errors, 0);
        page.runtime.define_global(
            "payload", Value::String("PRIVATE_ATTRIBUTE_MARKER".repeat(256)),
        ).unwrap();
        page.runtime.execute(
            "var tail=0,sentinel=0;function burn(){var i=0;while(i<64){payload.length;i++;}tail=1;}",
            &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state },
        ).unwrap();
        page.begin_job();
        page.runtime.constrain_job(
            250_000, 16384, 2_000_000, Instant::now() + Duration::from_secs(1),
        );
        page.runtime.execute(
            "Promise.resolve().then(burn);Promise.resolve().then(function(){sentinel=1;});",
            &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state },
        ).unwrap();
        page.microtask_checkpoint(&mut doc, &mut forms);
        assert_eq!(page.error_count(), 1);
        assert!(!page.runtime.has_microtasks());
        let snapshot = page.runtime.string_work_failure_report().unwrap();
        assert!(snapshot.len() <= 2048);
        assert!(!snapshot.contains("PRIVATE_ATTRIBUTE_MARKER"));
        assert!(!snapshot.contains("Promise.resolve"));
        assert!(snapshot.contains("effective=16384;"));
        let report = page.report();
        assert!(report.contains("Microtask checkpoint: JavaScript error"), "{report}");
        assert_eq!(report.matches("String/copy work attribution:").count(), 1);
        let outcome = page.console_eval(&mut doc, &mut forms, "tail+sentinel;");
        assert_eq!(outcome.errors, 0);
        assert_eq!(page.runtime.string_work_failure_report().as_deref(), Some(snapshot.as_str()));
        let usage = page.runtime.job_usage();
        for _ in 0..MAX_LOG_LINES + 1 {
            page.state.log(&"full-log witness".repeat(200));
        }
        for _ in 0..2 {
            let report = page.report();
            assert!(report.contains(&snapshot));
            assert!(report.len() <= MAX_LOG_BYTES + 1024);
            assert_eq!(report.matches("String/copy work attribution:").count(), 1);
            assert_eq!(page.runtime.job_usage(), usage);
        }
        page.clear_console();
        assert_eq!(page.runtime.string_work_failure_report().as_deref(), Some(snapshot.as_str()));
        assert_eq!(page.error_count(), 1);
        assert_eq!(page.runtime.job_usage(), usage);
        // A replacement document, unlike a fresh job, owns a new runtime.
        page.start(&mut doc, &mut forms, &[]);
        assert!(page.runtime.string_work_failure_report().is_none());
    }

}
