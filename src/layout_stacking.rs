//! Hierarchical context paint order. Geometry and source reading order stay separate.
use super::*;
use std::rc::Rc;
#[derive(Clone, Copy)]
enum Kind {
    Context(usize),
    Auto(usize),
}
struct Unit {
    kind: Kind,
    order: usize,
    level: Option<Rc<css::z_index::Value>>,
}
impl Unit {
    fn category(&self) -> u8 {
        match self.level.as_ref().map_or(0, |v| v.sign()) {
            -1 => 1,
            0 => 3,
            _ => 4,
        }
    }
}
#[derive(Default)]
struct Bucket {
    background: Vec<usize>,
    flow: Vec<usize>,
    units: Vec<Unit>,
}
#[derive(Clone, Copy)]
enum Task {
    Context(usize),
    Auto(usize),
    Item(usize),
}
fn reserve<T>(count: usize) -> Result<Vec<T>, String> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| "Stacking metadata allocation failed.")?;
    Ok(v)
}
fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, String> {
    let mut v = reserve(count)?;
    v.resize(count, value);
    Ok(v)
}
fn push<T>(v: &mut Vec<T>, value: T) -> Result<(), String> {
    v.try_reserve(1)
        .map_err(|_| "Stacking metadata growth failed.")?;
    v.push(value);
    Ok(())
}
impl Engine<'_> {
    pub(super) fn rank_auto_positioned_items(&mut self, budget: Option<&mut css::CssBudget<'_>>) -> Result<(), String> {
        let count = self.doc.nodes.len();
        if count == 0 { return Ok(()); }
        if let Some(meter) = budget {
            meter.work(count.saturating_mul(3).saturating_add(self.layout.items.len()))?;
            meter.copies(count.saturating_mul(2 * std::mem::size_of::<usize>()))?;
        }
        let mut groups = filled(count, usize::MAX)?;
        let mut stack = reserve(count)?;
        groups[0] = 0;
        stack.push(0);
        let mut next = 0;
        while let Some(node) = stack.pop() {
            if self.styles[node].position != Position::Static
                && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) {
                next += 1;
                groups[node] = next;
            }
            for &child in self.doc.nodes[node].children.iter().rev() {
                if child < count && self.parents[child] == Some(node) && groups[child] == usize::MAX {
                    groups[child] = groups[node];
                    stack.push(child);
                }
            }
        }
        for (index, item) in self.layout.items.iter().enumerate() {
            let owner = self.paint_owner(item);
            let group = groups.get(owner).copied().filter(|&g| g != usize::MAX)
                .ok_or("Positioned paint item has no connected document owner.")?;
            self.layout.positioned_items[index] = group;
        }
        Ok(())
    }

    fn z_eligible(&self, node: usize) -> bool {
        self.styles[node].position != Position::Static
            || self.parents[node]
                .and_then(|p| self.styles.get(p))
                .is_some_and(|s| s.display.flex_container() || s.display == Display::Grid)
    }
    pub(super) fn order_explicit_stacking(
        &mut self,
        mut budget: Option<&mut css::CssBudget<'_>>,
    ) -> Result<bool, String> {
        let n = self.doc.nodes.len();
        let m = self.layout.items.len();
        if !self
            .styles
            .iter()
            .enumerate()
            .any(|(i, s)| s.display != Display::None && s.z_index.is_some() && self.z_eligible(i))
        {
            return Ok(false);
        }
        let max_level = self
            .styles
            .iter()
            .filter_map(|s| s.z_index.as_ref().map(|v| v.text().len()))
            .max()
            .unwrap_or(1);
        let levels = n.max(1).ilog2() as usize + 1;
        let work = n
            .saturating_mul(16)
            .saturating_add(m.saturating_mul(8))
            .saturating_add(
                n.saturating_mul(levels)
                    .saturating_mul(max_level.saturating_add(1))
                    .saturating_mul(2),
            );
        let storage = n
            .saturating_mul(
                std::mem::size_of::<Bucket>()
                    + std::mem::size_of::<Unit>()
                    + 12 * std::mem::size_of::<usize>(),
            )
            .saturating_add(m.saturating_mul(
                std::mem::size_of::<PaintItem>()
                    + 12 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<Option<ClipRect>>(),
            ));
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(work)?;
            meter.copies(storage)?;
        }
        let mut buckets: Vec<Bucket> = reserve(n)?;
        buckets.resize_with(n, Bucket::default);
        let mut context = filled(n, 0usize)?;
        let mut auto = filled(n, usize::MAX)?;
        let mut seen = filled(n, false)?;
        let mut stack = reserve(n)?;
        let mut ordinal = 0usize;
        let root = self.doc.nodes.first().and_then(|r| {
            r.children
                .iter()
                .copied()
                .find(|&i| matches!(self.doc.nodes[i].kind, NodeKind::Element { .. }))
        });
        if n != 0 {
            stack.push((0usize, 0usize, usize::MAX));
        }
        while let Some((node, mut ctx, mut layer)) = stack.pop() {
            if seen[node] {
                continue;
            }
            seen[node] = true;
            let order = ordinal;
            ordinal += 1;
            let element = matches!(self.doc.nodes[node].kind, NodeKind::Element { .. });
            let style = &self.styles[node];
            let own_opacity = self.opacity_nodes.get(node).is_some_and(|&id| {
                id != self.parents[node]
                    .and_then(|p| self.opacity_nodes.get(p).copied())
                    .unwrap_or(0)
            });
            let transform = self.transformable(node) && style.transform != css::Transform::None;
            let explicit = self.z_eligible(node) && style.z_index.is_some();
            let creates = node != 0
                && element
                && style.display != Display::None
                && (root == Some(node)
                    || explicit
                    || own_opacity
                    || transform
                    || matches!(style.position, Position::Fixed | Position::Sticky));
            if creates {
                let level = if explicit {
                    style.z_index.clone()
                } else {
                    None
                };
                push(
                    &mut buckets[ctx].units,
                    Unit {
                        kind: Kind::Context(node),
                        order,
                        level,
                    },
                )?;
                ctx = node;
                layer = usize::MAX;
            } else if node != 0
                && element
                && style.display != Display::None
                && style.position != Position::Static
            {
                push(
                    &mut buckets[ctx].units,
                    Unit {
                        kind: Kind::Auto(node),
                        order,
                        level: None,
                    },
                )?;
                layer = node;
            }
            context[node] = ctx;
            auto[node] = layer;
            for &child in self.doc.nodes[node].children.iter().rev() {
                if child < n && self.parents[child] == Some(node) && !seen[child] {
                    push(&mut stack, (child, ctx, layer))?;
                }
            }
        }
        for (index, item) in self.layout.items.iter().enumerate() {
            let node = self.paint_owner(item);
            if node >= n || !seen[node] {
                return Err("Stacking paint owner is unavailable.".into());
            }
            let ctx = context[node];
            if node == ctx
                && matches!(
                    item,
                    PaintItem::Rect { .. }
                        | PaintItem::RoundedRect { .. }
                        | PaintItem::Background { .. }
                        | PaintItem::HitBox { .. }
                )
            {
                push(&mut buckets[ctx].background, index)?;
            } else if auto[node] != usize::MAX {
                push(&mut buckets[auto[node]].flow, index)?;
            } else {
                push(&mut buckets[ctx].flow, index)?;
            }
        }
        for bucket in &mut buckets {
            bucket.units.sort_unstable_by(|a, b| {
                let category = a.category().cmp(&b.category());
                if category != std::cmp::Ordering::Equal {
                    return category;
                }
                let z = if matches!(a.category(), 1 | 4) {
                    a.level.as_ref().cmp(&b.level.as_ref())
                } else {
                    std::cmp::Ordering::Equal
                };
                z.then_with(|| a.order.cmp(&b.order))
            });
        }
        let mut tasks = reserve(n.saturating_add(m))?;
        tasks.push(Task::Context(0));
        let mut ranks = filled(m, usize::MAX)?;
        let mut next = 0;
        while let Some(task) = tasks.pop() {
            match task {
                Task::Item(index) => {
                    if ranks[index] != usize::MAX {
                        return Err("Stacking item was emitted twice.".into());
                    }
                    ranks[index] = next;
                    next += 1;
                }
                Task::Auto(node) => {
                    for &item in buckets[node].flow.iter().rev() {
                        push(&mut tasks, Task::Item(item))?;
                    }
                }
                Task::Context(node) => {
                    for unit in buckets[node]
                        .units
                        .iter()
                        .rev()
                        .filter(|u| u.category() >= 3)
                    {
                        push(
                            &mut tasks,
                            match unit.kind {
                                Kind::Context(i) => Task::Context(i),
                                Kind::Auto(i) => Task::Auto(i),
                            },
                        )?;
                    }
                    for &item in buckets[node].flow.iter().rev() {
                        push(&mut tasks, Task::Item(item))?;
                    }
                    for unit in buckets[node]
                        .units
                        .iter()
                        .rev()
                        .filter(|u| u.category() == 1)
                    {
                        push(
                            &mut tasks,
                            match unit.kind {
                                Kind::Context(i) => Task::Context(i),
                                Kind::Auto(i) => Task::Auto(i),
                            },
                        )?;
                    }
                    for &item in buckets[node].background.iter().rev() {
                        push(&mut tasks, Task::Item(item))?;
                    }
                }
            }
        }
        if next != m {
            return Err("Stacking order omitted paint items.".into());
        }
        let mut scene = reserve(m)?;
        let items = std::mem::take(&mut self.layout.items);
        let fixed = std::mem::take(&mut self.layout.fixed_items);
        let layers = std::mem::take(&mut self.layout.positioned_items);
        let clips = std::mem::take(&mut self.layout.item_clips);
        let opacity = std::mem::take(&mut self.opacity_items);
        let has_opacity = !opacity.is_empty();
        for (index, (((item, fixed), layer), clip)) in items
            .into_iter()
            .zip(fixed)
            .zip(layers)
            .zip(clips)
            .enumerate()
        {
            scene.push((
                ranks[index],
                item,
                fixed,
                layer,
                clip,
                if has_opacity { opacity[index] } else { 0 },
            ));
        }
        scene.sort_unstable_by_key(|e| e.0);
        for (_, item, fixed, layer, clip, opacity) in scene {
            self.layout.items.push(item);
            self.layout.fixed_items.push(fixed);
            self.layout.positioned_items.push(layer);
            self.layout.item_clips.push(clip);
            if has_opacity {
                self.opacity_items.push(opacity);
            }
        }
        Ok(true)
    }
}
