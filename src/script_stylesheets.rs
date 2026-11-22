//! Live LINK stylesheets use the existing CSS renderer and real, bounded
//! HTTP responses. Attribute changes invalidate old completions; no stylesheet
//! or load event is invented when a request is blocked or canceled.
use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

pub type DynamicStylesheet = Arc<StylesheetRequest>;

#[derive(Debug)]
struct PendingStorage { used: Arc<AtomicUsize>, bytes: usize }
impl Drop for PendingStorage {
    fn drop(&mut self) { self.used.fetch_sub(self.bytes, Ordering::AcqRel); }
}

#[derive(Debug)]
pub struct StylesheetRequest {
    pub limits: crate::css::StylesheetLimits,
    pub node: usize,
    pub generation: u64,
    /// Immutable request mode and source lifetime, shared with the HTTP worker.
    pub same_origin_only: bool,
    pub cancel: Arc<AtomicBool>,
    pub target: crate::navigation::Location,
    pub origin: crate::navigation::Location,
    pub nonce: Option<String>,
    pub policy: Arc<crate::csp::PolicySet>,
    pub error: Option<String>,
    pub storage_limit: usize,
    _storage: PendingStorage,
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
    pending_storage: Arc<AtomicUsize>,
    policy: Option<Arc<crate::csp::PolicySet>>,
    pub source_bytes: usize,
    pub source_rules: usize,
    pub sources: HashMap<usize, crate::css::StylesheetUsage>,
    pub source_capacity: usize,
    pub withheld_capacity: usize,
    pub sources_prepared: bool,
    pub source_setup_error: Option<String>,
    pub in_flight: usize,
}

#[path="script_style_batches.rs"]
mod batches;

pub(super) enum InlineSheetPlan {
    Disconnected,
    Withheld(Option<String>),
    Admitted { source: crate::css::StylesheetUsage, total: crate::css::StylesheetUsage },
}

pub(super) fn validate_inline_sheet_media(raw: Option<&str>) -> Result<(), String> {
    let media = raw.unwrap_or("").trim_matches([' ', '\t', '\r', '\n', '\u{c}']);
    if media.is_empty() || ["all", "screen", "print"].iter().any(|kind| media.eq_ignore_ascii_case(kind)) {
        Ok(())
    } else {
        Err("Style element media currently supports the empty, all, screen and print media types.".into())
    }
}

/// The fragment slice admits no LINK attributes, including seemingly inert
/// metadata. Later author mutations use the normal checked stylesheet path.
pub(super) fn bare_fragment_link(doc: &Document, node: usize) -> bool {
    doc.nodes.get(node).is_some_and(|item|
        item.namespace == crate::html::Namespace::Html && item.children.is_empty()
            && matches!(&item.kind, NodeKind::Element {tag, attrs, ..} if tag == "link" && attrs.is_empty()))
}

impl StylesheetState {
    pub(super) fn source_metadata_bytes(&self) -> usize {
        self.source_capacity * std::mem::size_of::<(usize, crate::css::StylesheetUsage)>() + self.withheld_capacity*std::mem::size_of::<usize>()
    }

    fn retire_source(&mut self, node: usize) {
        if let Some(source) = self.sources.remove(&node) {
            self.source_bytes = self.source_bytes.checked_sub(source.bytes).expect("owned stylesheet bytes");
            self.source_rules = self.source_rules.checked_sub(source.rules).expect("owned stylesheet rules");
        }
    }

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

impl State {
    fn prepare_stylesheet_source(
        &mut self, node: usize, source: &str, limits: crate::css::StylesheetLimits,
    ) -> Result<(crate::css::StylesheetUsage, crate::css::StylesheetUsage), String> {
        let old = self.stylesheets.sources.get(&node).copied().unwrap_or_default();
        let used = crate::css::StylesheetUsage {
            bytes: self.stylesheets.source_bytes.checked_sub(old.bytes).ok_or("Stylesheet byte ownership mismatch.")?,
            rules: self.stylesheets.source_rules.checked_sub(old.rules).ok_or("Stylesheet rule ownership mismatch.")?,
        };
        let total = limits.accept(source, used)?;
        let record = crate::css::StylesheetUsage { bytes: total.bytes - used.bytes, rules: total.rules - used.rules };
        self.reserve_stylesheet_source_slots(usize::from(!self.stylesheets.sources.contains_key(&node)))?;
        Ok((record, total))
    }

