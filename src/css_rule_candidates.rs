//! Conservative candidates for rules with a mandatory rightmost class or ID.
use super::*;
use std::borrow::Cow;

const BUCKETS: usize = 64;
const MAX_KEY_BYTES: usize = 4096;

pub(super) struct RuleCandidates {
    unkeyed: Vec<usize>,
    keyed: [Vec<usize>; BUCKETS],
    occupied: u64,
    has_classes: bool,
    has_ids: bool,
}

fn bucket(name: &str) -> usize {
    let hash = name.bytes().fold(0x811c9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
    });
    ((hash ^ (hash >> 16)) as usize) % BUCKETS
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KeyKind { Class, Id }

fn selector_key(selector: &Selector) -> Option<(KeyKind, &str)> {
    let rightmost = selector.parts.last()?;
    rightmost.classes.first().map(|class| (KeyKind::Class, class.as_str()))
        .or_else(|| rightmost.ids.first().map(|id| (KeyKind::Id, id.as_str())))
}

fn selector_bucket(selector: &Selector) -> Option<(KeyKind, usize)> {
    selector_key(selector).map(|(kind, name)| (kind, bucket(name)))
}

fn selected_buckets(mut keys: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if keys == 0 { return None; }
        let key = keys.trailing_zeros() as usize;
        keys &= keys - 1;
        Some(key)
    })
}

impl RuleCandidates {
    pub(super) fn build(rules: &[Rule], mut meter: Option<&mut CssBudget<'_>>) -> Result<Self, String> {
        if let Some(meter) = meter.as_deref_mut() {
            meter.copies(std::mem::size_of::<Self>() +
                (rules.len() * 2 + 4 * (BUCKETS + 1)) * std::mem::size_of::<usize>())?;
        }
        let mut candidates = Self {
            unkeyed: Vec::new(), keyed: std::array::from_fn(|_| Vec::new()), occupied: 0,
            has_classes: false, has_ids: false,
        };
        for (index, rule) in rules.iter().enumerate() {
            if let Some(meter) = meter.as_deref_mut() {
                for selector in &rule.selectors {
                    let bytes = selector_key(selector).map_or(0, |(_, name)| name.len());
                    meter.work(1 + bytes)?;
                }
            }
            // Every alternative must require this bucket. Collisions only
            // admit extra candidates, which still run the complete matcher.
            let key = rule.selectors.first().and_then(selector_bucket).filter(|&key| {
                rule.selectors.iter().skip(1).all(|selector| selector_bucket(selector) == Some(key))
            });
            match key {
                Some((kind, key)) => {
                    match kind {
                        KeyKind::Class => candidates.has_classes = true,
                        KeyKind::Id => candidates.has_ids = true,
                    }
                    candidates.keyed[key].push(index);
                    candidates.occupied |= 1u64 << key;
                }
                None => candidates.unkeyed.push(index),
            }
        }
        Ok(candidates)
    }

    pub(super) fn for_element(
        &self, context: &MatchContext<'_>, node: usize, count: usize,
        mut meter: Option<&mut CssBudget<'_>>,
    ) -> Result<Option<Cow<'_, [usize]>>, String> {
        if self.unkeyed.len() == count { return Ok(None); }
        let mut keys = 0;
        if self.has_classes {
            let before = meter.as_ref().map_or(usize::MAX, |meter| meter.remaining_work());
            let mut work = before;
            let attribute = context.attribute(node, "class", &mut work).unwrap_or("");
            if let Some(meter) = meter.as_deref_mut() {
                meter.work(before - work)?;
                if work == 0 { return Err("CSS selector job work limit reached.".into()); }
            }
            if attribute.len() > MAX_KEY_BYTES { return Ok(None); }
            if let Some(meter) = meter.as_deref_mut() { meter.work(1 + attribute.len() * 2)?; }
            for class in attribute.split_ascii_whitespace() { keys |= 1u64 << bucket(class); }
        }
        if self.has_ids {
            if let Some(meter) = meter.as_deref_mut() { meter.work(1)?; }
            if let NodeKind::Element { id: Some(id), .. } = &context.doc.nodes[node].kind {
                if id.len() > MAX_KEY_BYTES { return Ok(None); }
                if let Some(meter) = meter.as_deref_mut() { meter.work(id.len())?; }
                keys |= 1u64 << bucket(id);
            }
        }
        let keys = self.occupied & keys;
        if keys == 0 { return Ok(Some(Cow::Borrowed(&self.unkeyed))); }
        if let Some(meter) = meter.as_deref_mut() {
            meter.work(4 * keys.count_ones() as usize)?;
        }
        let added: usize = selected_buckets(keys).map(|key| self.keyed[key].len()).sum();
        let total = added + self.unkeyed.len();
        if total == count { return Ok(None); }
        let single_bucket = keys.is_power_of_two();
        if let Some(meter) = meter.as_deref_mut() {
            let work_per_rule = if single_bucket { 4 }
                else { 4 * (usize::BITS - total.leading_zeros()) as usize + 1 };
            meter.work(total.saturating_mul(work_per_rule))?;
            meter.copies(total.saturating_mul(std::mem::size_of::<usize>()))?;
        }
        let mut indices = Vec::with_capacity(total);
        if single_bucket {
            let mut keyed = self.keyed[keys.trailing_zeros() as usize].iter().copied().peekable();
            for &index in &self.unkeyed {
                while keyed.peek().is_some_and(|&next| next < index) {
                    indices.push(keyed.next().unwrap());
                }
                indices.push(index);
            }
            indices.extend(keyed);
            return Ok(Some(Cow::Owned(indices)));
        }
        indices.extend_from_slice(&self.unkeyed);
        for key in selected_buckets(keys) {
            indices.extend_from_slice(&self.keyed[key]);
        }
        indices.sort_unstable();
        Ok(Some(Cow::Owned(indices)))
    }
}
