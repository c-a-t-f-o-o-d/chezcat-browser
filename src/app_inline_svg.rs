//! Connected passive SVG uses the original, non-networking SVG decoder.
//! Snapshots are rebuilt from current DOM data on reflow, never from a loaded
//! resource or a detached node. Failed replacements discard the previous paint.
use super::*;
use crate::css;
use crate::html::{Namespace, NodeKind};

pub(super) struct Source {
    text: String,
    geometry:Vec<crate::image::SvgStyle>,
    intrinsic: Option<(usize, usize)>,
    viewport: Option<(usize, usize)>,
}

impl Source {
    fn bytes(text:&String,geometry:&Vec<crate::image::SvgStyle>,seen:&mut std::collections::HashSet<usize>)->Option<usize>{
        let mut bytes=text.capacity().checked_add(std::mem::size_of::<Self>())?.checked_add(geometry.capacity().checked_mul(std::mem::size_of::<crate::image::SvgStyle>())?)?;
        for value in geometry {for (identity,size) in value.backing_records().into_iter().flatten(){if !seen.contains(&identity){seen.try_reserve(1).ok()?;seen.insert(identity);bytes=bytes.checked_add(size)?;}}}
        Some(bytes)
    }
}

pub(super) fn natural_sizes(
    images: &HashMap<usize, std::sync::Arc<Bitmap>>,
    densities: &HashMap<usize, f64>,
    sources: &HashMap<usize, Source>,
) -> HashMap<usize, (usize, usize)> {
    let mut sizes = super::natural_image_sizes(images, densities);
    for (&id, source) in sources {
        if let Some(size) = source.intrinsic {
            sizes.insert(id, size);
        }
    }
    sizes
}

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
fn source(doc: &Document, root: usize, computed:&css::ComputedProperties) -> Result<(String,Vec<crate::image::SvgStyle>), String> {
    let mut out = String::new();
    let mut geometry=Vec::new();
    let mut geometry_bytes=0usize;
    let mut backing_seen=std::collections::HashSet::new();
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
                geometry.try_reserve(1).map_err(|_|"Inline SVG computed geometry reservation failed.")?;
                let value=computed.read_svg_geometry(id).map_err(|_|"Inline SVG computed geometry is unavailable.")?.ok_or("Inline SVG computed geometry is absent.")?;
                for (identity,bytes) in value.backing_records().into_iter().flatten(){if !backing_seen.contains(&identity){backing_seen.try_reserve(1).map_err(|_|"Inline SVG backing identity reservation failed.")?;backing_seen.insert(identity);geometry_bytes=geometry_bytes.checked_add(bytes).ok_or("Inline SVG computed style backing overflow.")?;}}
                geometry.push(value.clone());
                if geometry.capacity().checked_mul(std::mem::size_of::<crate::image::SvgStyle>()).and_then(|n|n.checked_add(geometry_bytes)).and_then(|n|n.checked_add(out.capacity())).is_none_or(|bytes|bytes>crate::image::MAX_INPUT_BYTES) {return Err("Inline SVG computed geometry exceeds source cache allowance.".into());}
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
            NodeKind::Text(text) | NodeKind::Cdata(text)
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
    Ok((out,geometry))
}

