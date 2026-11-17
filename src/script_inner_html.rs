//! A checked HTML fragment slice. Replacement stages every node/resource plan
//! before publishing children; old arena nodes and their native identities live
//! until document teardown. Unsupported contexts never commit a parsed prefix.
use super::*;

fn append(state: &mut State, out: &mut String, text: &str) -> Result<(), String> {
    if out.len().checked_add(text.len()).is_none_or(|len| len > state.limits.host.string_bytes) { return Err("innerHTML serialization exceeds the host string limit.".into()); }
    state.copies(text.len())?;
    out.try_reserve(text.len()).map_err(|_| "HTML serialization reservation failed.")?;
    out.push_str(text);
    Ok(())
}

fn escaped(state: &mut State, out: &mut String, text: &str, attribute: bool, raw: bool) -> Result<(), String> {
    state.work(text.len())?;
    if raw { return append(state, out, text); }
    for ch in text.chars() {
        let mut encoded = [0u8; 4];
        let text = match ch { '&' => "&amp;", '\u{a0}' => "&nbsp;", '<' => "&lt;", '>' => "&gt;", '"' if attribute => "&quot;", _ => ch.encode_utf8(&mut encoded) };
        append(state, out, text)?;
    }
    Ok(())
}

fn raw_text(name: &str, scripting: bool) -> bool {
    matches!(name, "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext") || name == "noscript" && scripting
}