    fn reserve_stylesheet_source_slots(&mut self,added:usize)->Result<(),String>{
        let required=self.stylesheets.sources.len().checked_add(added).ok_or("Stylesheet source count overflow.")?;
        if required<=self.stylesheets.sources.capacity(){return Ok(());}
        let slot=std::mem::size_of::<(usize,crate::css::StylesheetUsage)>();let before=self.stylesheets.source_capacity;let available=self.retained_limit.saturating_sub(self.retained);
        self.work(before.saturating_mul(2))?;
        let target=if required<=before{before}else{before.checked_add(before.max(4).max(required-before).min(available/slot)).filter(|target|*target>=required).ok_or("Stylesheet source ownership storage exhausted.")?};
        let mut replacement=HashMap::new();replacement.try_reserve(target).map_err(|_|"Cannot allocate stylesheet source ownership.")?;
        let bytes=replacement.capacity().checked_sub(before).and_then(|slots|slots.checked_mul(slot)).ok_or("Stylesheet ownership capacity overflow.")?;
        let copied=self.stylesheets.sources.len().checked_mul(slot).ok_or("Stylesheet ownership movement overflow.")?;
        self.replace_storage(0,bytes,copied)?;replacement.extend(self.stylesheets.sources.drain());self.stylesheets.source_capacity=replacement.capacity();self.stylesheets.sources=replacement;Ok(())
    }

