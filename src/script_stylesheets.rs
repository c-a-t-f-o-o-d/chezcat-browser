//! Created LINK stylesheets use the existing CSS renderer and real, bounded
//! HTTP responses. Attribute changes invalidate old completions; no stylesheet
//! or load event is invented when a request is blocked or canceled.
use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const MAX_STYLESHEET_REQUESTS: usize = crate::loader::MAX_STYLESHEETS;

#[derive(Clone, Debug)]
pub struct DynamicStylesheet {
    pub limits: crate::css::StylesheetLimits,
    pub node: usize,
    pub generation: u64,
    /// Immutable request mode and source lifetime, shared with the HTTP worker.
    pub same_origin_only: bool,
    pub cancel: Arc<AtomicBool>,
    pub target: crate::navigation::Location,
    pub origin: crate::navigation::Location,
    pub nonce: Option<String>,
    pub policy: crate::csp::PolicySet,
    pub error: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct SheetKey {
    href: String,
    nonce: Option<String>,
    integrity: Option<String>,
    crossorigin: Option<String>,
}

#[derive(Default)]
struct SheetEntry {
    key: Option<SheetKey>,
    generation: u64,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct StylesheetState {
    pub pending: VecDeque<DynamicStylesheet>,
    pub removed: HashSet<usize>,
    entries: HashMap<usize, SheetEntry>,
    attempts: usize,
    pub source_bytes: usize,
    pub source_rules: usize,
    pub in_flight: usize,
}

/// The fragment slice admits no LINK attributes, including seemingly inert
/// metadata. Later author mutations use the normal checked stylesheet path.
pub(super) fn bare_fragment_link(doc: &Document, node: usize) -> bool {
    doc.nodes.get(node).is_some_and(|item|
        item.namespace == crate::html::Namespace::Html && item.children.is_empty()
            && matches!(&item.kind, NodeKind::Element {tag, attrs, ..} if tag == "link" && attrs.is_empty()))
}

impl StylesheetState {
    /// A keyless retired entry still owns generation/cancellation history and
    /// possibly a worker completion outside `pending`. Never erase that state
    /// through fragment replacement. A pristine empty entry has never issued
    /// a request, including when an inert attribute change created the entry.
    fn permits_bare_fragment_removal(&self, node: usize) -> bool {
        !self.removed.contains(&node) && !self.pending.iter().any(|request| request.node == node)
            && self.entries.get(&node).is_none_or(|entry|
                entry.key.is_none() && entry.generation == 0 && !entry.cancel.load(Ordering::Acquire))
    }
}

pub(super) fn link_attribute(property: &str) -> Option<&'static str> {
    Some(match property {
        "href" => "href",
        "rel" => "rel",
        "type" => "type",
        "media" => "media",
        "as" => "as",
        "nonce" => "nonce",
        "integrity" => "integrity",
        "crossOrigin" => "crossorigin",
        "referrerPolicy" => "referrerpolicy",
        "fetchPriority" => "fetchpriority",
        "disabled" => "disabled",
        _ => return None,
    })
}

impl Page {
    /// Include already retained external styles in the dynamic-source budget.
    pub fn set_stylesheet_usage(&mut self, usage: crate::css::StylesheetUsage) {
        self.state.stylesheets.source_bytes = usage.bytes;
        self.state.stylesheets.source_rules = usage.rules;
    }

    pub fn take_dynamic_stylesheets(&mut self) -> VecDeque<DynamicStylesheet> {
        std::mem::take(&mut self.state.stylesheets.pending)
    }

    pub fn take_stylesheet_removals(&mut self) -> HashSet<usize> {
        std::mem::take(&mut self.state.stylesheets.removed)
    }

    pub(super) fn clear_stylesheet_requests(&mut self) {
        self.state.stylesheets.pending.clear();
        self.state.stylesheets.in_flight = 0;
        for entry in self.state.stylesheets.entries.values_mut() {
            entry.cancel.store(true, Ordering::Release);
            entry.generation = entry.generation.saturating_add(1);
        }
    }

    /// Generation ownership remains meaningful after an author load callback.
    pub fn owns_dynamic_stylesheet(&self, request: &DynamicStylesheet) -> bool {
        self.state.stylesheets.entries.get(&request.node)
            .is_some_and(|entry| entry.key.is_some()
                && entry.generation == request.generation
                && Arc::ptr_eq(&entry.cancel, &request.cancel)
                && !request.cancel.load(Ordering::Acquire))
    }

    pub fn accepts_dynamic_stylesheet(&self, doc: &Document, request: &DynamicStylesheet) -> bool {
        self.state.enabled && doc.scripting_enabled && self.owns_dynamic_stylesheet(request)
    }

