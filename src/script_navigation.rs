//! Original, bounded Location requests and session-cookie DOM access.
//! URL resolution uses the browser's existing parser. The App executes a
//! navigation only after the current JavaScript job has returned.

use super::*;
use crate::navigation::Location;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Navigation {
    pub target: Location,
    pub request: crate::http::Request,
    pub form_policy: Option<crate::csp::PolicySet>,
    pub replace: bool,
    pub reload: bool,
    pub fragment_navigation: bool,
}

impl Page {
    pub fn set_cookie_session(&mut self, cookies: Arc<crate::cookies::Session>) {
        self.state.cookies = cookies;
    }

    pub fn take_navigation(&mut self) -> Option<Navigation> {
        self.state.navigation.take()
    }

    pub fn has_host_tasks(&self) -> bool {
        self.has_media_selection() || self.state.navigation.is_some()
            || self.state.history.pending.is_some()
            || !self.state.dynamic.pending.is_empty()
            || !self.state.stylesheets.pending.is_empty()
            || !self.state.stylesheets.removed.is_empty()
            || self.state.images.has_changes()
            || self.state.dom.pending_focus.is_some()
    }

    pub fn host_error(&mut self, label: &str, error: &str) {
        self.state.error(label, error);
    }
}

impl Bridge<'_> {
    pub(super) fn location_instance_owner(&self,object:u32)->Option<u32>{if object==LOCATION {self.state.location.is_some().then_some(WINDOW)}else{self.frame_location_owner(object)}}
    pub(super) fn location_string_setter(object: u32, property: &str) -> bool {
        matches!(object, WINDOW | DOCUMENT) && property == "location"
            || object == LOCATION && matches!(property, "href" | "hash" | "search")
    }

    fn queue_navigation(
        &mut self,
        reference: &str,
        replace: bool,
        reload: bool,
    ) -> Result<(), String> {
        let current = &self
            .state
            .location
            .as_ref()
            .ok_or("Location requires a committed document URL.")?
            .source;
        let origin = current
            .url
            .as_ref()
            .ok_or("Script navigation currently requires an HTTP or HTTPS document.")?;
        let target = crate::navigation::resolve(reference, current)?;
        if target
            .url
            .as_ref()
            .is_none_or(|url| !origin.same_origin(url))
        {
            return Err(
                "Script navigation must keep the document's scheme, host and port in this build."
                    .into(),
            );
        }
        if self.state.navigation_requests >= 16 {
            return Err("Script navigation request limit reached (16 per document).".into());
        }
        self.state.navigation_requests += 1;
        // Multiple requests in one job replace the outstanding request. The
        // executing realm is never destroyed underneath an active callback.
        self.state.navigation = Some(Navigation {
            target,
            request: crate::http::Request::Get,
            form_policy: None,
            // HTML Location-object navigate: auto becomes replace only before
            // complete loading without transient incumbent-window activation.
            // Capture here, not when App later drains the completed JS job.
            replace: replace || (!self.state.document_complete
                && !self.state.incumbent_has_transient_activation(Instant::now())),
            reload,
            fragment_navigation: reference.contains('#'),
        });
        Ok(())
    }

    pub(super) fn set_location_value(
        &mut self,
        object: u32,
        property: &str,
        value: &Value,
    ) -> Result<(), String> {
        let text = self.text(value)?;
        let reference = if object == LOCATION && property == "hash" {
            format!("#{}", text.strip_prefix('#').unwrap_or(&text))
        } else if object == LOCATION && property == "search" {
            let location = self
                .state
                .location
                .as_ref()
                .ok_or("Location is unavailable.")?;
            let query = text.strip_prefix('?').unwrap_or(&text);
            let mut reference = location.pathname.clone();
            if !text.is_empty() {
                reference.push('?');
                reference.push_str(query);
            }
            reference.push_str(&location.hash);
            reference
        } else {
            text
        };
        self.queue_navigation(&reference, false, false)
    }

    pub(super) fn location_call(&mut self, method: &str, args: &[Value]) -> Result<Value, String> {
        match method {
            "toString" => return self.location_property("href"),
            "assign" | "replace" => {
                let value = args.first().ok_or("Location navigation needs a URL.")?;
                let reference = self.text(value)?;
                self.queue_navigation(&reference, method == "replace", false)?;
            }
            "reload" => {
                let reference = self
                    .state
                    .location
                    .as_ref()
                    .ok_or("Location is unavailable.")?
                    .href
                    .clone();
                self.queue_navigation(&reference, true, true)?;
            }
            _ => return Err(format!("Location method {method:?} is not supported.")),
        }
        Ok(Value::Undefined)
    }

    pub(super) fn document_cookie(&mut self) -> Result<Value, String> {
        let text = match self
            .state
            .location
            .as_ref()
            .and_then(|l| l.source.url.as_ref())
        {
            Some(url) => self.state.cookies.document(url),
            None => String::new(),
        };
        self.string(&text)
    }

    pub(super) fn set_document_cookie(&mut self, value: &Value) -> Result<(), String> {
        let text = self.text(value)?;
        let Some(url) = self
            .state
            .location
            .as_ref()
            .and_then(|l| l.source.url.as_ref())
        else {
            // Cookie-averse local/about documents do not acquire network state.
            return Ok(());
        };
        if let Err(reason) = self.state.cookies.set_document(url, &text) {
            // Like a browser cookie setter, policy-rejected assignments do not
            // throw or expose cookie values through the diagnostic report.
            self.state
                .log(&format!("Document cookie declined: {reason}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Page {
        let mut doc = crate::html::parse("<html><head></head><body></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        page.set_location(
            &crate::navigation::address(
                "https://www.example.test/find?q=old#start",
                &Location::home(),
            )
            .unwrap(),
        );
        page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                source: source.into(),
                label: "navigation fixture".into(),
            }],
        );
        page
    }

    #[test]
    fn location_requests_preserve_current_document_until_app_commit() {
        for statement in [
            "location='/search?q=test';",
            "window.location='/search?q=test';",
            "document.location='/search?q=test';",
            "location.href='/search?q=test';",
            "location.assign({toString(){return '/search?q=test';}});",
            "location.replace('/search?q=test');",
        ] {
            let mut page = run(&format!("{statement}if(location.href!=='https://www.example.test/find?q=old#start')throw Error('premature commit');"));
            assert_eq!(page.error_count(), 0, "{}\n{}", statement, page.report());
            let request = page.take_navigation().unwrap();
            assert_eq!(
                request.target.display(),
                "https://www.example.test/search?q=test"
            );
            // These assignments execute before complete loading. Explicit
            // replace and unactivated auto both replace this bridge document.
            assert!(request.replace, "{statement}");
            assert!(!request.reload);
        }
        let mut page = run("location.search='?q=next';");
        assert_eq!(
            page.take_navigation().unwrap().target.display(),
            "https://www.example.test/find?q=next#start"
        );
        let mut page = run("location.hash='#end';");
        assert_eq!(
            page.take_navigation().unwrap().target.display(),
            "https://www.example.test/find?q=old#end"
        );
        let mut page = run("location.reload();");
        let request = page.take_navigation().unwrap();
        assert!(request.reload && request.replace);
        assert_eq!(
            request.target.display(),
            "https://www.example.test/find?q=old#start"
        );
    }

    fn deferred(source: &str) -> (crate::html::Document, Forms, Page) {
        let mut doc = crate::html::parse("<html><body><button id=b>Go</button></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        page.set_location(&crate::navigation::address(
            "https://www.example.test/bridge", &Location::home()).unwrap());
        page.start_before_load(&mut doc, &mut forms,
            &[Script { node: 0, source: source.into(), label: "history fixture".into() }],
            &crate::csp::PolicySet::default());
        assert_eq!(page.error_count(), 0, "{}", page.report());
        (doc, forms, page)
    }

    #[test]
    fn automatic_history_handling_is_captured_before_complete_loading() {
        for source in [
            "location.assign('/destination');",
            "document.addEventListener('DOMContentLoaded',function(){location.assign('/destination');});",
            "window.onload=function(){if(document.readyState!=='complete')throw Error('ready');location.assign('/destination');};",
            "window.onload=function(){Promise.resolve().then(function(){location.assign('/destination');});};",
        ] {
            let (mut doc, mut forms, mut page) = deferred(source);
            page.finish_loading(&mut doc, &mut forms);
            assert_eq!(page.error_count(), 0, "{source} {}", page.report());
            assert!(page.state.document_complete);
            assert!(page.take_navigation().unwrap().replace, "{source}");
        }
    }

    #[test]
    fn completed_document_assign_pushes_but_explicit_replace_still_replaces() {
        for (method, replacement) in [("assign", false), ("replace", true)] {
            let (mut doc, mut forms, mut page) = deferred(&format!(
                "document.addEventListener('probe',function(){{location.{method}('/destination');}});"));
            page.finish_loading(&mut doc, &mut forms);
            page.begin_job();
            page.dispatch_to(&mut doc, &mut forms, DOCUMENT, "probe", false, false);
            assert_eq!(page.error_count(), 0, "{}", page.report());
            assert_eq!(page.take_navigation().unwrap().replace, replacement);
        }
    }

    #[test]
    fn transient_activation_is_native_bounded_and_not_synthetic_dispatch() {
        let handler = "document.addEventListener('keydown',function(){location.assign('/destination');});";
        let (mut doc, mut forms, mut page) = deferred(handler);
        page.begin_job();
        page.keyboard(&mut doc, &mut forms, KeyboardInput {
            down: true, key: "Enter".into(), code: "Enter".into(), ..KeyboardInput::default()
        });
        assert_eq!(page.error_count(), 0, "{}", page.report());
        assert!(!page.take_navigation().unwrap().replace);
        assert!(page.state.has_transient_activation(Instant::now()));
        page.state.last_activation = Some(Instant::now() - TRANSIENT_ACTIVATION);
        page.begin_job();
        page.dispatch_to(&mut doc, &mut forms, DOCUMENT, "keydown", false, false);
        assert!(page.take_navigation().unwrap().replace);
        assert!(!page.state.has_transient_activation(Instant::now()));

        let mut page = run("document.addEventListener('click',function(){location.assign('/destination');});var e=document.createEvent('Event');e.initEvent('click',false,false);document.dispatchEvent(e); ");
        assert_eq!(page.error_count(), 0, "{}", page.report());
        assert!(page.state.last_activation.is_none());
        assert!(page.take_navigation().unwrap().replace);

        let (mut doc, mut forms, mut page) = deferred(handler);
        page.begin_job();
        page.keyboard(&mut doc, &mut forms, KeyboardInput {
            down: true, key: "Escape".into(), code: "Escape".into(), ..KeyboardInput::default()
        });
        assert!(page.state.last_activation.is_none());
        assert!(page.take_navigation().unwrap().replace);
    }

    #[test]
    fn saved_location_methods_use_incumbent_activation_not_owner_activation() {
        for operation in ["loc.assign('/destination');", "loc.href='/destination';",
            "assign.call(loc,'/destination');"] {
            for created_after_input in [false, true] {
                let (mut doc, mut forms, mut page) = deferred("");
                if created_after_input {page.notify_user_activation();}
                let result = page.console_eval(&mut doc, &mut forms, &format!(
                    "var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var child=f.contentWindow.Function('loc','assign',{:?});", operation));
                assert_eq!(result.errors, 0, "{}", page.report());
                if !created_after_input {page.notify_user_activation();}
                let result = page.console_eval(&mut doc, &mut forms,
                    "child(location,location.assign);");
                assert_eq!(result.errors, 0, "{operation} {}", page.report());
                // Existing same-origin descendants receive native activation;
                // a Window created after the input does not inherit it merely
                // by calling an activated owner's saved Location operation.
                assert_eq!(page.take_navigation().unwrap().replace, created_after_input,
                    "{operation}; created_after_input={created_after_input}");
                let result = page.console_eval(&mut doc, &mut forms,
                    "location.assign('/root-activated');");
                assert_eq!(result.errors, 0, "{}", page.report());
                assert!(!page.take_navigation().unwrap().replace);
            }
        }
    }

    #[test]
    fn activation_survives_nested_synthetic_dispatch_but_not_new_document() {
        let (mut doc, mut forms, mut page) = deferred(
            "document.addEventListener('click',function(){location.assign('/destination');});");
        page.notify_user_activation();
        let activated_at = page.state.last_activation;
        let result = page.console_eval(&mut doc, &mut forms,
            "var e=document.createEvent('Event');e.initEvent('click',false,false);document.dispatchEvent(e); ");
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(page.state.last_activation, activated_at);
        assert!(!page.take_navigation().unwrap().replace);
        page.begin_job();
        page.keyboard(&mut doc, &mut forms, KeyboardInput {
            down: false, key: "Enter".into(), ..KeyboardInput::default()
        });
        assert_eq!(page.state.last_activation, activated_at);
        page.start_before_load(&mut doc, &mut forms,
            &[Script {node:0,source:"location.assign('/new-document');".into(),label:"fresh".into()}],
            &crate::csp::PolicySet::default());
        assert!(page.state.last_activation.is_none());
        assert!(!page.state.document_complete);
        assert!(page.take_navigation().unwrap().replace);
    }

    #[test]
    fn location_network_boundary_and_navigation_allowance_remain_bounded() {
        for source in [
            "location.assign('file:///etc/passwd');",
            "location.href='javascript:throw 1';",
            "location.replace('https://other.test/');",
            "location.href='http://www.example.test/';",
            "for(var i=0;i<17;i++)location.assign('/search?q='+i);",
        ] {
            let page = run(source);
            assert_eq!(page.error_count(), 1, "{}\n{}", source, page.report());
        }
        let page = run("document.cookie='a=one; Path=/; Secure';if(document.cookie!=='a=one')throw Error('session cookie');document.cookie='a=; Path=/; Max-Age=0; Secure';if(document.cookie!=='')throw Error('deletion');");
        assert_eq!(page.error_count(), 0, "{}", page.report());
    }
}
