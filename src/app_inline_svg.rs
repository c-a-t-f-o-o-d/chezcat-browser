//! Connected passive SVG uses the original, non-networking SVG decoder.
//! Snapshots are rebuilt from current DOM data on reflow, never from a loaded
//! resource or a detached node. Failed replacements discard the previous paint.
use super::*;
use crate::html::{Namespace, NodeKind};

fn append(out: &mut String, text: &str) -> Result<(), String> {
    if out
        .len()
        .checked_add(text.len())
        .is_none_or(|n| n > crate::image::MAX_INPUT_BYTES)
    {
        return Err("Inline SVG exceeds the image source allowance.".into());
    }
    out.try_reserve(text.len())
        .map_err(|_| "Inline SVG source reservation failed.")?;
    out.push_str(text);
    Ok(())
}
fn escaped(out: &mut String, text: &str) -> Result<(), String> {
    for c in text.chars() {
        let mut bytes = [0; 4];
        append(
            out,
            match c {
                '&' => "&amp;",
                '<' => "&lt;",
                '>' => "&gt;",
                '"' => "&quot;",
                _ => c.encode_utf8(&mut bytes),
            },
        )?;
    }
    Ok(())
}
fn source(doc: &Document, root: usize) -> Result<String, String> {
    let mut out = String::new();
    let mut stack = Vec::new();
    stack
        .try_reserve(1)
        .map_err(|_| "Inline SVG traversal reservation failed.")?;
    stack.push((root, false));
    let mut visited = 0usize;
    while let Some((id, end)) = stack.pop() {
        let node = doc
            .nodes
            .get(id)
            .ok_or("Inline SVG child is unavailable.")?;
        if end {
            let NodeKind::Element { tag, .. } = &node.kind else {
                unreachable!()
            };
            append(&mut out, "</")?;
            append(&mut out, tag)?;
            append(&mut out, ">")?;
            continue;
        }
        visited = visited
            .checked_add(1)
            .filter(|&n| n <= doc.nodes.len())
            .ok_or("Inline SVG topology is cyclic or duplicated.")?;
        match &node.kind {
            NodeKind::Element { tag, attrs, .. } => {
                if !crate::html::svg_dom_transfer(node) {
                    return Err(
                        "Inline SVG requires supported passive names and local references.".into(),
                    );
                }
                append(&mut out, "<")?;
                append(&mut out, tag)?;
                for attr in attrs {
                    append(&mut out, " ")?;
                    append(&mut out, &attr.name.qualified)?;
                    append(&mut out, "=\"")?;
                    escaped(&mut out, &attr.value)?;
                    append(&mut out, "\"")?;
                }
                append(&mut out, ">")?;
                stack
                    .try_reserve(node.children.len().saturating_add(1))
                    .map_err(|_| "Inline SVG traversal reservation failed.")?;
                stack.push((id, true));
                stack.extend(node.children.iter().rev().map(|&child| (child, false)));
            }
            NodeKind::Text(text)
                if text
                    .chars()
                    .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')) =>
            {
                escaped(&mut out, text)?
            }
            NodeKind::Comment(_) => {}
            _ => return Err("Inline SVG contains unsupported content.".into()),
        }
    }
    Ok(out)
}

impl App {
    pub(super) fn sync_inline_svg(&mut self) {
        let mut previous = std::mem::take(&mut self.inline_svg_sources);
        let mut stack = Vec::new();
        if stack.try_reserve(1).is_err() {
            for id in previous.keys() {
                self.images.remove(id);
            }
            self.status = "Inline SVG traversal reservation failed.".into();
            return;
        }
        stack.push(0);
        let mut visited = 0usize;
        let started = std::time::Instant::now();
        while let Some(id) = stack.pop() {
            visited += 1;
            if visited > self.document.nodes.len() {
                break;
            }
            let Some(node) = self.document.nodes.get(id) else {
                break;
            };
            if node.namespace == Namespace::Svg
                && matches!(&node.kind, NodeKind::Element { tag, .. } if tag=="svg")
            {
                let candidate = source(&self.document, id);
                let old = previous.remove(&id);
                match candidate {
                    Ok(source) => {
                        let source_bytes = self
                            .inline_svg_sources
                            .values()
                            .try_fold(source.capacity(), |n, s| n.checked_add(s.capacity()));
                        if source_bytes.is_none_or(|n| n > crate::image::MAX_INPUT_BYTES)
                            || self.inline_svg_sources.try_reserve(1).is_err()
                        {
                            self.images.remove(&id);
                            self.status =
                                "Inline SVG source cache allowance or reservation exhausted."
                                    .into();
                            continue;
                        }
                        let unchanged =
                            old.as_deref() == Some(&source) && self.images.contains_key(&id);
                        if !unchanged {
                            self.images.remove(&id);
                            let bitmap = crate::image::decode_checked(source.as_bytes(), &|| {
                                if started.elapsed() > std::time::Duration::from_secs(45) {
                                    Err("Inline SVG image work budget exhausted.".into())
                                } else {
                                    Ok(())
                                }
                            });
                            match bitmap {
                                Ok(bitmap) => {
                                    let used = self.images.values().try_fold(0usize, |n, image| {
                                        image
                                            .pixels
                                            .capacity()
                                            .checked_mul(std::mem::size_of::<u32>())
                                            .and_then(|bytes| n.checked_add(bytes))
                                    });
                                    let bytes = bitmap
                                        .pixels
                                        .capacity()
                                        .checked_mul(std::mem::size_of::<u32>());
                                    if used
                                        .zip(bytes)
                                        .and_then(|(used, bytes)| used.checked_add(bytes))
                                        .is_some_and(|total| {
                                            total <= loader::MAX_DECODED_IMAGE_BYTES
                                        })
                                        && self.images.try_reserve(1).is_ok()
                                    {
                                        self.images.insert(id, std::sync::Arc::new(bitmap));
                                    } else {
                                        self.status = "Inline SVG decoded image allowance or reservation exhausted.".into();
                                    }
                                }
                                Err(error) => {
                                    self.status = format!("Inline SVG was not painted: {error}");
                                }
                            }
                        }
                        self.inline_svg_sources.insert(id, source);
                    }
                    Err(error) => {
                        self.images.remove(&id);
                        self.status = format!("Inline SVG was not painted: {error}");
                    }
                }
                continue;
            }
            // Template descendants have their own inert tree, never a viewport.
            if matches!(&node.kind, NodeKind::Element { tag, .. } if tag=="template") {
                continue;
            }
            if stack.try_reserve(node.children.len()).is_err() {
                break;
            }
            stack.extend(node.children.iter().rev().copied());
        }
        for id in previous.keys() {
            self.images.remove(id);
        }
    }
}