    pub fn complete_dynamic_stylesheet(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        styles: &mut HashMap<usize, String>,
        request: &DynamicStylesheet,
        result: Result<String, String>,
    ) -> Outcome {
        if !self.state.enabled || !doc.scripting_enabled {
            return Outcome::default();
        }
        self.begin_job();
        let before = self.state.errors;
        self.state.stylesheets.in_flight = self.state.stylesheets.in_flight.saturating_sub(1);
        let current = self.owns_dynamic_stylesheet(request);
        let mut changed = false;
        if current {
            let result = result.and_then(|source| {
                crate::css::check_stylesheet_candidate(doc,styles,request.node,&source)?;
                let next=request.limits.accept(&source,crate::css::StylesheetUsage{bytes:self.state.stylesheets.source_bytes,rules:self.state.stylesheets.source_rules})?;
                Ok((source,next))
            });
            let event = match result {
                Ok((source,next)) => {
                    // Count every accepted source, even if later removed. This
                    // prevents repeated href changes from resetting the budget.
                    self.state.stylesheets.source_bytes = next.bytes;
                    self.state.stylesheets.source_rules = next.rules;
                    if let Some(geometry) = &mut self.state.geometry {
                        geometry.set_stylesheet(request.node, Some(&source));
                    }
                    styles.insert(request.node, source);
                    // App must recompute selected backgrounds before this
                    // callback or the load-ready check can finish window load.
                    self.defer_backgrounds_for_layout(true);
                    changed = true;
                    "load"
                }
                Err(error) => {
                    self.state
                        .error(&format!("Dynamic stylesheet node {}", request.node), &error);
                    "error"
                }
            };
            let outcome = self.dispatch(doc, forms, request.node, event, false);
            self.defer_backgrounds_for_layout(outcome.layout_changed);
            changed |= outcome.layout_changed;
        }
        if self.state.dynamic.load_ready {
            changed |= self.finish_loading(doc, forms).layout_changed;
        }
        Outcome {
            prevented: false,
            layout_changed: changed | std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }
}

impl Bridge<'_> {
    pub(super) fn removable_fragment_link(&mut self, node: usize) -> Result<bool, String> {
        self.state.work(1 + self.state.stylesheets.pending.len())?;
        Ok(bare_fragment_link(self.doc, node) && self.state.stylesheets.permits_bare_fragment_removal(node))
    }

    pub(super) fn stylesheet_get(
        &mut self,
        node: usize,
        property: &str,
    ) -> Result<Option<Value>, String> {
        if !self.dom_stylesheet(node) {
            return Ok(None);
        }
        let Some(attribute) = link_attribute(property) else {
            return Ok(None);
        };
        if attribute == "disabled" {
            return Ok(Some(Value::Bool(self.doc.attr(node, attribute).is_some())));
        }
        let raw = self.doc.attr(node, attribute).unwrap_or("").to_owned();
        let value = if attribute == "href" && !raw.is_empty() {
            self.state
                .dynamic
                .base
                .as_ref()
                .and_then(|base| crate::navigation::resolve(&raw, base).ok())
                .map_or(raw, |target| target.display())
        } else {
            raw
        };
        self.string(&value).map(Some)
    }

    pub(super) fn stylesheet_set(
        &mut self,
        node: usize,
        property: &str,
        value: &Value,
    ) -> Result<bool, String> {
        if !self.dom_stylesheet(node) {
            return Ok(false);
        }
        let Some(attribute) = link_attribute(property) else {
            return Ok(false);
        };
        let value = if attribute == "disabled" {
            value.truthy().then(String::new)
        } else {
            Some(self.text(value)?)
        };
        self.attribute(node, attribute, value)?;
        self.queue_dynamic_stylesheet(node)?;
        Ok(true)
    }

    pub(super) fn stylesheet_attribute_change(
        &mut self,
        node: usize,
        name: &str,
        value: Option<&str>,
    ) -> Result<bool, String> {
        if !self.dom_stylesheet(node) {
            return Ok(false);
        }
        if !matches!(
            name,
            "href"
                | "rel"
                | "type"
                | "media"
                | "as"
                | "nonce"
                | "integrity"
                | "crossorigin"
                | "referrerpolicy"
                | "fetchpriority"
                | "disabled"
        ) {
            return Ok(false);
        }
        self.state.copies(value.map_or(0, str::len))?;
        self.attribute(node, name, value.map(str::to_owned))?;
        self.queue_dynamic_stylesheet(node)?;
        Ok(true)
    }

