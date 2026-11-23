//! Table insertion contexts for complete HTML documents. Character tokens
//! stay pending until the next non-character token determines their placement.
use super::*;

#[cfg(test)]
#[path = "html_table_construction_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
struct OpenTable {
    node: usize,
    parent: usize,
    slot: usize,
    depth: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Table,
    Section,
    Row,
    Cell,
    Caption,
    Columns,
}
pub(super) enum Action {
    Normal,
    Consumed,
    Foster,
}
#[derive(Default)]
pub(super) struct Tables {
    open: Vec<OpenTable>,
    pending: String,
    pending_parent: Option<usize>,
    nonspace: bool,
}

fn name(doc: &Document, node: usize) -> &str {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag,
        _ => "",
    }
}
fn section(s: &str) -> bool {
    matches!(s, "tbody" | "thead" | "tfoot")
}
fn part(s: &str) -> bool {
    section(s) || matches!(s, "caption" | "col" | "colgroup" | "td" | "th" | "tr")
}
fn space(b: u8) -> bool {
    matches!(b, 9 | 10 | 12 | 13 | 32)
}
fn table_parent(s: &str) -> bool {
    section(s) || matches!(s, "table" | "tr")
}
fn limit_error(n: usize) -> String {
    format!("HTML document node limit reached ({n} nodes).")
}