    fn commit_stylesheet_source(&mut self, node: usize, source: crate::css::StylesheetUsage, total: crate::css::StylesheetUsage) {
        self.stylesheets.sources.insert(node, source);
        self.stylesheets.source_bytes = total.bytes;
        self.stylesheets.source_rules = total.rules;
    }
}

impl Page {
    /// Prepare initial ownership before start_document resets execution state.
    pub fn set_stylesheet_sources(&mut self, doc: &Document, external: &HashMap<usize, String>) {
        let result = (|| -> Result<_, String> {
            let (accepted, _, total) = crate::css::accepted_stylesheets(doc, external);
            let mut records = HashMap::new();
            records.try_reserve(accepted.len()).map_err(|_| "Cannot reserve stylesheet source ownership.")?;
            let bytes = records.capacity().checked_mul(std::mem::size_of::<(usize, crate::css::StylesheetUsage)>()).and_then(|bytes|doc.withheld_inline_styles.capacity().checked_mul(std::mem::size_of::<usize>()).and_then(|flags|bytes.checked_add(flags)))
                .ok_or("Stylesheet source ownership size overflow.")?;
            if bytes > self.state.limits.host.retained_bytes {
                return Err("Stylesheet source ownership exceeds host retained storage.".into());
            }
            for (node, source) in accepted {
                let usage = doc.stylesheet_limits.accept(&source, crate::css::StylesheetUsage::default())?;
                records.insert(node, usage);
            }
            Ok((records, total))
        })();
        self.state.stylesheets.sources_prepared = true;
        self.state.stylesheets.withheld_capacity=doc.withheld_inline_styles.capacity();
        match result {
            Ok((records, total)) => {
                self.state.stylesheets.source_capacity = records.capacity();
                self.state.stylesheets.sources = records;
                self.state.stylesheets.source_bytes = total.bytes;
                self.state.stylesheets.source_rules = total.rules;
                self.state.stylesheets.source_setup_error = None;
            }
            Err(error) => self.state.stylesheets.source_setup_error = Some(error),
        }
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
                // The immutable fetch profile still applies to this complete
                // source; document retention uses the current connected set.
                request.limits.accept(&source,crate::css::StylesheetUsage::default())?;
                crate::css::check_stylesheet_candidate(doc,styles,request.node,&source)?;
                let (record, total) = self.state.prepare_stylesheet_source(request.node, &source, doc.stylesheet_limits)?;
                Ok((source,record,total))
            });
            let event = match result {
                Ok((source,record,total)) => {
                    // Replaced/retired sources do not consume a lifetime byte
                    // or rule allowance. Network and work accounting is separate.
                    self.state.commit_stylesheet_source(request.node, record, total);
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
    // https://html.spec.whatwg.org/multipage/semantics.html#update-a-style-block
    // The node stays real on CSP/MIME refusal. Source-limit failures precede
    // mutation and never publish a partial sheet or displace a preceding one.
    pub(super) fn prepare_inline_sheet(&mut self, node: usize, connected: bool) -> Result<InlineSheetPlan, String> {
        if !connected { return Ok(InlineSheetPlan::Disconnected); }
        let entry = &self.doc.nodes[node];
        let NodeKind::Element { attrs, .. } = &entry.kind else { return Err("Expected a style element.".into()); };
        self.state.work(attrs.len() + entry.children.len())?;
        for attr in attrs {
            self.state.work(attr.name.bytes().saturating_add(attr.value.len()).div_ceil(64))?;
        }
        let mut bytes = 0usize;
        for &child in &entry.children {
            let (NodeKind::Text(value) | NodeKind::Cdata(value)) = &self.doc.nodes[child].kind else {
                return Err("Created style elements currently admit only direct text children.".into());
            };
            bytes = bytes.checked_add(value.len()).ok_or("Stylesheet source size overflow.")?;
        }
        validate_inline_sheet_media(self.doc.attr(node, "media"))?;
        if self.doc.attr(node, "type").is_some_and(|kind| !kind.is_empty() && !kind.eq_ignore_ascii_case("text/css")) {
            self.reserve_inline_withheld(1)?;
            return Ok(InlineSheetPlan::Withheld(None));
        }
        if bytes > self.doc.stylesheet_limits.sheet_bytes {
            return Err("Complete inline stylesheet exceeds its byte allowance.".into());
        }
        let mut meter = crate::css::CssBudget::new(&mut self.state.host_work, &mut self.state.copy_work);
        meter.source(bytes)?;
        let source = crate::css::inline_stylesheet(self.doc, node)?;
        if let Err(reason) = self.state.dynamic.policy.allows_inline_style(
            usize::MAX, crate::csp::nonce(self.doc, node), &source,
        ) {
            self.reserve_inline_withheld(1)?;
            return Ok(InlineSheetPlan::Withheld(Some(reason)));
        }
        let (source, total) = self.state.prepare_stylesheet_source(node, &source, self.doc.stylesheet_limits)?;
        Ok(InlineSheetPlan::Admitted { source, total })
    }



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
        let retained_source = self.state.stylesheets.sources.contains_key(&node);
        let entry = self.state.stylesheets.entries.entry(node).or_default();
        if entry.key == key && !(key.is_none() && retained_source) {
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
        self.state.stylesheets.retire_source(node);
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
                .allows_external_style(usize::MAX, key.nonce.as_deref(), &target)?;
            Ok(target)
        })();
        let (target, error) = match prepare {
            Ok(target) => (target, None),
            Err(error) => (origin.clone(), Some(error)),
        };
        let policy = match &self.state.stylesheets.policy {
            Some(policy) if policy.as_ref() == &self.state.dynamic.policy => policy.clone(),
            _ => {
                let policy = Arc::new(self.state.dynamic.policy.clone());
                self.state.stylesheets.policy = Some(policy.clone());
                policy
            }
        };
        let bytes = std::mem::size_of::<StylesheetRequest>()
            .saturating_add(target.payload_bytes()).saturating_add(origin.payload_bytes())
            .saturating_add(key.nonce.as_ref().map_or(0, String::capacity))
            .saturating_add(error.as_ref().map_or(0, String::capacity));
        let storage_limit = self.state.limits.host.retained_bytes;
        let in_flight = self.state.stylesheets.in_flight.checked_add(1)
            .ok_or("Dynamic stylesheet in-flight counter overflow.")?;
        let used = self.state.stylesheets.pending_storage.clone();
        used.try_update(Ordering::AcqRel, Ordering::Acquire,
            |used| used.checked_add(bytes).filter(|used| *used <= storage_limit))
            .map_err(|_| "Dynamic stylesheet pending-record storage exhausted.")?;
        let storage = PendingStorage { used, bytes };
        self.state.stylesheets.pending.try_reserve(1)
            .map_err(|_| "Cannot reserve dynamic stylesheet requests.")?;
        self.state.stylesheets.pending.push_back(Arc::new(StylesheetRequest {
            limits: self.doc.stylesheet_limits,
            node,
            generation,
            same_origin_only,
            cancel,
            target,
            origin,
            nonce: key.nonce,
            policy,
            error,
            storage_limit,
            _storage: storage,
        }));
        self.state.stylesheets.in_flight = in_flight;
        Ok(())
    }
}

