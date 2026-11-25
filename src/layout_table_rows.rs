//! Anonymous rows for consecutive table-cell children of a table or row group.
//! Ordinary children that would need anonymous cells retain the older fallback.
//! https://www.w3.org/TR/CSS22/tables.html#anonymous-boxes
use super::*;

const MAX_TABLE_VISITS: usize = 8_192;
const MAX_TABLE_ROWS: usize = 2_048;
const MAX_TABLE_CELLS: usize = 2_048;
const MAX_TABLE_COLUMNS: usize = 128;

struct Frame {
    parent: usize,
    next: usize,
    cells: Vec<usize>,
}

fn flush(rows: &mut Vec<TableRow>, frame: &mut Frame) {
    if !frame.cells.is_empty() && rows.len() < MAX_TABLE_ROWS {
        rows.push(TableRow {
            node: None,
            parent: frame.parent,
            cells: std::mem::take(&mut frame.cells),
        });
    }
}

impl Engine<'_> {
    fn table_ignores_child(&self, node: usize) -> bool {
        match &self.doc.nodes[node].kind {
            NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. }
            | NodeKind::DocumentFragment | NodeKind::DocumentType(_) => true,
            NodeKind::Text(text) | NodeKind::Cdata(text) => text.as_str().bytes()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)),
            NodeKind::Element { tag, .. } => {
                self.styles[node].display == Display::None
                    || forms::kind(self.doc, node) == Some(Kind::Hidden)
                    || matches!(tag.as_str(), "head" | "title" | "script" | "style" | "link"
                        | "meta" | "base" | "template" | "iframe" | "object" | "datalist")
                    || self.doc.scripting_enabled && tag == "noscript"
            }
        }
    }

    fn table_supports_anonymous_rows(&self, root: usize) -> bool {
        // Decide for the whole table before admitting a new row: otherwise a
        // repaired row group could disable fallback and discard text elsewhere.
        // Frames retain only the current path, not every unvisited sibling.
        let mut pending = vec![(root, 0usize)];
        let mut visits = 0;
        while let Some((parent, next)) = pending.last_mut() {
            let Some(&node) = self.doc.nodes[*parent].children.get(*next) else {
                pending.pop();
                continue;
            };
            *next += 1;
            let row_parent = *parent;
            if visits == MAX_TABLE_VISITS {
                return false;
            }
            visits += 1;
            if self.table_ignores_child(node) {
                continue;
            }
            // Positioned children require anonymous inline/cell handling, not
            // a new in-flow column. Hidden/transparent boxes still take space.
            if matches!(self.styles[node].position, Position::Absolute | Position::Fixed) {
                return false;
            }
            match self.styles[node].display {
                Display::TableRow | Display::TableCell => {}
                Display::TableRowGroup if row_parent == root && pending.len() < MAX_DEPTH => {
                    pending.push((node, 0));
                }
                _ => return false,
            }
        }
        true
    }

    pub(super) fn table_rows(&self, root: usize) -> Vec<TableRow> {
        let anonymous = self.table_supports_anonymous_rows(root);
        let mut pending = vec![Frame { parent: root, next: 0, cells: Vec::new() }];
        let mut rows = Vec::new();
        let (mut visits, mut cells) = (0usize, 0usize);
        while !pending.is_empty()
            && visits < MAX_TABLE_VISITS
            && rows.len() < MAX_TABLE_ROWS
            && cells < MAX_TABLE_CELLS
        {
            let index = pending.len() - 1;
            let parent = pending[index].parent;
            let Some(&node) = self.doc.nodes[parent].children.get(pending[index].next) else {
                let mut frame = pending.pop().unwrap();
                flush(&mut rows, &mut frame);
                continue;
            };
            pending[index].next += 1;
            visits += 1;
            if self.table_ignores_child(node) {
                continue;
            }
            match self.styles[node].display {
                Display::TableRow => {
                    flush(&mut rows, &mut pending[index]);
                    if rows.len() == MAX_TABLE_ROWS { break; }
                    let row: Vec<_> = self.doc.nodes[node].children.iter().copied()
                        .filter(|&cell| self.styles[cell].display == Display::TableCell)
                        .take(MAX_TABLE_COLUMNS.min(MAX_TABLE_CELLS - cells))
                        .collect();
                    cells += row.len();
                    rows.push(TableRow { node: Some(node), parent, cells: row });
                }
                Display::TableRowGroup => {
                    flush(&mut rows, &mut pending[index]);
                    if pending.len() < MAX_DEPTH {
                        pending.push(Frame { parent: node, next: 0, cells: Vec::new() });
                    }
                }
                Display::TableCell if anonymous => {
                    if pending[index].cells.len() < MAX_TABLE_COLUMNS {
                        pending[index].cells.push(node);
                        cells += 1;
                    }
                }
                _ => {}
            }
        }
        // An admitted prefix may end at the cell/visit limit, with its final
        // anonymous row still pending. Ancestors were flushed before descent.
        if let Some(frame) = pending.last_mut() {
            flush(&mut rows, frame);
        }
        rows
    }
}