impl App {
    pub(super) fn sync_inline_svg_viewports(&mut self, started: std::time::Instant) -> bool {
        let mut success=true;
        for at in 0..self.page.items.len() {
            let layout::PaintItem::Image {
                node,
                width,
                height,
                ..
            } = self.page.items[at]
            else {
                continue;
            };
            if width <= 0 || height <= 0 {
                continue;
            }
            let target = (width as usize, height as usize);
            let Some(source) = self.inline_svg_sources.get_mut(&node) else {
                continue;
            };
            if source.viewport == Some(target) && self.images.contains_key(&node) {
                continue;
            }
            if self
                .images
                .get(&node)
                .is_some_and(|image| (image.width, image.height) == target)
            {
                source.viewport = Some(target);
                continue;
            }
            source.viewport = None;
            self.images.remove(&node);
            let bitmap = crate::image::decode_svg_context(
                source.text.as_bytes(),
                Some(target),
                &source.geometry,
                &|| {
                    if started.elapsed() > std::time::Duration::from_secs(45) {
                        Err("Inline SVG image work budget exhausted.".into())
                    } else {
                        Ok(())
                    }
                },
            );
            match bitmap {
                Ok(bitmap) => {
                    let used = self.images.values().try_fold(0usize, |n, image| {
                        image
                            .pixels
                            .capacity()
                            .checked_mul(std::mem::size_of::<u32>())
                            .and_then(|b| n.checked_add(b))
                    });
                    let bytes = bitmap
                        .pixels
                        .capacity()
                        .checked_mul(std::mem::size_of::<u32>());
                    if used
                        .zip(bytes)
                        .and_then(|(a, b)| a.checked_add(b))
                        .is_some_and(|n| n <= loader::MAX_DECODED_IMAGE_BYTES)
                        && self.images.try_reserve(1).is_ok()
                    {
                        self.images.insert(node, std::sync::Arc::new(bitmap));
                        source.viewport = Some(target);
                    } else {
                        success=false;
                        self.status =
                            "Inline SVG used viewport exceeds decoded image allowance.".into();
                    }
                }
                Err(error) => {success=false;self.status = format!("Inline SVG was not painted: {error}");},
            }
        }
        success
    }
    pub(super) fn sync_inline_svg(&mut self, started: std::time::Instant) -> bool {
        let mut success=true;
        let mut previous = std::mem::take(&mut self.inline_svg_sources);
        let mut computed=None;
        let mut stack = Vec::new();
        if stack.try_reserve(1).is_err() {
            for id in previous.keys() {
                self.images.remove(id);
            }
            self.status = "Inline SVG traversal reservation failed.".into();
            return false;
        }
        stack.push(0);
        let mut visited = 0usize;
        while let Some(id) = stack.pop() {
            visited += 1;
            if visited > self.document.nodes.len() {
                success=false;self.status="Inline SVG document topology is cyclic or duplicated.".into();
                break;
            }
            let Some(node) = self.document.nodes.get(id) else {
                success=false;self.status="Inline SVG document node is unavailable.".into();
                break;
            };
            if node.namespace == Namespace::Svg
                && matches!(&node.kind, NodeKind::Element { tag, .. } if tag=="svg")
            {
                if computed.is_none() {
                    let checked=|node|self.forms.controls.get(&node).filter(|c|matches!(c.kind,Kind::Checkbox|Kind::Radio)).map(|c|c.checked);
                    let mut work=usize::MAX;let mut copies=usize::MAX;
                    let mut budget=css::CssBudget::new(&mut work,&mut copies).with_deadline(Some(started+std::time::Duration::from_secs(45)));
                    let cascade=css::compute_with_environment_and_properties_budgeted_cached(&self.document,&self.styles,Some(&checked),self.document.media_environment,&mut budget,self.stylesheet_cache.as_ref());
                    if let Ok(cascade)=&cascade {self.stylesheet_cache=cascade.stylesheet_cache.clone();}
                    computed=Some(cascade.map(|cascade|cascade.computed_properties));
                }
                let candidate = if started.elapsed()>std::time::Duration::from_secs(45) {Err("Inline SVG image work budget exhausted.".into())} else {match computed.as_ref().unwrap() {Ok(values)=>source(&self.document,id,values),Err(error)=>Err(error.clone())}};
                let old = previous.remove(&id);
                match candidate {
                    Ok((source,geometry)) => {
                        let mut backing_seen=std::collections::HashSet::new();
                        let source_bytes = self
                            .inline_svg_sources
                            .values()
                            .try_fold(0usize, |n,s|Source::bytes(&s.text,&s.geometry,&mut backing_seen).and_then(|bytes|n.checked_add(bytes)))
                            .and_then(|n|Source::bytes(&source,&geometry,&mut backing_seen).and_then(|bytes|n.checked_add(bytes)));
                        if source_bytes.is_none_or(|n| n > crate::image::MAX_INPUT_BYTES)
                            || self.inline_svg_sources.try_reserve(1).is_err()
                        {
                            self.images.remove(&id);
                            success=false;
                            self.status =
                                "Inline SVG source cache allowance or reservation exhausted."
                                    .into();
                            continue;
                        }
                        let unchanged = old.as_ref().is_some_and(|s| s.text == source && s.geometry==geometry)
                            && self.images.contains_key(&id);
                        let mut intrinsic = old.as_ref().and_then(|s| s.intrinsic);
                        let mut viewport = old.as_ref().and_then(|s| s.viewport);
                        if !unchanged {
                            intrinsic = None;
                            viewport = None;
                            self.images.remove(&id);
                            let bitmap = crate::image::decode_svg_context(source.as_bytes(), None, &geometry, &|| {
                                if started.elapsed() > std::time::Duration::from_secs(45) {
                                    Err("Inline SVG image work budget exhausted.".into())
                                } else {
                                    Ok(())
                                }
                            });
                            match bitmap {
                                Ok(bitmap) => {
                                    intrinsic = Some((bitmap.width, bitmap.height));
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
                                        success=false;
                                        self.status = "Inline SVG decoded image allowance or reservation exhausted.".into();
                                    }
                                }
                                Err(error) => {
                                    success=false;
                                    self.status = format!("Inline SVG was not painted: {error}");
                                }
                            }
                        }
                        self.inline_svg_sources.insert(
                            id,
                            Source {
                                text: source,
                                geometry,
                                intrinsic,
                                viewport,
                            },
                        );
                    }
                    Err(error) => {
                        success=false;
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
                success=false;self.status="Inline SVG traversal reservation failed.".into();
                break;
            }
            stack.extend(node.children.iter().rev().copied());
        }
        for id in previous.keys() {
            self.images.remove(id);
        }
        success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_shared_paint_cache_charges_shared_backings_once_across_roots() {
        let paint=std::sync::Arc::new(crate::image::SvgPaint::new(0xff203040));
        let mut value=crate::image::SvgStyle::new(16.,16.);value.set_paint(Some(paint),true,0xff203040);
        let context=vec![value.clone(),value];let text=String::from("<svg/>");let mut seen=std::collections::HashSet::new();
        let first=Source::bytes(&text,&context,&mut seen).unwrap();assert_eq!(seen.len(),1);
        let second=Source::bytes(&text,&context,&mut seen).unwrap();assert_eq!(seen.len(),1);assert!(first>second,"a shared allocation is admitted only once across root caches");
        assert_eq!(first-second,std::mem::size_of::<crate::image::SvgPaint>()+2*std::mem::size_of::<usize>());
    }
}
