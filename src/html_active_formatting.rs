//! Active formatting entries retain the original parser element and its
//! current stack identity. Markers isolate cells, captions and templates.
use super::*;
#[path = "html_formatting_adoption.rs"]
mod adoption;
use adoption::Topology;

#[cfg(test)]
#[path = "html_active_formatting_tests.rs"]
mod tests;

struct Entry {
    node: usize,
    depth: usize,
}
#[derive(Default)]
pub(super) struct Formatting {
    entries: Vec<Entry>,
    markers: Vec<usize>,
    copied: usize,
    pub(super) topology: Topology,
}

fn element(doc: &Document, node: usize) -> (&str, &[Attribute]) {
    let NodeKind::Element { tag, attrs, .. } = &doc.nodes[node].kind else {
        unreachable!("open parser element")
    };
    (tag, attrs)
}

pub(super) fn reconstructs_on_start(tag: &str) -> bool {
    !matches!(
        tag,
        "html"
            | "head"
            | "body"
            | "frameset"
            | "frame"
            | "base"
            | "basefont"
            | "bgsound"
            | "link"
            | "meta"
            | "noframes"
            | "script"
            | "style"
            | "title"
            | "template"
            | "address"
            | "article"
            | "aside"
            | "blockquote"
            | "center"
            | "details"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "header"
            | "hgroup"
            | "main"
            | "menu"
            | "nav"
            | "ol"
            | "p"
            | "search"
            | "section"
            | "summary"
            | "ul"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "pre"
            | "listing"
            | "form"
            | "li"
            | "dd"
            | "dt"
            | "plaintext"
            | "table"
            | "hr"
            | "textarea"
            | "iframe"
            | "noembed"
            | "rb"
            | "rp"
            | "rt"
            | "rtc"
            | "caption"
            | "col"
            | "colgroup"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
    )
}

pub(super) fn scope_boundary(doc: &Document, node: usize) -> bool {
    doc.nodes[node].namespace != Namespace::Html
        || matches!(
            element(doc, node).0,
            "applet"
                | "caption"
                | "html"
                | "table"
                | "td"
                | "th"
                | "marquee"
                | "object"
                | "select"
                | "template"
        )
}

pub(super) fn special_html(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "applet"
            | "area"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "bgsound"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "iframe"
            | "img"
            | "input"
            | "keygen"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "marquee"
            | "menu"
            | "meta"
            | "nav"
            | "noembed"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "p"
            | "param"
            | "plaintext"
            | "pre"
            | "script"
            | "search"
            | "section"
            | "select"
            | "source"
            | "style"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
            | "wbr"
            | "xmp"
    )
}

fn copy_string(s: &str) -> Result<String, String> {
    let mut value = String::new();
    value
        .try_reserve_exact(s.len())
        .map_err(|_| "Cannot reserve reconstructed formatting string.")?;
    value.push_str(s);
    Ok(value)
}

fn clone_formatting(
    copied: &mut usize,
    doc: &mut Document,
    source: usize,
    limit: usize,
) -> Result<usize, String> {
    if doc.nodes.len() >= limit {
        return Err(format!("HTML document node limit reached ({limit} nodes)."));
    }
    let NodeKind::Element {
        tag,
        attrs,
        id,
        href,
        ..
    } = &doc.nodes[source].kind
    else {
        unreachable!()
    };
    // Reopening can duplicate one large attribute many times from a
    // small input. Charge newly retained storage before each clone.
    let bytes = tag.len()
        + id.as_ref().map_or(0, String::len)
        + href.as_ref().map_or(0, String::len)
        + std::mem::size_of_val(attrs.as_slice())
        + attrs.iter().map(Attribute::bytes).sum::<usize>();
    *copied = copied
        .checked_add(bytes)
        .filter(|n| *n <= crate::javascript::limits::BROWSER.host.retained_bytes)
        .ok_or("HTML formatting reconstruction storage limit reached.")?;
    let mut new_attrs = Vec::new();
    new_attrs
        .try_reserve_exact(attrs.len())
        .map_err(|_| "Cannot reserve reconstructed attributes.")?;
    for a in attrs {
        new_attrs.push(Attribute {
            name: a.name.copy()?,
            value: a.value.try_clone()?,
        });
    }
    let kind = NodeKind::Element {
        tag: copy_string(tag)?,
        expanded_name: Vec::new(),
        attrs: new_attrs,
        id: id.as_deref().map(copy_string).transpose()?,
        href: href.as_deref().map(copy_string).transpose()?,
    };
    let node = doc.nodes.len();
    doc.nodes
        .try_reserve(1)
        .map_err(|_| "Cannot reserve reconstructed element.")?;
    doc.nodes.push(Node {
        kind,
        namespace: Namespace::Html,
        children: Vec::new(),
    });
    Ok(node)
}

impl Formatting {
    // A table can pop a fostered element without executing that element's
    // end-tag rule. Its marker stays until an insertion mode clears it.
    pub(super) fn clear_to_marker(&mut self) {
        self.entries.truncate(self.markers.pop().unwrap_or(0));
    }

    fn start(&self) -> usize {
        self.markers.last().copied().unwrap_or(0)
    }

