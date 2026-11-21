//! Transactional inline-style batches over the existing CSP/source planner.
use super::*;
pub(in crate::scripting) struct InlineSheetBatch {
    plans: Vec<(usize, InlineSheetPlan)>,
    total: crate::css::StylesheetUsage,
}
impl Bridge<'_> {
    pub(super) fn reserve_inline_withheld(&mut self, additional: usize) -> Result<(), String> {
        let before = self.state.stylesheets.withheld_capacity;
        let reported = self.doc.withheld_inline_styles.capacity();
        let required = self
            .doc
            .withheld_inline_styles
            .len()
            .checked_add(additional)
            .ok_or("Stylesheet flag count overflow.")?;
        let slot = std::mem::size_of::<usize>();
        if required <= reported {
            if reported > before {
                self.state.replace_storage(
                    0,
                    (reported - before)
                        .checked_mul(slot)
                        .ok_or("Stylesheet flag storage overflow.")?,
                    0,
                )?;
                self.state.stylesheets.withheld_capacity = reported;
            }
            return Ok(());
        }
        let target = required.max(before);
        let minimum = target
            .saturating_sub(before)
            .checked_mul(slot)
            .ok_or("Stylesheet flag size overflow.")?;
        if minimum
            > self
                .state
                .retained_limit
                .saturating_sub(self.state.retained)
        {
            return Err("Stylesheet flag retained allowance exhausted.".into());
        }
        self.state
            .work(before.saturating_mul(2).saturating_add(additional))?;
        let mut flags = HashSet::new();
        flags
            .try_reserve(target)
            .map_err(|_| "Cannot reserve stylesheet flags.")?;
        let added = flags
            .capacity()
            .checked_sub(before)
            .and_then(|slots| slots.checked_mul(slot))
            .ok_or("Stylesheet flag capacity overflow.")?;
        let moved = self
            .doc
            .withheld_inline_styles
            .len()
            .checked_mul(slot)
            .ok_or("Stylesheet flag copy overflow.")?;
        self.state.replace_storage(0, added, moved)?;
        flags.extend(self.doc.withheld_inline_styles.drain());
        self.state.stylesheets.withheld_capacity = flags.capacity();
        self.doc.withheld_inline_styles = flags;
        Ok(())
    }
    pub(in crate::scripting) fn validate_style_destination(
        &mut self,
        parent: usize,
    ) -> Result<(), String> {
        let mut current = Some(parent);
        while let Some(node) = current.filter(|&node| node != 0) {
            self.state.work(1)?;
            if !self.fragment_receiver(node_handle(node)) {
                let name = tag(self.doc, node);
                if self.doc.nodes[node].namespace != crate::html::Namespace::Html
                    || crate::html::is_void(name)
                    || dom_tree::inert_dom_tag(name, self.doc.scripting_enabled)
                {
                    return Err("Inline stylesheet connection requires an ordinary HTML or fragment parent.".into());
                }
            }
            current = self.state.tree.parents[node];
        }
        Ok(())
    }
    pub(in crate::scripting) fn prepare_style_transfers(
        &mut self,
        incoming: &[usize],
        outgoing: &[usize],
        connected: bool,
    ) -> Result<InlineSheetBatch, String> {
        let count = incoming
            .iter()
            .chain(outgoing)
            .filter(|&&node| tag(self.doc, node) == "style")
            .count();
        let mut changes = Vec::new();
        changes
            .try_reserve_exact(count)
            .map_err(|_| "Cannot reserve inline stylesheet changes.")?;
        self.state
            .work(incoming.len().saturating_add(outgoing.len()))?;
        for &node in outgoing {
            if tag(self.doc, node) == "style" {
                changes.push((node, false));
            }
        }
        for &node in incoming {
            if tag(self.doc, node) == "style" {
                changes.push((node, connected));
            }
        }
        self.prepare_inline_sheet_batch(changes)
    }
    pub(in crate::scripting) fn prepare_inline_sheet_batch(
        &mut self,
        mut changes: Vec<(usize, bool)>,
    ) -> Result<InlineSheetBatch, String> {
        let original = crate::css::StylesheetUsage {
            bytes: self.state.stylesheets.source_bytes,
            rules: self.state.stylesheets.source_rules,
        };
        if changes.is_empty() {
            return Ok(InlineSheetBatch {
                plans: Vec::new(),
                total: original,
            });
        }
        let count = changes.len();
        let sorting = count.saturating_mul((usize::BITS - count.leading_zeros()) as usize + 1);
        self.state.work(sorting)?;
        self.state.copies(
            sorting
                .checked_mul(std::mem::size_of::<(usize, bool)>())
                .ok_or("Stylesheet change sort overflow.")?,
        )?;
        changes.sort_unstable_by_key(|&(node, connected)| (node, connected));
        let mut kept = 0;
        for index in 0..changes.len() {
            let change = changes[index];
            if kept != 0 && changes[kept - 1].0 == change.0 {
                changes[kept - 1].1 |= change.1;
            } else {
                changes[kept] = change;
                kept += 1;
            }
        }
        changes.truncate(kept);
        self.state.reserve_stylesheet_source_slots(changes.len())?;
        self.reserve_inline_withheld(changes.len())?;
        let mut old = Vec::new();
        let mut plans = Vec::new();
        old.try_reserve_exact(changes.len())
            .map_err(|_| "Cannot reserve stylesheet source journal.")?;
        plans
            .try_reserve_exact(changes.len())
            .map_err(|_| "Cannot reserve inline stylesheet plans.")?;
        let scratch = changes
            .capacity()
            .checked_mul(std::mem::size_of::<(usize, bool)>())
            .and_then(|bytes| {
                old.capacity()
                    .checked_mul(std::mem::size_of::<(usize, crate::css::StylesheetUsage)>())
                    .and_then(|old| bytes.checked_add(old))
            })
            .and_then(|bytes| {
                plans
                    .capacity()
                    .checked_mul(std::mem::size_of::<(usize, InlineSheetPlan)>())
                    .and_then(|plans| bytes.checked_add(plans))
            })
            .filter(|bytes| {
                *bytes
                    <= self
                        .state
                        .retained_limit
                        .saturating_sub(self.state.retained)
            })
            .ok_or("Inline stylesheet batch scratch allowance exhausted.")?;
        self.state.copies(scratch)?;
        let mut used = original;
        for &(node, _) in &changes {
            self.state.work(1)?;
            if !self.doc.nodes.get(node).is_some_and(|entry| {
                entry.namespace == crate::html::Namespace::Html
                    && matches!(&entry.kind,NodeKind::Element{tag,..} if tag=="style")
            }) {
                return Err("Inline stylesheet plan requires real HTML style nodes.".into());
            }
            if let Some(source) = self.state.stylesheets.sources.get(&node).copied() {
                used.bytes = used
                    .bytes
                    .checked_sub(source.bytes)
                    .ok_or("Stylesheet byte ownership mismatch.")?;
                used.rules = used
                    .rules
                    .checked_sub(source.rules)
                    .ok_or("Stylesheet rule ownership mismatch.")?;
                old.push((node, source));
            }
        }
        // No author callback or renderer runs during native preflight. Remove
        // only this batch's old contributions, project each new accepted source,
        // and restore the journal even when a later sheet refuses admission.
        for &(node, _) in &old {
            self.state.stylesheets.sources.remove(&node);
        }
        self.state.stylesheets.source_bytes = used.bytes;
        self.state.stylesheets.source_rules = used.rules;
        let result = (|| {
            for (node, connected) in changes {
                let plan = self.prepare_inline_sheet(node, connected)?;
                if let InlineSheetPlan::Admitted { total, .. } = &plan {
                    self.state.stylesheets.source_bytes = total.bytes;
                    self.state.stylesheets.source_rules = total.rules;
                }
                plans.push((node, plan));
            }
            Ok::<_, String>(())
        })();
        let total = crate::css::StylesheetUsage {
            bytes: self.state.stylesheets.source_bytes,
            rules: self.state.stylesheets.source_rules,
        };
        for (node, source) in old {
            self.state.stylesheets.sources.insert(node, source);
        }
        self.state.stylesheets.source_bytes = original.bytes;
        self.state.stylesheets.source_rules = original.rules;
        result?;
        Ok(InlineSheetBatch { plans, total })
    }
    pub(in crate::scripting) fn commit_inline_sheet_batch(&mut self, batch: InlineSheetBatch) {
        for (node, plan) in batch.plans {
            match plan {
                InlineSheetPlan::Disconnected => {
                    self.doc.withheld_inline_styles.remove(&node);
                    self.state.stylesheets.sources.remove(&node);
                }
                InlineSheetPlan::Withheld(reason) => {
                    self.doc.withheld_inline_styles.insert(node);
                    self.state.stylesheets.sources.remove(&node);
                    if let Some(reason) = reason {
                        self.state.log(&reason);
                    }
                }
                InlineSheetPlan::Admitted { source, .. } => {
                    self.doc.withheld_inline_styles.remove(&node);
                    self.state.stylesheets.sources.insert(node, source);
                }
            }
        }
        self.state.stylesheets.source_bytes = batch.total.bytes;
        self.state.stylesheets.source_rules = batch.total.rules;
    }
}