impl Tables {
    pub(super) fn opened(
        &mut self,
        node: usize,
        parent: usize,
        slot: usize,
        depth: usize,
    ) -> Result<(), String> {
        self.open
            .try_reserve(1)
            .map_err(|_| "Cannot reserve HTML table stack.")?;
        self.open.push(OpenTable {
            node,
            parent,
            slot,
            depth,
        });
        Ok(())
    }
    fn active(&mut self, stack: &[usize]) -> Option<OpenTable> {
        while self
            .open
            .last()
            .is_some_and(|t| stack.get(t.depth) != Some(&t.node))
        {
            self.open.pop();
        }
        self.open.last().copied()
    }
    pub(super) fn has_active(&mut self, stack: &[usize]) -> bool {
        self.active(stack).is_some()
    }
    pub(super) fn blocks_end(&mut self, doc: &Document, stack: &[usize], target: usize) -> bool {
        let Some(table) = self.active(stack) else {
            return false;
        };
        // Closing the nearest open HTML template unwinds its entire contents,
        // including an unfinished table, row or cell inside that template.
        if doc.nodes[stack[target]].namespace == Namespace::Html
            && name(doc, stack[target]) == "template"
        {
            return false;
        }
        target < table.depth
            || stack[target + 1..].iter().any(|&n| {
                doc.nodes[n].namespace != Namespace::Html
                    || matches!(
                        name(doc, n),
                        "template" | "td" | "th" | "caption" | "object" | "noscript" | "iframe"
                    )
            })
    }
    fn mode(doc: &Document, stack: &[usize], table: OpenTable) -> Option<(Mode, usize)> {
        for i in (table.depth + 1..stack.len()).rev() {
            let n = stack[i];
            if doc.nodes[n].namespace != Namespace::Html {
                return None;
            }
            match name(doc, n) {
                "template" | "noscript" | "object" | "iframe" => return None,
                "td" | "th" => return Some((Mode::Cell, i)),
                "tr" => return Some((Mode::Row, i)),
                "tbody" | "tfoot" | "thead" => return Some((Mode::Section, i)),
                "caption" => return Some((Mode::Caption, i)),
                "colgroup" => return Some((Mode::Columns, i)),
                _ => {}
            }
        }
        Some((Mode::Table, table.depth))
    }
    fn implied(
        doc: &mut Document,
        stack: &mut Vec<usize>,
        tag: &str,
        limit: usize,
    ) -> Result<(), String> {
        if doc.nodes.len() >= limit {
            return Err(limit_error(limit));
        }
        if stack.len() >= MAX_DEPTH {
            return Err(format!(
                "HTML document nesting limit reached ({MAX_DEPTH} open nodes)."
            ));
        }
        let parent = *stack.last().unwrap();
        let node = doc.nodes.len();
        doc.nodes
            .try_reserve(1)
            .map_err(|_| "Cannot reserve implied table element.")?;
        doc.nodes[parent]
            .children
            .try_reserve(1)
            .map_err(|_| "Cannot reserve table child.")?;
        stack
            .try_reserve(1)
            .map_err(|_| "Cannot reserve table insertion stack.")?;
        doc.nodes.push(Node {
            namespace: Namespace::Html,
            kind: NodeKind::Element {
                expanded_name: Vec::new(),
                tag: tag.into(),
                id: None,
                href: None,
                attrs: Vec::new(),
            },
            children: Vec::new(),
        });
        doc.nodes[parent].children.push(node);
        stack.push(node);
        Ok(())
    }
    pub(super) fn tag(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        tag: &Tag,
        limit: usize,
    ) -> Result<Action, String> {
        let n = tag.name.as_str();
        let end = tag.closing;
        loop {
            let Some(table) = self.active(stack) else {
                if part(n)
                    && !stack.iter().skip(1).any(|&i| {
                        doc.nodes[i].namespace != Namespace::Html || name(doc, i) == "template"
                    })
                {
                    return Ok(Action::Consumed);
                }
                return Ok(Action::Normal);
            };
            let Some((mode, depth)) = Self::mode(doc, stack, table) else {
                return Ok(Action::Normal);
            };
            let current = name(doc, stack[depth]);
            if end && matches!(n, "body" | "html") {
                return Ok(Action::Consumed);
            }
            match mode {
                Mode::Cell => {
                    if end && matches!(n, "td" | "th") {
                        if n == current {
                            stack.truncate(depth);
                        }
                        return Ok(Action::Consumed);
                    }
                    let close = !end && part(n)
                        || end
                            && matches!(n, "table" | "tr" | "tbody" | "tfoot" | "thead")
                            && stack[table.depth..depth].iter().any(|&i| name(doc, i) == n);
                    if close {
                        stack.truncate(depth);
                        continue;
                    }
                    return Ok(if end && part(n) {
                        Action::Consumed
                    } else {
                        Action::Normal
                    });
                }
                Mode::Caption => {
                    if end && n == "caption" {
                        stack.truncate(depth);
                        return Ok(Action::Consumed);
                    }
                    if !end && part(n) || end && n == "table" {
                        stack.truncate(depth);
                        continue;
                    }
                    return Ok(if end && part(n) {
                        Action::Consumed
                    } else {
                        Action::Normal
                    });
                }
                Mode::Columns => {
                    if !end && n == "col" || n == "template" {
                        return Ok(Action::Normal);
                    }
                    if end && n == "col" {
                        return Ok(Action::Consumed);
                    }
                    stack.truncate(depth);
                    if end && n == "colgroup" {
                        return Ok(Action::Consumed);
                    }
                    continue;
                }
                Mode::Row => {
                    if !end && matches!(n, "td" | "th") {
                        stack.truncate(depth + 1);
                        return Ok(Action::Normal);
                    }
                    if end && n == "tr" {
                        stack.truncate(depth);
                        return Ok(Action::Consumed);
                    }
                    if !end && part(n) || end && n == "table" {
                        stack.truncate(depth);
                        continue;
                    }
                    if end && section(n) {
                        if stack[table.depth..depth].iter().any(|&i| name(doc, i) == n) {
                            stack.truncate(depth);
                            continue;
                        }
                        return Ok(Action::Consumed);
                    }
                }
                Mode::Section => {
                    if !end && n == "tr" {
                        stack.truncate(depth + 1);
                        return Ok(Action::Normal);
                    }
                    if !end && matches!(n, "td" | "th") {
                        stack.truncate(depth + 1);
                        Self::implied(doc, stack, "tr", limit)?;
                        continue;
                    }
                    if end && section(n) {
                        if n == current {
                            stack.truncate(depth);
                        }
                        return Ok(Action::Consumed);
                    }
                    if !end && part(n) || end && n == "table" {
                        stack.truncate(depth);
                        continue;
                    }
                }
                Mode::Table => {}
            }
            // The row and section modes delegate the remaining tokens to the
            // table rules. These may deliberately reprocess the same token.
            if !end {
                if n == "caption" || n == "colgroup" || section(n) {
                    stack.truncate(table.depth + 1);
                    return Ok(Action::Normal);
                }
                if n == "col" {
                    stack.truncate(table.depth + 1);
                    Self::implied(doc, stack, "colgroup", limit)?;
                    continue;
                }
                if matches!(n, "tr" | "td" | "th") {
                    stack.truncate(table.depth + 1);
                    Self::implied(doc, stack, "tbody", limit)?;
                    continue;
                }
                if n == "table" {
                    stack.truncate(table.depth);
                    continue;
                }
                if matches!(n, "script" | "style" | "template" | "html" | "form")
                    || n == "input"
                        && tag
                            .attrs
                            .iter()
                            .any(|(k, v)| k == "type" && v.eq_ignore_ascii_case("hidden"))
                {
                    return Ok(Action::Normal);
                }
                if table_parent(name(doc, *stack.last().unwrap())) {
                    return Ok(Action::Foster);
                }
            } else {
                if n == "table" {
                    stack.truncate(table.depth);
                    return Ok(Action::Consumed);
                }
                if part(n) {
                    return Ok(Action::Consumed);
                }
            }
            return Ok(Action::Normal);
        }
    }