    pub(super) fn queue_dynamic_stylesheet(&mut self, node: usize) -> Result<(), String> {
        if !self.dom_stylesheet(node) {
            return Ok(());
        }
        self.state.rebuild(self.doc)?;
        let rel = self.doc.attr(node, "rel").unwrap_or("");
        let media = self.doc.attr(node, "media").unwrap_or("").trim();
        let kind = self.doc.attr(node, "type").unwrap_or("").trim();
        let href = self.doc.attr(node, "href").unwrap_or("");
        let enabled = self.state.dom.connected.get(node).copied().unwrap_or(false)
            && self.state.tree.active.get(node).copied().unwrap_or(false)
            && self.doc.attr(node, "disabled").is_none()
            && rel
                .split_ascii_whitespace()
                .any(|word| word.eq_ignore_ascii_case("stylesheet"))
            && !rel
                .split_ascii_whitespace()
                .any(|word| word.eq_ignore_ascii_case("alternate"))
            && (kind.is_empty() || kind.eq_ignore_ascii_case("text/css"))
            && (media.is_empty()
                || media.eq_ignore_ascii_case("all")
                || media.eq_ignore_ascii_case("screen"))
            && !href.trim().is_empty();
        if enabled {
            // A key is retained beside the DOM attributes and cloned into a
            // prepared request. Account those copies to the same host job.
            let bytes = href.len()
                + crate::csp::nonce(self.doc, node).map_or(0, str::len)
                + self.doc.attr(node, "integrity").map_or(0, str::len)
                + self.doc.attr(node, "crossorigin").map_or(0, str::len);
            self.state.copies(bytes.saturating_mul(2))?;
        }
        let key = enabled.then(|| SheetKey {
            href: href.to_owned(),
            nonce: crate::csp::nonce(self.doc, node).map(str::to_owned),
            integrity: self.doc.attr(node, "integrity").map(str::to_owned),
            crossorigin: self.doc.attr(node, "crossorigin").map(str::to_owned),
        });
        let entry = self.state.stylesheets.entries.entry(node).or_default();
        if entry.key == key {
            return Ok(());
        }
        // Retire the exact old source before a worker can follow another hop.
        // Token identity also keeps ownership exact if the counter saturates.
        entry.cancel.store(true, Ordering::Release);
        entry.cancel = Arc::new(AtomicBool::new(false));
        entry.generation = entry.generation.saturating_add(1);
        entry.key = key.clone();
        let generation = entry.generation;
        let cancel = entry.cancel.clone();
        self.state.stylesheets.removed.insert(node);
        if let Some(geometry) = &mut self.state.geometry {
            geometry.set_stylesheet(node, None);
        }
        let before = self.state.stylesheets.pending.len();
        self.state
            .stylesheets
            .pending
            .retain(|request| request.node != node);
        self.state.stylesheets.in_flight = self
            .state
            .stylesheets
            .in_flight
            .saturating_sub(before - self.state.stylesheets.pending.len());
        let Some(key) = key else { return Ok(()) };
        if self.state.stylesheets.attempts >= MAX_STYLESHEET_REQUESTS {
            self.state.error(
                "Dynamic stylesheet",
                &format!("Dynamic stylesheet request limit reached ({MAX_STYLESHEET_REQUESTS} per document)."),
            );
            return Ok(());
        }
        self.state.stylesheets.attempts += 1;
        let origin = self
            .state
            .location
            .as_ref()
            .map(|location| location.source.clone())
            .unwrap_or_else(crate::navigation::Location::home);
        let same_origin_only = key.crossorigin.is_some();
        let prepare = (|| {
            if key
                .integrity
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                return Err("Subresource Integrity is not implemented; the declared-integrity stylesheet was not fetched.".to_owned());
            }
            let target = crate::navigation::resolve(
                &key.href,
                self.state.dynamic.base.as_ref().unwrap_or(&origin),
            )?;
            let url = target
                .url
                .as_ref()
                .ok_or("Dynamic stylesheets currently require HTTP or HTTPS URLs.")?;
            let origin_url = origin.url.as_ref()
                .ok_or("Local pages do not automatically fetch network stylesheets.")?;
            crate::http::check_stylesheet_request(origin_url, url, same_origin_only)?;
            self.state
                .dynamic
                .policy
                .allows_external_style(node, key.nonce.as_deref(), &target)?;
            Ok(target)
        })();
        let (target, error) = match prepare {
            Ok(target) => (target, None),
            Err(error) => (origin.clone(), Some(error)),
        };
        self.state.stylesheets.pending.push_back(DynamicStylesheet {
            limits: self.doc.stylesheet_limits,
            node,
            generation,
            same_origin_only,
            cancel,
            target,
            origin,
            nonce: key.nonce,
            policy: self.state.dynamic.policy.clone(),
            error,
        });
        self.state.stylesheets.in_flight += 1;
        Ok(())
    }
}

#[cfg(test)]
#[path = "script_inert_link_fragment_tests.rs"]
mod inert_fragment_tests;

