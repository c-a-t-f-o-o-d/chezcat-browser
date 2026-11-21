//! Bounded dynamic external classic-script requests. Preparation happens on
//! the document thread; fetching belongs to App's cancelable worker. No request
//! succeeds merely because a script element was inserted into the DOM.

use super::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize,Ordering};
use crate::script_storage::Lease;

pub type DynamicScript=Arc<ScriptRequest>;

#[derive(Debug)]
pub struct ScriptRequest {
    pub node: usize,
    pub target: crate::navigation::Location,
    pub origin: crate::navigation::Location,
    pub nonce: Option<String>,
    pub policy: Arc<crate::csp::PolicySet>,
    pub same_origin_only: bool,
    /// Maximum source the document can admit when the request is prepared.
    /// Completion rechecks the remaining allowance after other scripts run.
    pub source_limit: usize,
    pub error: Option<String>,
    pub storage_limit:usize,
    pub(crate) _storage:Lease,
    pub(crate) _policy_storage:Arc<Lease>,
}

pub struct ScriptBatch {
    entries:VecDeque<DynamicScript>,
    used:Arc<AtomicUsize>,
    storage:Lease,
}
impl Default for ScriptBatch {fn default()->Self{Self::empty(Arc::new(AtomicUsize::new(0)))}}
impl ScriptBatch {
    fn empty(used:Arc<AtomicUsize>)->Self{Self{entries:VecDeque::new(),storage:Lease::empty(used.clone()),used}}
    #[cfg(test)]
    pub fn pop_front(&mut self)->Option<DynamicScript>{self.entries.pop_front()}
    fn clear(&mut self){self.entries=VecDeque::new();self.storage=Lease::empty(self.used.clone());}
    fn prepare(&self,limit:usize)->Result<(Option<VecDeque<DynamicScript>>,Lease),String> {
        let before=self.entries.capacity();
        if self.entries.len()<before{return Ok((None,Lease::empty(self.used.clone())));}
        let available=limit.saturating_sub(self.used.load(Ordering::Acquire));let slot=size_of::<DynamicScript>();
        let growth=before.max(4).min(available/slot);
        let capacity=before.checked_add(growth).filter(|_|growth>0).ok_or("Dynamic script pending queue storage exhausted.")?;
        let mut next=VecDeque::new();next.try_reserve_exact(capacity).map_err(|_|"Could not reserve dynamic script pending queue.")?;
        let bytes=next.capacity().saturating_sub(before).saturating_mul(slot);
        let storage=Lease::new(self.used.clone(),bytes,limit)?;Ok((Some(next),storage))
    }
}
impl std::ops::Deref for ScriptBatch {type Target=VecDeque<DynamicScript>;fn deref(&self)->&Self::Target{&self.entries}}
pub struct ScriptBatchIter {entries:std::collections::vec_deque::IntoIter<DynamicScript>,_storage:Lease}
impl Iterator for ScriptBatchIter {type Item=DynamicScript;fn next(&mut self)->Option<Self::Item>{self.entries.next()}}
impl IntoIterator for ScriptBatch {type Item=DynamicScript;type IntoIter=ScriptBatchIter;fn into_iter(self)->Self::IntoIter{ScriptBatchIter{entries:self.entries.into_iter(),_storage:self.storage}}}

#[derive(Default)]
pub(super) struct DynamicState {
    pub policy: crate::csp::PolicySet,
    pub base: Option<crate::navigation::Location>,
    pub pending: ScriptBatch,
    started:Vec<u64>,
    started_storage:Option<Lease>,
    shared_policy:Option<(Arc<crate::csp::PolicySet>,Arc<Lease>)>,
    pub script_count: usize,
    pub source_bytes: usize,
    pub in_flight: usize,
    pub load_ready: bool,
}

impl Page {
    pub(super) fn run_classic_script(
        &mut self, doc: &mut Document, forms: &mut Forms, script: &Script,
        dynamic_node: Option<usize>,
    ) {
        self.state.executed=self.state.executed.saturating_add(1);
        let previous;
        let result = {
            let mut bridge = Bridge { doc, forms, state: &mut self.state };
            let node = script.node;
            let is_script_element = dynamic_node.is_none_or(|requested| requested == node)
                && bridge.dom_accessible(node)
                && matches!(bridge.doc.nodes[node].namespace, crate::html::Namespace::Html | crate::html::Namespace::Svg)
                && tag(bridge.doc, node) == "script";
            previous = std::mem::replace(&mut bridge.state.current_script, is_script_element.then_some(node));
            self.runtime.execute(&script.source, &mut bridge)
        };
        if let Err(error) = result {
            let kind = if dynamic_node.is_some() { "dynamic script" } else { "script" };
            self.state.error(&format!("{} ({kind} node {})", script.label, script.node), &error);
        }
        // The classic-script checkpoint is part of execution. Restore the
        // prior slot before later script jobs and resource/lifecycle events.
        self.microtask_checkpoint(doc, forms);
        self.state.current_script = previous;
    }

