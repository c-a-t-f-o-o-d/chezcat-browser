//! Visibility filters output, not measured layout or guarded resource ownership.
use super::*;
fn owner(item: &PaintItem, runs: &[TextRun], controls: &[ControlBox]) -> usize {
    match item {
        PaintItem::Text(i) => runs[*i].node,
        PaintItem::Control(i) => controls[*i].node,
        PaintItem::Rect { node, .. }
        | PaintItem::RoundedRect { node, .. }
        | PaintItem::Image { node, .. }
        | PaintItem::Background { node, .. }
        | PaintItem::HitBox { node, .. } => *node,
    }
}
fn visible(styles: &[std::rc::Rc<Style>], node: usize) -> bool {
    node == 0 || styles.get(node).is_some_and(|s| s.visible)
}
fn mapping(count: usize) -> Result<Vec<usize>, String> {
    let mut map = Vec::new();
    map.try_reserve_exact(count)
        .map_err(|_| "Visibility index allocation failed.")?;
    map.resize(count, usize::MAX);
    Ok(map)
}
impl Engine<'_> {
    pub(super) fn paint_owner(&self, item: &PaintItem) -> usize {
        owner(item, &self.layout.runs, &self.layout.controls)
    }
    pub(super) fn filter_visibility_paint(
        &mut self,
        mut budget: Option<&mut css::CssBudget<'_>>,
    ) -> Result<(), String> {
        if self.styles.iter().all(|s| s.visible) {
            return Ok(());
        }
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(self.layout.items.len())?;
        }
        let hidden_backgrounds = self
            .layout
            .items
            .iter()
            .filter(|item| {
                matches!(item, PaintItem::Background { .. })
                    && !visible(&self.styles, self.paint_owner(item))
            })
            .count();
        if let Some(meter) = budget.as_deref_mut() {
            meter.copies(
                hidden_backgrounds
                    .saturating_mul(std::mem::size_of::<(usize, css::BackgroundImage)>()),
            )?;
        }
        self.layout
            .unpainted_backgrounds
            .try_reserve_exact(hidden_backgrounds)
            .map_err(|_| "Hidden background reference allocation failed.")?;
        let styles = &self.styles;
        let layout = &mut self.layout;
        let runs = &layout.runs;
        let controls = &layout.controls;
        let fixed = &mut layout.fixed_items;
        let layers = &mut layout.positioned_items;
        let clips = &mut layout.item_clips;
        let resources = &mut layout.unpainted_backgrounds;
        let opacity = &mut self.opacity_items;
        let has_opacity = !opacity.is_empty();
        let (mut index, mut kept) = (0, 0);
        layout.items.retain_mut(|item| {
            let keep = visible(styles, owner(item, runs, controls));
            if keep {
                fixed[kept] = fixed[index];
                layers[kept] = layers[index];
                clips[kept] = clips[index];
                if has_opacity {
                    opacity[kept] = opacity[index];
                }
                kept += 1;
            } else if matches!(item, PaintItem::Background { .. }) {
                let old = std::mem::replace(
                    item,
                    PaintItem::Rect {
                        node: 0,
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                        color: 0,
                        alpha: 0,
                    },
                );
                if let PaintItem::Background { node, image, .. } = old {
                    resources.push((node, image));
                }
            }
            index += 1;
            keep
        });
        fixed.truncate(kept);
        layers.truncate(kept);
        clips.truncate(kept);
        if has_opacity {
            opacity.truncate(kept);
        }
        Ok(())
    }
    pub(super) fn finish_visibility_outputs(
        &mut self,
        mut budget: Option<&mut css::CssBudget<'_>>,
    ) -> Result<(), String> {
        if self.styles.iter().all(|s| s.visible) {
            return Ok(());
        }
        let nr = self.layout.runs.len();
        let nc = self.layout.controls.len();
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(
                nr.saturating_add(nc)
                    .saturating_add(self.layout.items.len()),
            )?;
            meter.copies(
                nr.saturating_add(nc)
                    .saturating_mul(std::mem::size_of::<usize>()),
            )?;
        }
        let mut run_map = mapping(nr)?;
        let mut control_map = mapping(nc)?;
        let styles = &self.styles;
        let layout = &mut self.layout;
        let fixed = &mut layout.fixed_runs;
        let clips = &mut layout.run_clips;
        let (mut index, mut kept) = (0, 0);
        layout.runs.retain(|run| {
            let keep = visible(styles, run.node);
            if keep {
                run_map[index] = kept;
                fixed[kept] = fixed[index];
                clips[kept] = clips[index];
                kept += 1;
            }
            index += 1;
            keep
        });
        fixed.truncate(kept);
        clips.truncate(kept);
        let fixed = &mut layout.fixed_controls;
        let (mut index, mut kept) = (0, 0);
        layout.controls.retain(|control| {
            let keep = visible(styles, control.node);
            if keep {
                control_map[index] = kept;
                fixed[kept] = fixed[index];
                kept += 1;
            }
            index += 1;
            keep
        });
        fixed.truncate(kept);
        for item in &mut layout.items {
            match item {
                PaintItem::Text(i) => *i = run_map[*i],
                PaintItem::Control(i) => *i = control_map[*i],
                _ => {}
            }
        }
        Ok(())
    }
}
