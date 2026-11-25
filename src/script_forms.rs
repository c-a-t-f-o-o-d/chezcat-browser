//! HTMLFormElement.submit() snapshots supported form data before yielding to
//! App's existing navigation queue. No script-controlled request starts here.
//! requestSubmit shares this serializer and queue after synchronous validation
//! and trusted submit-event delivery in the current caller job.
use super::*;

impl Bridge<'_> {
    pub(super) fn form_receiver(&self, object: u32) -> bool {
        node_index(object)
            .is_some_and(|node| node < self.doc.nodes.len() && tag(self.doc, node) == "form")
    }

    pub(super) fn form_submit_method(&self, object: u32) -> Option<&'static str> {
        if !self.form_receiver(object) { return None; }
        // Native form named properties override prototype methods. A field
        // named or identified as "submit" must remain that actual field.
        if self.form_named_control(object,"submit") {
            None
        } else {
            Some("submit")
        }
    }

    pub(super) fn form_named_control(&self, object:u32, name:&str) -> bool {
        let Some(form) = node_index(object).filter(|_|self.form_receiver(object)) else {return false;};
        self.forms.controls.iter().any(|(&node, control)| {
            control.owner == Some(form)
                && control.kind != Kind::Image
                && (self.doc.dom_attr(node,"id").is_some_and(|value|value==name)
                    || self.doc.dom_attr(node,"name").is_some_and(|value|value==name))
        })
    }

    pub(super) fn submit_form(&mut self, object: u32) -> Result<Value, String> {
        let form = self.node(object)?;
        if tag(self.doc, form) != "form" {
            return Err("submit() requires a form receiver.".into());
        }
        self.queue_form_submission(form,None,false)
    }

    pub(super) fn queue_form_submission(&mut self,form:usize,submitter:Option<usize>,validated:bool)->Result<Value,String>{
        if !self.state.dom.connected.get(form).copied().unwrap_or(false) {
            return Ok(Value::Undefined); // A detached form cannot navigate.
        }
        if self.state.navigation_requests >= 16 {
            return Err("Script navigation request limit reached (16 per document).".into());
        }
        let current = self
            .state
            .location
            .as_ref()
            .ok_or("Script form submission requires a committed document URL.")?
            .source
            .clone();
        let origin = current
            .url
            .as_ref()
            .ok_or("Script form submission requires an HTTP or HTTPS document.")?;
        let base = self.state.dynamic.base.as_ref().unwrap_or(&current).clone();
        self.state.work(self.forms.controls.len())?;
        let copy_work = self.forms.controls.values().fold(0usize, |total, control| {
            total
                .saturating_add(control.name.len())
                .saturating_add(control.value.chars.len().saturating_mul(4))
        });
        // Serialization percent-encodes at most three output bytes per input
        // byte. Charge its bounded work and the maximum CSP snapshot together.
        self.state
            .copies(copy_work.saturating_mul(3).saturating_add(64 * 1024))?;
        let submission=if validated{
            self.forms.validated_submit_request(self.doc,form,submitter,&current,&base)?
        }else{self.forms.script_submit_request(self.doc,form,&current,&base)?};
        if submission
            .location
            .url
            .as_ref()
            .is_none_or(|target| !target.same_origin(origin))
        {
            return Err("Script form submission must keep the document's scheme, host and port in this build.".into());
        }
        self.state
            .dynamic
            .policy
            .allows_form_action(&submission.location)?;
        self.state.navigation_requests += 1;
        self.state.navigation = Some(Navigation {
            target: submission.location,
            request: submission.request,
            form_policy: Some(self.state.dynamic.policy.clone()),
            replace: false,
            reload: false,
            fragment_navigation: false,
        });
        Ok(Value::Undefined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html:&str,source:&str,policy:&str)->Page{run_with_page(html,source,policy,Page::for_test())}
    fn run_with_page(html: &str, source: &str, policy: &str,mut page:Page) -> Page {
        let mut doc = crate::html::parse(html);
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let location = crate::navigation::address(
            "https://example.test/home",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        page.set_resource_base(&location);
        let mut policies = crate::csp::PolicySet::new(&location);
        if !policy.is_empty() {
            policies.add_header(policy);
        }
        page.start_with_policy(
            &mut doc,
            &mut forms,
            &[Script {
                node: usize::MAX,
                source: source.into(),
                label: "form submit fixture".into(),
            }],
            &policies,
        );
        page
    }

    #[test]
    fn form_interface_real_prototype_submit_named_controls_and_saved_roots() {
        let page=run_with_page("<body><form name=f><input name=constructor value=constructor-field><input name=submit value=submit-field><input name=inheritedProbe value=probe-field><input name=getAttribute value=method-field><input name=children value=children-field></form><form name=other></form></body>",r#"
var f=document.f,other=document.other,C=HTMLFormElement,P=C.prototype;
if(!(f instanceof C)||!(f instanceof HTMLElement)||Object.getPrototypeOf(f)!==P||Object.getPrototypeOf(P)!==HTMLElement.prototype||Object.getPrototypeOf(C)!==HTMLElement||other.constructor!==C||Object.prototype.toString.call(f)!=='[object HTMLFormElement]')throw 'genuine form interface';
if(f.getAttribute.value!=='method-field'||typeof f.children.length!=='number'||Element.prototype.getAttribute.call(f,'name')!=='f'||String(other)!=='[object HTMLFormElement]')throw 'named native member precedence';
if(f.constructor.value!=='constructor-field')throw 'named constructor';if(f.submit.value!=='submit-field')throw 'named submit';if(other.submit!==P.submit)throw 'canonical prototype submit';
var calls=0;Object.defineProperty(P,'inheritedProbe',{get:function(){calls++;return this;},configurable:true});
if(f.inheritedProbe.value!=='probe-field'||calls!==0||other.inheritedProbe!==other||calls!==1)throw 'named control precedence over author prototype';
var method=Object.getOwnPropertyDescriptor(P,'submit');if(method.value.name!=='submit'||method.value.length!==0||!method.enumerable||!method.writable||!method.configurable)throw 'canonical submit descriptor';
for(var call of [function(){new C();},function(){C();},function(){method.value.call(document.body);}]){var caught=false;try{call();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'native form brand';}
P.submit=function(){return this;};if(other.submit()!==other||f.submit.value!=='submit-field')throw 'ordinary prototype override';delete P.submit;
if(typeof other.submit!=='undefined'||'submit' in other||f.submit.value!=='submit-field')throw 'deleted prototype operation resurrected';Object.defineProperty(P,'submit',method);
var inert=document.implementation.createHTMLDocument('inert').createElement('form');if(!(inert instanceof C))throw 'real inert class';
try{method.value.call(inert);throw 'inert fake submission';}catch(e){if(!(e instanceof DOMException)||e.name!=='NotSupportedError')throw e;}
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);if(frame.contentWindow.HTMLFormElement===C||f instanceof frame.contentWindow.HTMLFormElement)throw 'owner interface identity';
delete window.HTMLFormElement;if(!(other instanceof C)||P.constructor!==C||f.constructor.value!=='constructor-field')throw 'saved canonical roots';
"#, "",Page::new());
        assert_eq!(page.error_count(),0,"{}",page.report());assert!(page.state.navigation.is_none());
    }

    #[test]
    fn form_submit_snapshots_values_bypasses_validation_and_omits_submitter() {
        let html = "<form id=f name=f action='/search'><input name=required required><textarea name=q>before</textarea><button name=go value=button>Search</button></form>";
        let mut page = run(html, "document.f.onsubmit=function(){throw Error('submit event must not run');};if(typeof document.f.submit!=='function' || !('submit' in document.f))throw Error('native method');if(document.f.getAttribute('action')!=='/search')throw Error('form attribute method');document.f.addEventListener('submit',function(){throw Error('unexpected listener');});document.f.submit();document.f.q.value='after';", "");
        assert_eq!(page.error_count(), 0, "{}", page.report());
        let navigation = page.take_navigation().unwrap();
        assert_eq!(
            navigation.target.display(),
            "https://example.test/search?required=&q=before"
        );
        assert!(!navigation.request.is_post());
        assert!(navigation.form_policy.is_some());
        assert!(!navigation.fragment_navigation);
    }

    #[test]
    fn form_submit_preserves_name_shadow_and_rejects_incompatible_receivers() {
        let mut page = run("<body><form name=f><input name=submit value=field></form><form name=other></form></body>", "if(document.f.submit.value!=='field')throw Error('name shadow lost');var caught=0;try{document.f.submit();}catch(e){if(e.name==='TypeError')caught++;}var fn=document.other.submit;try{fn.call(document.body);}catch(e){if(e.name==='TypeError')caught++;}try{fn();}catch(e){if(e.name==='TypeError')caught++;}if(caught!==3)throw Error('receiver validation '+caught);fn.call(document.f);", "");
        assert_eq!(page.error_count(), 0, "{}", page.report());
        assert_eq!(
            page.take_navigation().unwrap().target.display(),
            "https://example.test/home?submit=field"
        );
    }

    #[test]
    fn form_submit_policy_origin_and_navigation_limits_precede_queueing() {
        for (html, source, policy) in [
            ("<form name=f action='https://other.test/search'><input name=q value=private></form>", "document.f.submit();", ""),
            ("<form name=f action='http://example.test/search'></form>", "document.f.submit();", ""),
            ("<form name=f action='/search'></form>", "document.f.submit();", "form-action 'none'"),
            ("<form name=f action='/search'></form>", "for(var i=0;i<17;i++)document.f.submit();", ""),
        ] {
            let page = run(html, source, policy);
            assert_eq!(page.error_count(), 1, "{}", page.report());
            if !source.contains("17") { assert!(page.state.navigation.is_none()); }
        }
        let mut page = run(
            "<form name=f><input name=q value=first></form>",
            "document.f.submit();document.f.q.value='last';document.f.submit();",
            "default-src 'none'; script-src 'unsafe-inline'",
        );
        assert_eq!(page.error_count(), 0, "{}", page.report());
        assert_eq!(
            page.take_navigation().unwrap().target.display(),
            "https://example.test/home?q=last"
        );
    }

    #[test]
    fn local_form_submit_cannot_acquire_network_navigation() {
        let mut doc = crate::html::parse(
            "<form name=f action='https://example.test/search'><input name=q value=private></form>",
        );
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        page.set_location(&crate::navigation::Location::home());
        page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                source: "document.f.submit();".into(),
                label: "local form submit".into(),
            }],
        );
        assert_eq!(page.error_count(), 1, "{}", page.report());
        assert!(page.report().contains("HTTP or HTTPS document"));
        assert!(page.take_navigation().is_none());
    }
}