#[cfg(test)]
#[path = "script_inert_link_fragment_tests.rs"]
mod inert_fragment_tests;

#[cfg(test)]
mod inline_connection_tests {
    use super::*;

    #[test]
    fn stylesheet_initial_ownership_metadata_refuses_before_scripts_and_recovers() {
        let mut doc = crate::html::parse("<html><head><style>body{color:red}</style></head><body></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let limit = page.state.limits.host.retained_bytes;
        page.state.limits.host.retained_bytes = 0;
        let source = Script { node: 0, source: "var reached=1;".into(), label: "Original stylesheet ownership".into() };
        assert_eq!(page.start(&mut doc, &mut forms, std::slice::from_ref(&source)).errors, 1);
        assert_eq!(page.state.executed, 0);
        assert!(page.report().contains("source ownership exceeds host retained storage"));
        page.state.limits.host.retained_bytes = limit;
        page.set_stylesheet_sources(&doc, &HashMap::new());
        assert_eq!(page.start(&mut doc, &mut forms, &[source]).errors, 0, "{}", page.report());
        assert_eq!(page.state.executed, 1);
        assert_eq!(page.state.retained, document_bytes(&doc) + page.state.tree_storage_bytes() + page.state.stylesheets.source_metadata_bytes());
    }

    #[test]
    fn stylesheet_source_ownership_is_rebuilt_for_a_new_document_start() {
        let mut page = Page::for_test();
        for text in ["body{color:red}", "p{color:blue}"] {
            let mut doc = crate::html::parse(&format!("<head><style>{text}</style></head><body></body>"));
            doc.scripting_enabled = true;
            let mut forms = Forms::new(&doc);
            assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
            assert_eq!(page.state.stylesheets.source_bytes, text.len());
            assert_eq!(page.state.stylesheets.source_rules, 1);
            assert!(!page.state.stylesheets.sources_prepared);
        }
    }

    #[test]
    fn stylesheet_source_ownership_growth_refusal_preserves_old_records_and_recovers() {
        let mut doc = crate::html::parse("<head><style>a{color:red}</style><style>b{color:red}</style><style>c{color:red}</style></head><body></body>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        assert_eq!(page.state.stylesheets.sources.len(), page.state.stylesheets.sources.capacity());
        let before = (page.state.retained, page.state.stylesheets.source_bytes, page.state.stylesheets.source_rules);
        let limit = page.state.retained_limit;
        page.state.retained_limit = page.state.retained;
        let refused = page.state.prepare_stylesheet_source(usize::MAX, "d{color:red}", doc.stylesheet_limits);
        assert!(refused.unwrap_err().contains("ownership storage exhausted"));
        assert_eq!(before, (page.state.retained, page.state.stylesheets.source_bytes, page.state.stylesheets.source_rules));
        assert_eq!(page.state.stylesheets.sources.len(), 3);
        page.state.retained_limit = limit;
        let (source, total) = page.state.prepare_stylesheet_source(usize::MAX, "d{color:red}", doc.stylesheet_limits).unwrap();
        page.state.commit_stylesheet_source(usize::MAX, source, total);
        assert_eq!(page.state.stylesheets.source_rules, 4);
        assert!(page.state.retained > before.0);
        page.state.stylesheets.retire_source(usize::MAX);
        assert_eq!(page.state.stylesheets.source_bytes, before.1);
        assert_eq!(page.state.stylesheets.source_rules, before.2);
    }

    #[test]
    fn stylesheet_source_churn_reuses_the_owned_map_reservation_without_spare_bytes() {
        let mut doc = crate::html::parse("<head></head><body></body>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        for node in 0..256 {
            let (record, total) = page.state.prepare_stylesheet_source(node, "a{color:red}", doc.stylesheet_limits).unwrap();
            page.state.commit_stylesheet_source(node, record, total);
        }
        let retained = page.state.retained;
        let metadata = page.state.stylesheets.source_metadata_bytes();
        page.state.retained_limit = retained;
        for node in 0..2048 {
            if node % 128 == 0 { page.begin_job(); }
            page.state.stylesheets.retire_source(node);
            let (record, total) = page.state.prepare_stylesheet_source(node + 256, "a{color:red}", doc.stylesheet_limits).unwrap();
            page.state.commit_stylesheet_source(node + 256, record, total);
        }
        assert_eq!(page.state.stylesheets.source_rules, 256);
        assert_eq!(page.state.stylesheets.source_bytes, 256 * "a{color:red}".len());
        assert_eq!(page.state.retained, retained);
        assert_eq!(page.state.stylesheets.source_metadata_bytes(), metadata);
    }

    #[test]
    fn stylesheet_request_storage_transfers_through_clones_and_refunds_on_drop() {
        let mut doc = crate::html::parse("<html><head></head><body></body></html>");
        doc.scripting_enabled = true;
        let mut page = Page::for_test();
        let mut forms = Forms::new(&doc);
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        let outcome = page.console_eval(&mut doc, &mut forms, "var link=document.createElement('link');link.rel='stylesheet';link.href='https://styles.test/sheet.css';document.head.appendChild(link);");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let used = page.state.stylesheets.pending_storage.clone();
        let before = used.load(Ordering::Acquire);
        assert!(before >= std::mem::size_of::<StylesheetRequest>());
        let requests = page.take_dynamic_stylesheets();
        let saved = requests.front().unwrap().clone();
        page.clear_stylesheet_requests();
        drop(requests);
        assert_eq!(used.load(Ordering::Acquire), before);
        drop(saved);
        assert_eq!(used.load(Ordering::Acquire), 0);
        let limit = page.state.limits.host.retained_bytes;
        page.state.limits.host.retained_bytes = 0;
        let outcome = page.console_eval(&mut doc, &mut forms,
            "var refused=document.createElement('link');refused.rel='stylesheet';refused.href='https://styles.test/first.css';document.head.appendChild(refused);");
        assert_eq!(outcome.errors, 1);
        assert!(page.report().contains("pending-record storage exhausted"));
        assert!(page.state.stylesheets.pending.is_empty());
        assert_eq!(used.load(Ordering::Acquire), 0);
        page.state.limits.host.retained_bytes = limit;
        let outcome = page.console_eval(&mut doc, &mut forms,
            "refused.href='https://styles.test/recovery.css';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(used.load(Ordering::Acquire) > 0);
        page.clear_stylesheet_requests();
        assert_eq!(used.load(Ordering::Acquire), 0);
    }

    #[test]
    fn inline_style_connection_refuses_complete_sheet_before_changing_tree_or_budget() {
        let mut doc = crate::html::parse("<html><head><link id=base rel=stylesheet href='base.css'></head><body><input id=field value=seed></body></html>");
        doc.scripting_enabled = true;
        let original = "#field{color:red}";
        let extra = "#field{opacity:.5}";
        let link = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("base")).unwrap();
        let external = HashMap::from([(link, original.to_owned())]);
        doc.stylesheet_limits = crate::css::StylesheetLimits {
            sheet_bytes: 512, document_bytes: original.len() + extra.len(), rules: 2,
        };
        let mut page = Page::for_test();
        let mut forms = Forms::new(&doc);
        page.set_stylesheet_sources(&doc, &external);
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        let setup = format!("var f=document.getElementById('field'); f.value='edited'; var s=document.createElement('style'); s.textContent='{extra}'; var t=s.firstChild,list=s.childNodes; document.head.appendChild(s);");
        let result = page.console_eval(&mut doc, &mut forms, &setup);
        assert_eq!(result.errors, 0, "{}", page.report());
        let connected = page.console_eval(&mut doc, &mut forms,
            "s.media='all';var mediaAttr=s.getAttributeNode('media');var other=document.createAttribute('media');other.value='(width: 300px)';");
        assert_eq!(connected.errors, 0, "{}", page.report());
        let connected_commits = page.state.dom_commits;
        for source in ["s.media='screen and (width: 2px)';", "mediaAttr.value='(width: 300px)';",
                       "s.setAttributeNode(other);", "s.setAttributeNS(null,'media','screen, print');",
                       "t.data='body{display:none}';", "t.nodeValue='';", "s.textContent='';",
                       "s.appendChild(document.body);"] {
            let result = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(result.errors, 1, "{source}: {}", page.report());
            assert_eq!(page.state.dom_commits, connected_commits);
        }
        let checked = page.console_eval(&mut doc, &mut forms,
            "if(s.media!=='all'||s.getAttributeNode('media')!==mediaAttr||other.ownerElement!==null||s.firstChild!==t||t.data!==s.textContent||!t.isConnected||!s.isConnected) throw 'partial connected mutation';");
        assert_eq!(checked.errors, 0, "{}", page.report());
        let result = page.console_eval(&mut doc, &mut forms, "document.head.insertBefore(s,document.getElementById('base'));document.head.removeChild(s);");
        assert_eq!(result.errors, 0, "{}", page.report());
        let usage = (page.state.stylesheets.source_bytes, page.state.stylesheets.source_rules);
        assert_eq!(usage, (original.len(), 1));
        let result = page.console_eval(&mut doc, &mut forms, "document.head.insertBefore(s,document.getElementById('base'));");
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!((page.state.stylesheets.source_bytes, page.state.stylesheets.source_rules), (original.len() + extra.len(), 2));
        let check = page.console_eval(&mut doc, &mut forms,
            "if(s.parentNode!==document.head||!s.isConnected||t.parentNode!==s||list[0]!==t||s.textContent!==t.data||f.value!=='edited'||f.defaultValue!=='seed') throw 'sheet reinsertion';var excess=document.createElement('style');excess.textContent='p{color:blue}';");
        assert_eq!(check.errors, 0, "{}", page.report());
        let commits = page.state.dom_commits;
        let result = page.console_eval(&mut doc, &mut forms, "document.head.appendChild(excess);");
        assert_eq!(result.errors, 1, "{}", page.report());
        assert!(page.report().contains("per document"), "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        assert_eq!(page.console_eval(&mut doc, &mut forms, "if(excess.parentNode!==null||excess.isConnected)throw 'over-budget insertion committed';").errors, 0);
        let (accepted, errors, _) = crate::css::accepted_stylesheets(&doc, &external);
        assert!(errors.is_empty());
        assert_eq!(accepted.len(), 2);
        assert_eq!(accepted[1].0, link);
        assert_eq!(accepted[1].1, original);
        assert!(!page.has_host_tasks());
    }
}

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

#[cfg(test)]
#[path="script_style_import_tests.rs"]
mod import_tests;
