//! Bounded dynamic external classic-script requests. Preparation happens on
//! the document thread; fetching belongs to App's cancelable worker. No request
//! succeeds merely because a script element was inserted into the DOM.

use super::*;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct DynamicScript {
    pub node: usize,
    pub target: crate::navigation::Location,
    pub origin: crate::navigation::Location,
    pub nonce: Option<String>,
    pub policy: crate::csp::PolicySet,
    pub same_origin_only: bool,
    /// Maximum source the document can admit when the request is prepared.
    /// Completion rechecks the remaining allowance after other scripts run.
    pub source_limit: usize,
    pub error: Option<String>,
}

#[derive(Default)]
pub(super) struct DynamicState {
    pub policy: crate::csp::PolicySet,
    pub base: Option<crate::navigation::Location>,
    pub pending: VecDeque<DynamicScript>,
    started: HashSet<usize>,
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
        self.state.executed += 1;
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

    pub fn take_dynamic_scripts(&mut self) -> VecDeque<DynamicScript> {
        std::mem::take(&mut self.state.dynamic.pending)
    }

    pub fn clear_host_requests(&mut self) {
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
                if script.source.len() > self.state.limits.capacity.source_bytes
                    || script.source.len()
                        > self.state.limits.capacity.retained_source.saturating_sub(self.state.dynamic.source_bytes)
                {
                    self.state.error(
                        "Dynamic script",
                        &format!("Script source exceeds the {} KiB per-script or {} MiB document budget.", self.state.limits.capacity.source_bytes / 1024, self.state.limits.capacity.retained_source / (1024 * 1024)),
                    );
                    "error"
                } else {
                    self.state.dynamic.source_bytes += script.source.len();
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
            || self.state.dynamic.started.contains(&node)
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
        self.state.dynamic.started.insert(node);
        if self.state.dynamic.script_count >= self.state.limits.capacity.programs {
            self.state.error(
                "Dynamic script",
                &format!("Document script attempt limit reached ({} including parser-loaded scripts).", self.state.limits.capacity.programs),
            );
            return Ok(());
        }
        self.state.dynamic.script_count += 1;
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
        self.state.dynamic.pending.push_back(DynamicScript {
            node,
            target,
            origin,
            nonce,
            policy: self.state.dynamic.policy.clone(),
            same_origin_only,
            source_limit: self.state.limits.capacity.source_bytes.min(
                self.state.limits.capacity.retained_source.saturating_sub(self.state.dynamic.source_bytes)
            ),
            error,
        });
        self.state.dynamic.in_flight += 1;
        Ok(())
    }
}