    pub(super) fn characters(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        mut text: String,
        limit: usize,
    ) -> Result<Option<String>, String> {
        let Some(table) = self.active(stack) else {
            return Ok(Some(text));
        };
        let Some((mode, depth)) = Self::mode(doc, stack, table) else {
            return Ok(Some(text));
        };
        if mode == Mode::Columns {
            let prefix = text.bytes().take_while(|b| space(*b)).count();
            if prefix == text.len() {
                if !append_text_value(doc, stack[depth], text, limit)? {
                    return Err(limit_error(limit));
                }
                return Ok(None);
            }
            if prefix != 0 {
                let mut whitespace = String::new();
                whitespace
                    .try_reserve_exact(prefix)
                    .map_err(|_| "Cannot reserve table whitespace.")?;
                whitespace.push_str(&text[..prefix]);
                if !append_text_value(doc, stack[depth], whitespace, limit)? {
                    return Err(limit_error(limit));
                }
                text.drain(..prefix);
            }
            stack.truncate(depth);
        }
        let parent = *stack.last().unwrap();
        if !table_parent(name(doc, parent)) {
            return Ok(Some(text));
        }
        if self.pending_parent.is_some_and(|n| n != parent) {
            return Err("Table text parent changed before its token boundary.".into());
        }
        self.pending_parent = Some(parent);
        self.nonspace |= text.bytes().any(|b| !space(b));
        if self.pending.is_empty() {
            self.pending = text;
        } else {
            self.pending
                .try_reserve(text.len())
                .map_err(|_| "Cannot reserve pending table characters.")?;
            self.pending.push_str(&text);
        }
        Ok(None)
    }
    pub(super) fn foster_location(&mut self, doc: &Document) -> Result<(usize, usize), String> {
        let table = self
            .open
            .last_mut()
            .ok_or("No open table for foster insertion.")?;
        let children = &doc.nodes[table.parent].children;
        if children.get(table.slot) != Some(&table.node) {
            table.slot = children
                .iter()
                .position(|&n| n == table.node)
                .ok_or("Missing native table parent edge.")?;
        }
        Ok((table.parent, table.slot))
    }
    pub(super) fn link_before_table(
        &mut self,
        doc: &mut Document,
        node: usize,
    ) -> Result<(), String> {
        let (parent, slot) = self.foster_location(doc)?;
        doc.nodes[parent]
            .children
            .try_reserve(1)
            .map_err(|_| "Cannot reserve foster child edge.")?;
        doc.nodes[parent].children.insert(slot, node);
        self.open.last_mut().unwrap().slot += 1;
        Ok(())
    }
    pub(super) fn flush(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        formatting: &mut active_formatting::Formatting,
        limit: usize,
    ) -> Result<(), String> {
        let Some(parent) = self.pending_parent.take() else {
            return Ok(());
        };
        let text = std::mem::take(&mut self.pending);
        let foster = std::mem::take(&mut self.nonspace);
        if !foster {
            if !append_text_value(doc, parent, text, limit)? {
                return Err(limit_error(limit));
            }
            return Ok(());
        }
        if formatting.reconstruct(doc, stack, limit, Some(self))? {
            if !append_text_value(doc, *stack.last().unwrap(), text, limit)? {
                return Err(limit_error(limit));
            }
            return Ok(());
        }
        let (parent, slot) = self.foster_location(doc)?;
        if let Some(previous) = slot
            .checked_sub(1)
            .and_then(|at| doc.nodes[parent].children.get(at))
            .copied()
        {
            if let NodeKind::Text(value) = &mut doc.nodes[previous].kind {
                return value.append_parser_scalar(&text);
            }
        }
        if doc.nodes.len() >= limit {
            return Err(limit_error(limit));
        }
        let node = doc.nodes.len();
        doc.nodes
            .try_reserve(1)
            .map_err(|_| "Cannot reserve fostered Text node.")?;
        self.link_before_table(doc, node)?;
        doc.nodes.push(Node {
            namespace: Namespace::None,
            kind: NodeKind::Text(text.into()),
            children: Vec::new(),
        });
        Ok(())
    }
}
