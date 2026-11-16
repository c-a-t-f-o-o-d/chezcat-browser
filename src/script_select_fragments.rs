//! Whole conventional select subtrees share the native form model. Existing
//! editors and selectedness are retained; prospective parent metadata is staged.
use super::*;

struct SelectView<'a> { document: &'a Document, fragment: Option<(&'a Document, usize)> }
impl SelectView<'_> {
    fn source(&self, node: usize) -> (&Document, usize) {
        match self.fragment { Some((fragment, base)) if node >= base => (fragment, node - base + 1), _ => (self.document, node) }
    }
    fn tag(&self, node: usize) -> &str { let (doc, node) = self.source(node); tag(doc, node) }
    fn attr(&self, node: usize, name: &str) -> Option<&str> { let (doc, node) = self.source(node); doc.attr(node, name) }
}
impl Bridge<'_> {
    // Only a complete known select subtree may cross a DOM boundary. Mutating
    // inside a select remains guarded by mutation_ancestors in this slice.
    pub(super) fn owned_select_member(&self, root: usize, node: usize) -> bool {
        let mut current = Some(node);
        for _ in 0..128 {
            let Some(index) = current else { break; };
            if tag(self.doc, index) == "select" {
                return self.forms.controls.get(&index).is_some_and(|control| control.kind == Kind::Select);
            }
            if index == root { break; }
            current = self.state.tree.parents.get(index).copied().flatten();
        }
        false
    }
    pub(super) fn prepare_select_form_change(&mut self, fragment: Option<(&Document, usize)>,
        overrides: &[(usize, Option<usize>)]) -> Result<forms::SelectFormPlan, String> {
        let mut affected = Vec::new();
        affected.try_reserve(self.forms.controls.len()).map_err(|_| "Select affected-control reservation failed.")?;
        for (&node, control) in &self.forms.controls {
            if control.kind != Kind::Select { continue; }
            let mut ancestor = Some(node);
            for _ in 0..128 {
                let Some(index) = ancestor else { break; };
                self.state.work(1)?;
                if overrides.iter().any(|(root, _)| *root == index) { affected.push(node); break; }
                ancestor = self.state.tree.parents.get(index).copied().flatten();
            }
        }
        let new_select = fragment.is_some_and(|(doc, _)| (1..doc.nodes.len()).any(|node| tag(doc, node) == "select"));
        if affected.is_empty() && !new_select {
            return Ok(forms::SelectFormPlan { additions: std::collections::BTreeMap::new(), metadata: Vec::new(), retained_bytes: 0 });
        }
        let extra = fragment.map_or(0, |(doc, _)| doc.nodes.len().saturating_sub(1));
        if fragment.is_some_and(|(_, base)| base != self.doc.nodes.len()) { return Err("Select fragment identity base changed.".into()); }
        self.state.work(self.doc.nodes.len().saturating_add(extra).saturating_add(self.forms.controls.len()))?;
        let mut plan = self.forms.prepare_selects(fragment)?;
        let mut parents = Vec::new();
        parents.try_reserve(self.doc.nodes.len().saturating_add(extra)).map_err(|_| "Select parent projection reservation failed.")?;
        parents.extend(self.state.tree.parents.iter().copied());
        parents.resize(self.doc.nodes.len() + extra, None);
        if let Some((doc, base)) = fragment {
            for local in 1..doc.nodes.len() {
                for &child in &doc.nodes[local].children { parents[base + child - 1] = Some(base + local - 1); }
            }
        }
        for &(node, parent) in overrides {
            if node >= parents.len() || parent.is_some_and(|node| node >= parents.len()) { return Err("Select parent projection is invalid.".into()); }
            parents[node] = parent;
        }
        let mut selects = Vec::new();
        selects.try_reserve(self.forms.controls.len().saturating_add(plan.additions.len())).map_err(|_| "Select owner plan reservation failed.")?;
        selects.extend(affected);
        selects.extend(plan.additions.keys().copied());
        let view = SelectView { document: self.doc, fragment };
        for node in selects {
            let mut ancestor = Some(node);
            let mut connected = false;
            let mut nearest_form = None;
            let mut disabled = view.attr(node, "disabled").is_some();
            let mut depth = 0;
            while let Some(index) = ancestor {
                self.state.work(1)?;
                depth += 1;
                if depth > 128 || index >= parents.len() { return Err("Select projected ancestry limit reached.".into()); }
                let name = view.tag(index);
                if index != node && (matches!(name, "head" | "script" | "style" | "template" | "iframe" | "object" | "datalist")
                    || name == "noscript" && self.doc.scripting_enabled) {
                    return Err("Native select connection in an inert context is not implemented.".into());
                }
                if name == "form" && nearest_form.is_none() { nearest_form = Some(index); }
                if name == "fieldset" && view.attr(index, "disabled").is_some() {
                    // Fragment fieldsets/legends remain refused. Their existing
                    // first-legend membership is unaffected by whole-select moves.
                    let legend = self.doc.nodes.get(index).and_then(|item| item.children.iter().copied().find(|&child| tag(self.doc, child) == "legend"));
                    let mut current = Some(node);
                    let mut inside_legend = false;
                    for _ in 0..128 {
                        if current == legend && legend.is_some() { inside_legend = true; break; }
                        if current == Some(index) || current.is_none() { break; }
                        self.state.work(1)?;
                        current = parents[current.unwrap()];
                    }
                    disabled |= !inside_legend;
                }
                if index == 0 { connected = true; break; }
                ancestor = parents[index];
            }
            let owner = if let Some(id) = view.attr(node, "form") {
                if connected { self.state.tree.ids.get(id).copied().filter(|&owner| tag(self.doc, owner) == "form") } else { None }
            } else { nearest_form };
            plan.metadata.push((node, owner, disabled));
        }
        Ok(plan)
    }
}

#[cfg(test)]
#[path = "script_select_fragment_tests.rs"]
mod tests;