#[cfg(test)]
mod source_lifetime_tests {
    use super::*;
    fn start()->(Document,Forms,Page) {
        let mut doc=crate::html::parse("<html><head></head><body><div id=target>Original target</div></body></html>");doc.scripting_enabled=true;
        let mut forms=Forms::new(&doc);let mut page=Page::for_test();
        let origin=crate::navigation::address("https://original.test/page",&crate::navigation::Location::home()).unwrap();
        page.set_location(&origin);page.set_resource_base(&origin);
        let result=page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original stylesheet lifetime".into(),source:"var loads=0,errors=0,sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/one.css';sheet.onload=function(){loads++;};sheet.onerror=function(){errors++;};document.head.appendChild(sheet);".into()}]);
        assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
    }
    fn eval(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
        let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{}",page.report());
    }
    #[test]
    fn stylesheet_source_mutations_retire_exact_request_and_snapshot_crossorigin_mode() {
        for (mutation,marked) in [("sheet.href='/two.css';",false),("sheet.nonce='fresh';",false),("sheet.crossOrigin='anonymous';",true)] {
            let (mut doc,mut forms,mut page)=start();
            let old=page.take_dynamic_stylesheets().pop_front().unwrap();assert!(!old.same_origin_only);
            eval(&mut doc,&mut forms,&mut page,mutation);
            let next=page.take_dynamic_stylesheets().pop_front().unwrap();
            assert!(old.cancel.load(Ordering::Acquire));assert!(!next.cancel.load(Ordering::Acquire));
            assert!(!page.owns_dynamic_stylesheet(&old));assert!(page.owns_dynamic_stylesheet(&next));
            assert_eq!(next.same_origin_only,marked);assert!(!old.same_origin_only,"prepared request mode changed");
            assert_eq!(page.state.stylesheets.in_flight,2);
            let mut styles=HashMap::new();
            let stale=page.complete_dynamic_stylesheet(&mut doc,&mut forms,&mut styles,&old,Ok("#target{width:999px}".into()));
            assert_eq!(stale.errors,0);assert!(styles.is_empty());assert_eq!(page.state.stylesheets.in_flight,1);
            let current=page.complete_dynamic_stylesheet(&mut doc,&mut forms,&mut styles,&next,Ok("#target{width:73px}".into()));
            assert_eq!(current.errors,0);assert_eq!(styles.get(&next.node).map(String::as_str),Some("#target{width:73px}"));
            assert_eq!(page.state.stylesheets.in_flight,0);
            eval(&mut doc,&mut forms,&mut page,"if(loads!==1||errors!==0)throw 'stale stylesheet events';");
        }
    }
    #[test]
    fn stylesheet_disable_detach_and_stop_retire_active_source_without_phantom_tasks() {
        for action in ["sheet.disabled=true;","document.head.removeChild(sheet);","stop"] {
            let(mut doc,mut forms,mut page)=start();let old=page.take_dynamic_stylesheets().pop_front().unwrap();
            if action=="stop" {page.clear_host_requests();} else {eval(&mut doc,&mut forms,&mut page,action);}
            assert!(old.cancel.load(Ordering::Acquire));assert!(!page.owns_dynamic_stylesheet(&old));assert!(page.take_dynamic_stylesheets().is_empty());
            // App consumes the actual cascade-removal notification separately
            // from the worker completion; a standalone Page must do so too.
            let removed=page.take_stylesheet_removals();assert_eq!(removed.len(),1);assert!(removed.contains(&old.node));
            let mut styles=HashMap::new();let out=page.complete_dynamic_stylesheet(&mut doc,&mut forms,&mut styles,&old,Ok("#target{width:999px}".into()));
            assert_eq!(out.errors,0);assert!(styles.is_empty());assert_eq!(page.state.stylesheets.in_flight,0);assert!(!page.has_host_tasks());
            eval(&mut doc,&mut forms,&mut page,"if(loads!==0||errors!==0)throw 'retired stylesheet event';");
        }
    }
    #[test]
    fn stylesheet_source_token_preserves_ownership_when_generation_counter_saturates() {
        let(mut doc,mut forms,mut page)=start();let old=page.take_dynamic_stylesheets().pop_front().unwrap();
        page.state.stylesheets.entries.get_mut(&old.node).unwrap().generation=u64::MAX;
        eval(&mut doc,&mut forms,&mut page,"sheet.href='/two.css';");
        let first=page.take_dynamic_stylesheets().pop_front().unwrap();assert_eq!(first.generation,u64::MAX);
        eval(&mut doc,&mut forms,&mut page,"sheet.href='/three.css';");
        let second=page.take_dynamic_stylesheets().pop_front().unwrap();assert_eq!(second.generation,u64::MAX);
        assert!(first.cancel.load(Ordering::Acquire));assert!(!page.owns_dynamic_stylesheet(&first));assert!(page.owns_dynamic_stylesheet(&second));
    }
}
