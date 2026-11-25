use super::*;

#[derive(Default)]
pub(super) struct ItemOrder {
    children: Vec<Option<Vec<usize>>>,
    source: Vec<usize>,
    visual: Vec<usize>,
}

fn reserve<T>(count: usize) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    values.try_reserve_exact(count)
        .map_err(|_| "CSS item order storage allocation failed.")?;
    Ok(values)
}

fn effective<'a>(doc: &Document, styles: &'a [Style], node: usize,
    zero: &'a css::order::Value) -> &'a css::order::Value {
    match (doc.nodes.get(node), styles.get(node)) {
        (Some(node), Some(style))
            if matches!(node.kind, NodeKind::Element { .. })
                && !matches!(style.position, Position::Absolute | Position::Fixed) => &style.order,
        _ => zero,
    }
}

fn preorder(doc: &Document, parents: &[Option<usize>],
    children: &[Option<Vec<usize>>]) -> Result<Vec<usize>, String> {
    let count = doc.nodes.len();
    let mut ranks = reserve(count)?;
    ranks.resize(count, usize::MAX);
    let mut stack = reserve(count)?;
    if count != 0 {
        stack.push(0);
        ranks[0] = usize::MAX - 1;
    }
    let mut next = 0;
    while let Some(node) = stack.pop() {
        ranks[node] = next;
        next += 1;
        let ordered = children.get(node).and_then(Option::as_deref)
            .unwrap_or(&doc.nodes[node].children);
        for &child in ordered.iter().rev() {
            if child < count && parents[child] == Some(node) && ranks[child] == usize::MAX {
                ranks[child] = usize::MAX - 1;
                stack.push(child);
            }
        }
    }
    Ok(ranks)
}

impl ItemOrder {
    pub(super) fn prepare(doc: &Document, styles: &[Style], parents: &[Option<usize>],
        mut budget: Option<&mut css::CssBudget<'_>>) -> Result<Self, String> {
        if !styles.iter().any(|style| !style.order.is_zero()) {
            return Ok(Self::default());
        }
        let count = doc.nodes.len();
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(count.saturating_mul(8))?;
            meter.copies(count.saturating_mul(
                std::mem::size_of::<Option<Vec<usize>>>() + 4 * std::mem::size_of::<usize>()))?;
        }
        let mut children = reserve(count)?;
        children.resize_with(count, || None);
        let zero = css::order::Value::default();
        let mut changed = false;
        for (node, parent) in doc.nodes.iter().enumerate() {
            if !styles.get(node).is_some_and(|s| s.display.flex_container() || s.display == Display::Grid)
                || parent.children.len() < 2 {
                continue;
            }
            let length = parent.children.len();
            let digits = parent.children.iter()
                .map(|&child| effective(doc, styles, child, &zero).text().len()).max().unwrap_or(1);
            if let Some(meter) = budget.as_deref_mut() {
                let levels = length.ilog2() as usize + 1;
                meter.work(length.saturating_mul(levels).saturating_mul(digits.saturating_add(3))
                    .saturating_mul(2))?;
                meter.copies(length.saturating_mul(std::mem::size_of::<usize>()))?;
            }
            let mut indices = reserve(length)?;
            indices.extend(0..length);
            indices.sort_unstable_by(|&a, &b| {
                effective(doc, styles, parent.children[a], &zero)
                    .cmp(effective(doc, styles, parent.children[b], &zero))
                    .then_with(|| a.cmp(&b))
            });
            if indices.iter().enumerate().all(|(index, &value)| index == value) {
                continue;
            }
            for index in &mut indices {
                *index = parent.children[*index];
            }
            children[node] = Some(indices);
            changed = true;
        }
        if !changed {
            return Ok(Self::default());
        }
        let source = preorder(doc, parents, &[])?;
        let visual = preorder(doc, parents, &children)?;
        Ok(Self { children, source, visual })
    }

    pub(super) fn active(&self) -> bool {
        !self.visual.is_empty()
    }

    pub(super) fn reorders(&self, node: usize) -> bool {
        self.children.get(node).is_some_and(Option::is_some)
    }

    pub(super) fn children<'a>(&'a self, doc: &'a Document, node: usize) -> &'a [usize] {
        self.children.get(node).and_then(Option::as_deref).unwrap_or(&doc.nodes[node].children)
    }

    pub(super) fn source_rank(&self, node: usize) -> usize {
        self.source.get(node).copied().unwrap_or(node)
    }

    pub(super) fn visual_rank(&self, node: usize) -> usize {
        self.visual.get(node).copied().unwrap_or(node)
    }
}

impl Engine<'_> {
    pub(super) fn visual_children(&self, node: usize) -> &[usize] {
        self.item_order.children(self.doc, node)
    }
}