    pub(super) fn opened(
        &mut self,
        doc: &Document,
        stack: &[usize],
        node: usize,
    ) -> Result<(), String> {
        let (tag, attrs) = element(doc, node);
        if matches!(
            tag,
            "caption" | "td" | "th" | "template" | "applet" | "marquee" | "object"
        ) {
            self.markers
                .try_reserve(1)
                .map_err(|_| "Cannot reserve formatting marker.")?;
            self.markers.push(self.entries.len());
        } else if fragment_formatting(tag) {
            let start = self.start();
            // The three-entry limit compares the complete unordered attribute
            // set. Parser-created HTML attributes here are scalar, unqualified
            // and already de-duplicated by the tokenizer.
            let mut matching = 0;
            let mut first = 0;
            let mut lookup = None;
            for (i, e) in self.entries[start..].iter().enumerate() {
                let (other, properties) = element(doc, e.node);
                if other != tag || properties.len() != attrs.len() {
                    continue;
                }
                if !attrs.is_empty() {
                    if lookup.is_none() {
                        let mut map = std::collections::HashMap::new();
                        map.try_reserve(attrs.len())
                            .map_err(|_| "Cannot index formatting attributes.")?;
                        map.extend(
                            attrs
                                .iter()
                                .map(|a| (a.name.qualified.as_str(), a.value.as_str())),
                        );
                        lookup = Some(map);
                    }
                    let map = lookup.as_ref().unwrap();
                    if !properties.iter().all(|a| {
                        map.get(a.name.qualified.as_str()).copied() == Some(a.value.as_str())
                    }) {
                        continue;
                    }
                }
                if matching == 0 {
                    first = start + i;
                }
                matching += 1;
                if matching == 3 {
                    break;
                }
            }
            if matching == 3 {
                self.entries.remove(first);
            }
            self.entries
                .try_reserve(1)
                .map_err(|_| "Cannot reserve active formatting entry.")?;
            self.entries.push(Entry {
                node,
                depth: stack.len() - 1,
            });
        }
        Ok(())
    }

    pub(super) fn end(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        tag: &str,
        tables: &mut table_construction::Tables,
        limits: LoadedTree<'_>,
    ) -> Result<bool, String> {
        for _ in 0..8 {
            let start = self.start();
            let Some(offset) = self.entries[start..]
                .iter()
                .rposition(|e| element(doc, e.node).0 == tag)
            else {
                for &n in stack.iter().rev() {
                    let name = element(doc, n).0;
                    if name == tag {
                        return Ok(false);
                    }
                    if doc.nodes[n].namespace != Namespace::Html || special_html(name) {
                        break;
                    }
                }
                return Ok(true);
            };
            let index = start + offset;
            let entry = &self.entries[index];
            if stack.get(entry.depth) != Some(&entry.node) {
                self.entries.remove(index);
                return Ok(true);
            }
            if stack[entry.depth + 1..]
                .iter()
                .any(|&n| scope_boundary(doc, n))
            {
                return Ok(true);
            }
            let depth = entry.depth;
            if let Some(relative) = stack[depth + 1..]
                .iter()
                .position(|&n| special_html(element(doc, n).0))
            {
                self.adopt_block(doc, stack, index, depth + 1 + relative, tables, limits)?;
                continue;
            }
            self.entries.remove(index);
            stack.truncate(depth);
            return Ok(true);
        }
        Ok(true)
    }

    // A nested nobr uses the open-element scope, which can still contain
    // an older nobr even when a formatting marker hides its list entry.
    pub(super) fn close_nested_nobr(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        tables: &mut table_construction::Tables,
        limits: LoadedTree<'_>,
    ) -> Result<bool, String> {
        for depth in (1..stack.len()).rev() {
            let node = stack[depth];
            let name = element(doc, node).0;
            if scope_boundary(doc, node) {
                break;
            }
            if name == "nobr" {
                if !self.end(doc, stack, "nobr", tables, limits)? {
                    stack.truncate(depth);
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn reconstruct(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        limit: usize,
        mut foster: Option<&mut table_construction::Tables>,
    ) -> Result<bool, String> {
        if self.entries.is_empty() {
            return Ok(false);
        }
        let start = self.start();
        let mut first = self.entries.len();
        while first > start {
            let e = &self.entries[first - 1];
            if stack.get(e.depth) == Some(&e.node) {
                break;
            }
            first -= 1;
        }
        if first == self.entries.len()
            || *stack.last().unwrap() != 0
                && doc.nodes[*stack.last().unwrap()].namespace != Namespace::Html
        {
            return Ok(false);
        }
        for entry in &mut self.entries[first..] {
            if doc.nodes.len() >= limit {
                return Err(format!("HTML document node limit reached ({limit} nodes)."));
            }
            if stack.len() >= MAX_DEPTH {
                return Err(format!(
                    "HTML document nesting limit reached ({MAX_DEPTH} open nodes)."
                ));
            }
            let node = clone_formatting(&mut self.copied, doc, entry.node, limit)?;
            let parent = *stack.last().unwrap();
            stack
                .try_reserve(1)
                .map_err(|_| "Cannot reserve reconstructed stack entry.")?;
            let parent = if let Some(table) = foster.take() {
                let physical = table.foster_location(doc)?.0;
                table.link_before_table(doc, node)?;
                physical
            } else {
                doc.nodes[parent]
                    .children
                    .try_reserve(1)
                    .map_err(|_| "Cannot reserve reconstructed element edge.")?;
                doc.nodes[parent].children.push(node);
                parent
            };
            self.topology.linked(node, parent)?;
            *entry = Entry {
                node,
                depth: stack.len(),
            };
            stack.push(node);
        }
        Ok(true)
    }
}
