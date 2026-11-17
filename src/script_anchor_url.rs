//! Original bounded hyperlink URL reflection. Reading or assigning a URL never
//! admits a navigation, a resource request, or a file capability.
use super::*;

fn property(name: &str) -> bool {
    matches!(name, "href" | "origin" | "protocol" | "username" | "password"
        | "host" | "hostname" | "port" | "pathname" | "search" | "hash")
}

fn local_reference(raw: &str) -> bool {
    let raw = raw.trim_matches(|c| c <= '\u{20}');
    let Some((scheme, _)) = raw.split_once(':') else { return true; };
    let has_scheme = scheme.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && scheme.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'+'|b'-'|b'.'));
    !has_scheme || scheme.eq_ignore_ascii_case("file")
}

impl Bridge<'_> {
    pub(super) fn anchor_url_property(&self, node: usize, name: &str) -> bool {
        property(name) && self.doc.nodes.get(node).is_some_and(|entry|
            entry.namespace == crate::html::Namespace::Html && tag(self.doc, node) == "a")
    }

    // Charge all parser/serializer temporaries before making the first copy.
    // The helper is a pure URL value API, never the navigation admission API.
    fn anchor_url_inputs(&mut self, node: usize, extra: usize) -> Result<(String, Option<String>), String> {
        let raw = self.doc.attr(node, "href").unwrap_or("");
        if raw.len().saturating_add(extra) > self.state.limits.host.string_bytes {
            return Err("Anchor URL exceeds the configured string allowance.".into());
        }
        let base = self.state.dynamic.base.as_ref()
            .or_else(|| self.state.location.as_ref().map(|location| &location.source));
        let cost = dom_compat::url_copy_cost(raw, base)?.checked_add(extra.saturating_mul(20))
            .ok_or("Anchor URL copy accounting overflow.")?;
        self.state.work(cost)?;
        self.state.copies(cost)?;
        let base = self.state.dynamic.base.as_ref()
            .or_else(|| self.state.location.as_ref().map(|location| &location.source))
            .map(crate::navigation::Location::display);
        Ok((self.doc.attr(node, "href").unwrap_or("").to_owned(), base))
    }

    pub(super) fn anchor_url_get(&mut self, node: usize, name: &str) -> Result<Value, String> {
        if self.doc.attr(node, "href").is_none() {
            return self.string(if name == "protocol" { ":" } else { "" });
        }
        // Reborrow live attributes after charging rather than keeping a stale
        // parsed record across an Attr mutation or an effective-base change.
        let (raw, base) = self.anchor_url_inputs(node, 0)?;
        if name == "href" && base.as_ref().is_some_and(|base| base.starts_with("file:"))
            && local_reference(&raw) {
            return self.reflected_url(node, "href");
        }
        let result = crate::javascript::anchor_url_project(&raw, base.as_deref(), name)?;
        match result {
            Some(text) => self.string(&text),
            None => self.string(if name == "href" { &raw } else if name == "protocol" { ":" } else { "" }),
        }
    }

    pub(super) fn anchor_url_set(&mut self, node: usize, name: &str, value: &Value) -> Result<(), String> {
        // The evaluator performs Web IDL string coercion before this call.
        // Its observable side effects therefore precede the live base/Attr read.
        let text = self.text(value)?;
        if name == "href" {
            return self.attribute(node, "href", Some(text));
        }
        if self.doc.attr(node, "href").is_none() { return Ok(()); }
        let (raw, base) = self.anchor_url_inputs(node, text.len())?;
        if let Some(updated) = crate::javascript::anchor_url_mutate(&raw, base.as_deref(), name, &text)? {
            self.attribute(node, "href", Some(updated))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "script_anchor_url_tests.rs"]
mod tests;