    pub fn set_resource_base(&mut self, base: &crate::navigation::Location) {
        self.state.dynamic.base = Some(base.clone());
    }

    pub fn take_dynamic_scripts(&mut self)->ScriptBatch {
        let used=self.state.dynamic.pending.used.clone();
        std::mem::replace(&mut self.state.dynamic.pending,ScriptBatch::empty(used))
    }

    pub fn clear_host_requests(&mut self) {
        self.state.xhr.cancel();
        self.clear_message_tasks();
        self.clear_stylesheet_requests();
        self.clear_image_requests();
        self.state.dynamic.pending.clear();
        self.state.dynamic.in_flight = 0;
        // Explicit host cancellation abandons the deferred document load.
        self.state.dynamic.load_ready = false;
        self.state.navigation = None;
        self.state.dom.pending_focus = None;
    }

    pub fn complete_dynamic_script(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        request: &DynamicScript,
        result: Result<Script, String>,
    ) -> Outcome {
        if !self.state.enabled || !doc.scripting_enabled {
            return Outcome::default();
        }
        self.begin_job();
        let before = self.state.errors;
        self.state.dynamic.in_flight = self.state.dynamic.in_flight.saturating_sub(1);
        let event = match result {
            Ok(script) => {
                #[cfg(test)]
                let total_refused=self.state.limits.test_entry_arenas&&script.source.len()>self.state.limits.capacity.retained_source.saturating_sub(self.state.dynamic.source_bytes);
                #[cfg(not(test))]
                let total_refused=false;
                if script.source.len()>self.state.limits.capacity.source_bytes||total_refused
                {
                    let error=if total_refused{format!("Dynamic script document source budget is full ({} MiB).",self.state.limits.capacity.retained_source/(1024*1024))}
                        else{format!("Dynamic script source exceeds the per-script allowance ({} KiB).",self.state.limits.capacity.source_bytes/1024)};
                    self.state.error("Dynamic script",&error);
                    "error"
                } else {
                    self.state.dynamic.source_bytes=self.state.dynamic.source_bytes.saturating_add(script.source.len());
                    self.run_classic_script(doc, forms, &script, Some(request.node));
                    "load"
                }
            }
            Err(error) => {
                self.state
                    .error(&format!("Dynamic script node {}", request.node), &error);
                "error"
            }
        };
        let outcome = self.dispatch(doc, forms, request.node, event, false);
        self.defer_backgrounds_for_layout(outcome.layout_changed || self.state.changed);
        let loading = if self.state.dynamic.load_ready {
            self.finish_loading(doc, forms)
        } else {
            Outcome::default()
        };
        Outcome {
            prevented: false,
            layout_changed: outcome.layout_changed
                | loading.layout_changed
                | std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }
}

impl Bridge<'_> {
    pub(super) fn queue_dynamic_script(&mut self, node: usize) -> Result<(), String> {
        self.state.rebuild(self.doc)?;
        if tag(self.doc, node) != "script"
            || !self.state.dom.created.contains(&node)
            || !self.state.dom.connected.get(node).copied().unwrap_or(false)
            || self.state.dynamic.started.get(node/64).is_some_and(|word|word&(1u64<<(node%64))!=0)
            || self.state.dom.fragment_scripts.contains(&node)
        {
            return Ok(());
        }
        let kind = self
            .doc
            .attr(node, "type")
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !kind.is_empty() && kind != "module" && !crate::loader::javascript_mime(&kind) {
            return Ok(()); // Non-JavaScript data blocks are inert.
        }
        let source = self.doc.attr(node, "src").map(str::to_owned);
        if source.is_none() && self.doc.nodes[node].children.iter().all(|&child| {
            !matches!(&self.doc.nodes[child].kind, NodeKind::Text(text) if !text.trim().is_empty())
        }) {
            return Ok(()); // An empty script can acquire src/text later.
        }
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&self.state.dynamic.script_count>=self.state.limits.capacity.programs {
            self.state.error(
                "Dynamic script",
                &format!("Document script attempt limit reached ({} including parser-loaded scripts).", self.state.limits.capacity.programs),
            );
            return Ok(());
        }
        let origin = self
            .state
            .location
            .as_ref()
            .map(|l| l.source.clone())
            .unwrap_or_else(crate::navigation::Location::home);
        let nonce = crate::csp::nonce(self.doc, node).map(str::to_owned);
        let same_origin_only = self.doc.attr(node, "crossorigin").is_some();
        let prepare = (|| {
            if kind == "module" {
                return Err("JavaScript modules are not implemented.".to_owned());
            }
            let reference = source.as_deref().ok_or(
                "Dynamic inline-script execution is not implemented; no source was executed.",
            )?;
            if reference.trim().is_empty() {
                return Err("Dynamic script src is empty.".into());
            }
            if self
                .doc
                .attr(node, "integrity")
                .is_some_and(|value| !value.trim().is_empty())
            {
                return Err("Subresource Integrity is not implemented; the declared-integrity script was not fetched.".into());
            }
            let target = crate::navigation::resolve(
                reference,
                self.state.dynamic.base.as_ref().unwrap_or(&origin),
            )?;
            let url = target
                .url
                .as_ref()
                .ok_or("Dynamic scripts currently require HTTP or HTTPS URLs.")?;
            let origin_url = origin
                .url
                .as_ref()
                .ok_or("Local pages do not automatically fetch network scripts.")?;
            crate::http::check_classic_script_mode(origin_url, url, same_origin_only)?;
            self.state
                .dynamic
                .policy
                .allows_external(node, nonce.as_deref(), &target)?;
            Ok(target)
        })();
        let (target, error) = match prepare {
            Ok(target) => (target, None),
            Err(error) => (origin.clone(), Some(error)),
        };
        let limit=self.state.limits.host.retained_bytes;
        let used=self.state.dynamic.pending.used.clone();
        let (policy,policy_storage)=match &self.state.dynamic.shared_policy {
            Some((policy,storage)) if policy.as_ref()==&self.state.dynamic.policy=>(policy.clone(),storage.clone()),
            _=>{
                let policy=self.state.dynamic.policy.clone();let bytes=policy.owned_bytes()+size_of::<Lease>()+4*size_of::<usize>();self.state.copies(bytes)?;
                let storage=Arc::new(Lease::new(used.clone(),bytes,limit)?);
                (Arc::new(policy),storage)
            }
        };
        let bytes=size_of::<ScriptRequest>()+2*size_of::<usize>()+target.payload_bytes()+origin.payload_bytes()+nonce.as_ref().map_or(0,String::capacity)+error.as_ref().map_or(0,String::capacity);
        self.state.copies(bytes)?;
        let storage=Lease::new(used.clone(),bytes,limit)?;
        let block=node/64;let required=block.checked_add(1).ok_or("Dynamic script identity bitmap overflow.")?;
        let queue_minimum=if self.state.dynamic.pending.len()==self.state.dynamic.pending.capacity(){size_of::<DynamicScript>()}else{0};
        let available=limit.saturating_sub(used.load(Ordering::Acquire)).checked_sub(queue_minimum).ok_or("Dynamic script queue backing exhausted.")?;
        let (bitmap,added)=retained_storage::grow(&self.state.dynamic.started,required.saturating_sub(self.state.dynamic.started.len()),available)?;
        let bitmap_storage=Lease::new(used.clone(),added,limit)?;
        self.state.work(required.saturating_sub(self.state.dynamic.started.len()))?;
        if bitmap.is_some(){self.state.copies(self.state.dynamic.started.len()*size_of::<u64>())?;}
        let (queue,queue_storage)=self.state.dynamic.pending.prepare(limit)?;
        if queue.is_some(){self.state.copies(self.state.dynamic.pending.len()*size_of::<DynamicScript>())?;}
        let in_flight=self.state.dynamic.in_flight.checked_add(1).ok_or("Dynamic script in-flight counter overflow.")?;
        #[cfg(test)]
        let source_limit=if self.state.limits.test_entry_arenas{self.state.limits.capacity.source_bytes.min(self.state.limits.capacity.retained_source.saturating_sub(self.state.dynamic.source_bytes))}else{self.state.limits.capacity.source_bytes};
        #[cfg(not(test))]
        let source_limit=self.state.limits.capacity.source_bytes;
        let request=Arc::new(ScriptRequest{node,target,origin,nonce,policy:policy.clone(),same_origin_only,source_limit,error,
            storage_limit:self.state.limits.capacity.script_storage_bytes,_storage:storage,_policy_storage:policy_storage.clone()});
        if let Some(mut next)=bitmap{next.append(&mut self.state.dynamic.started);self.state.dynamic.started=next;}
        if required>self.state.dynamic.started.len(){self.state.dynamic.started.resize(required,0);}
        if let Some(storage)=&mut self.state.dynamic.started_storage{storage.merge(bitmap_storage);}else{self.state.dynamic.started_storage=Some(bitmap_storage);}
        if let Some(mut next)=queue{next.append(&mut self.state.dynamic.pending.entries);self.state.dynamic.pending.entries=next;}
        self.state.dynamic.pending.storage.merge(queue_storage);
        self.state.dynamic.shared_policy=Some((policy,policy_storage));
        self.state.dynamic.started[block]|=1u64<<(node%64);
        self.state.dynamic.pending.entries.push_back(request);
        self.state.dynamic.script_count=self.state.dynamic.script_count.saturating_add(1);
        self.state.dynamic.in_flight=in_flight;
        Ok(())
    }
}

#[cfg(test)]
#[path="script_resource_storage_tests.rs"]
mod storage_tests;