impl Bridge<'_> {
    pub(super) fn inner_html(&mut self, node: usize) -> Result<Value, String> {
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("innerHTML requires known HTML namespace provenance.".into()); }
        if tag(self.doc, node) == "template" { return Err("Template content serialization is not implemented.".into()); }
        self.state.work(self.doc.nodes[node].children.len())?;
        let raw = raw_text(tag(self.doc, node), self.doc.scripting_enabled);
        let mut stack: Vec<_> = self.doc.nodes[node].children.iter().rev().map(|&child| (child, false, raw)).collect();
        let mut out = String::new();
        while let Some((node, end, raw)) = stack.pop() {
            self.state.work(1)?;
            let item = &self.doc.nodes[node];
            match &item.kind {
                NodeKind::DocumentFragment => {
                    self.state.work(item.children.len())?;
                    stack.extend(item.children.iter().rev().map(|&child| (child, false, raw)));
                }
                NodeKind::Text(text) => escaped(self.state, &mut out, text, false, raw)?,
                NodeKind::Comment(data) => {
                    self.state.work(data.len())?;
                    append(self.state, &mut out, "<!--")?;
                    append(self.state, &mut out, data)?;
                    append(self.state, &mut out, "-->")?;
                }
                NodeKind::Element { tag, attrs, .. } => {
                    if item.namespace != crate::html::Namespace::Html || tag == "template" { return Err("HTML serialization of foreign/template content is not implemented.".into()); }
                    if end { append(self.state, &mut out, "</")?; append(self.state, &mut out, tag)?; append(self.state, &mut out, ">")?; continue; }
                    append(self.state, &mut out, "<")?;
                    append(self.state, &mut out, tag)?;
                    self.state.work(attrs.len())?;
                    for attr in attrs {
                        append(self.state, &mut out, " ")?;
                        let prefix = match attr.name.namespace_uri() {
                            Some("http://www.w3.org/XML/1998/namespace") => Some("xml:"),
                            Some("http://www.w3.org/2000/xmlns/") if attr.name.local() != "xmlns" => Some("xmlns:"),
                            Some("http://www.w3.org/2000/xmlns/") => Some(""),
                            Some("http://www.w3.org/1999/xlink") => Some("xlink:"),
                            _ => None,
                        };
                        if let Some(prefix) = prefix { append(self.state, &mut out, prefix)?; append(self.state, &mut out, attr.name.local())?; }
                        else { append(self.state, &mut out, &attr.name.qualified)?; }
                        append(self.state, &mut out, "=\"")?;
                        escaped(self.state, &mut out, &attr.value, true, false)?;
                        append(self.state, &mut out, "\"")?;
                    }
                    append(self.state, &mut out, ">")?;
                    if !crate::html::is_void(tag) {
                        self.state.work(item.children.len())?;
                        stack.push((node, true, false));
                        let raw = raw_text(tag, self.doc.scripting_enabled);
                        stack.extend(item.children.iter().rev().map(|&child| (child, false, raw)));
                    }
                }
            }
        }
        Ok(Value::String(out))
    }

    pub(super) fn set_inner_html(&mut self, node: usize, source: &str) -> Result<(), String> {
        self.state.rebuild(self.doc)?;
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("innerHTML replacement requires known HTML namespace provenance.".into()); }
        if matches!(tag(self.doc, node), "script" | "style") { return Err("innerHTML replacement of script/style resource contexts is not implemented.".into()); }
        self.mutation_ancestors(node)?;
        self.state.work(source.len())?;
        self.state.copies(source.len().saturating_mul(2))?;
        if source.len() > self.state.limits.host.string_bytes { return Err("HTML fragment source exceeds the host string limit.".into()); }
        let mut staged = crate::html::parse_fragment(source, tag(self.doc, node), self.doc.scripting_enabled)?;
        let count = staged.nodes.len() - 1;
        if self.doc.nodes.len().checked_add(count).and_then(|n| n.checked_add(self.state.blank_frame_nodes)).is_none_or(|n| n > self.state.node_limit) { return Err("DOM appended-node limit reached during HTML replacement.".into()); }
        let base = self.doc.nodes.len();
        let connected = self.state.dom.connected[node];
        let mut old = Vec::new();
        for child in self.doc.nodes[node].children.clone() { old.extend(self.mutation_subtree(child)?.0); }
        if old.iter().any(|&node| matches!(tag(self.doc, node), "link" | "iframe")) { return Err("HTML replacement of stylesheet/frame resource contexts is not implemented.".into()); }
        let mut bytes = 0usize;
        let mut scripts = Vec::new();
        let mut images = Vec::new();
        let mut parents = vec![None; count];
        let mut depth = vec![0usize; staged.nodes.len()];
        let mut parent_depth = 0usize;
        let mut ancestor = Some(node);
        while let Some(parent) = ancestor { self.state.work(1)?; parent_depth += 1; ancestor = self.state.tree.parents[parent]; }
        for local in 0..staged.nodes.len() {
            self.state.work(1 + staged.nodes[local].children.len())?;
            for &child in &staged.nodes[local].children {
                depth[child] = depth[local] + 1;
                if depth[child].saturating_add(parent_depth) > 128 { return Err("DOM tree depth limit reached during HTML replacement.".into()); }
                parents[child - 1] = Some(if local == 0 { node } else { base + local - 1 });
            }
            if local == 0 { continue; }
            let item = &staged.nodes[local];
            bytes = bytes.checked_add(node_bytes(item)).ok_or("HTML fragment retained size overflow.")?;
            if let NodeKind::Element { tag: name, attrs, .. } = &item.kind {
                let href = match &item.kind { NodeKind::Element {href, ..} => href.as_ref().map_or(0, String::len), _ => 0 };
                bytes = bytes.checked_add(name.len()).and_then(|bytes| bytes.checked_add(href)).ok_or("HTML fragment retained size overflow.")?;
                if item.namespace != crate::html::Namespace::Html
                    || forms::kind(&staged, local).is_some() && name != "select" && !forms::fragment_text_input(&staged, local)
                    || matches!(name.as_str(), "form" | "fieldset" | "legend" | "style" | "link" | "base" | "meta" | "title" | "iframe" | "object" | "embed" | "source" | "audio" | "video") {
                    return Err("HTML fragment replacement of native controls/forms or unsupported resources is not implemented.".into());
                }
                self.state.work(attrs.len())?;
                if attrs.iter().any(|attr| attr.name.qualified.starts_with("on")) { return Err("HTML fragment inline event-handler registration is not implemented.".into()); }
                if let Some(style) = staged.attr(local, "style") { validate_style(style, self.state.limits.host.string_bytes)?; }
                if let Some(id) = staged.attr(local, "id").filter(|id| !id.is_empty()) {
                    self.state.work(self.forms.controls.len())?;
                    if self.forms.controls.keys().any(|&control| self.doc.attr(control, "form") == Some(id)) { return Err("HTML fragment id changes an explicit native form association.".into()); }
                }
                if name == "script" { scripts.push(base + local - 1); }
                if name == "img" {
                    if staged.attr(local, "srcset").is_some() { return Err("Dynamic responsive image selection is not implemented.".into()); }
                    if connected { images.push((base + local - 1, staged.attr(local, "src").map(str::to_owned))); }
                }
            }
        }
        self.state.copies(bytes)?;
        self.reserve_image_changes(images.len(), images.iter().filter(|(_, source)| source.is_some()).count())?;
        let mut image_plans = Vec::new();
        image_plans.try_reserve(images.len()).map_err(|_| "HTML image-plan reservation failed.")?;
        for (image, source) in images { image_plans.push((image, self.prepare_image_change(image, source.as_deref())?)); }
        self.doc.nodes.try_reserve(count).map_err(|_| "HTML node reservation failed.")?;
        self.doc.nonce_unsafe_nodes.try_reserve(staged.nonce_unsafe_nodes.len()).map_err(|_| "HTML nonce metadata reservation failed.")?;
        self.state.dom.created.try_reserve(count).map_err(|_| "HTML identity reservation failed.")?;
        self.state.dom.fragment_scripts.try_reserve(scripts.len()).map_err(|_| "Inert-script reservation failed.")?;
        self.state.detached_nodes.try_reserve(old.len().saturating_add(count)).map_err(|_| "Detached-node reservation failed.")?;
        self.state.dom.connected.try_reserve(count).map_err(|_| "HTML connection reservation failed.")?;
        self.state.tree.parents.try_reserve(count).map_err(|_| "HTML parent reservation failed.")?;
        self.state.tree.active.try_reserve(count).map_err(|_| "HTML active-node reservation failed.")?;
        let mut children = Vec::new();
        children.try_reserve(staged.nodes[0].children.len()).map_err(|_| "HTML children reservation failed.")?;
        children.extend(staged.nodes[0].children.iter().map(|&child| base + child - 1));
        let mut form_roots = Vec::new();
        form_roots.try_reserve(self.doc.nodes[node].children.len().saturating_add(children.len())).map_err(|_| "Select fragment roots reservation failed.")?;
        form_roots.extend(self.doc.nodes[node].children.iter().map(|&root| (root, None)));
        form_roots.extend(children.iter().map(|&root| (root, Some(node))));
        let form_plan = self.prepare_select_form_change(Some((&staged, base)), &form_roots)?;
        self.state.copies(form_plan.retained_bytes)?;
        let bytes = bytes.checked_add(form_plan.retained_bytes).ok_or("HTML/native select payload overflow.")?;
        self.state.replace_bytes(0, bytes)?; // Detached old storage stays retained.
        // No fallible operation follows publication of the new child list.
        for &root in &self.doc.nodes[node].children { self.state.tree.parents[root] = None; }
        for old in old { self.state.detached_nodes.insert(old); self.state.dom.connected[old] = false; self.state.tree.active[old] = false; }
        self.doc.nodes[node].children = children;
        for (offset, mut item) in staged.nodes.drain(1..).enumerate() {
            let id = base + offset;
            for child in &mut item.children { *child += base - 1; }
            self.doc.nodes.push(item);
            self.state.dom.created.insert(id);
            self.state.dom.connected.push(connected);
            self.state.tree.parents.push(parents[offset]);
            self.state.tree.active.push(false);
            if !connected { self.state.detached_nodes.insert(id); }
        }
        self.state.dom.fragment_scripts.extend(scripts);
        self.doc.nonce_unsafe_nodes.extend(staged.nonce_unsafe_nodes.into_iter().map(|local| base + local - 1));
        form_plan.commit(self.forms);
        self.commit_image_connections(image_plans);
        self.state.dom_committed();
        self.state.dirty = true;
        if connected { self.state.layout_changed(); }
        Ok(())
    }
}

#[cfg(test)]
#[path = "script_inner_html_tests.rs"]
mod tests;
