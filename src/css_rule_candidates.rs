//! Conservative candidates for rules with a mandatory rightmost class.
use super::*;
use std::borrow::Cow;

const BUCKETS: usize = 64;
const MAX_CLASS_BYTES: usize = 4096;

pub(super) struct RuleCandidates {
    unkeyed: Vec<usize>,
    classes: [Vec<usize>; BUCKETS],
    occupied: u64,
}

fn bucket(class: &str) -> usize {
    let hash = class.bytes().fold(0x811c9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
    });
    ((hash ^ (hash >> 16)) as usize) % BUCKETS
}

fn selector_bucket(selector: &Selector) -> Option<usize> {
    selector.parts.last()?.classes.first().map(|class| bucket(class))
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
            unkeyed: Vec::new(), classes: std::array::from_fn(|_| Vec::new()), occupied: 0,
        };
        for (index, rule) in rules.iter().enumerate() {
            if let Some(meter) = meter.as_deref_mut() {
                for selector in &rule.selectors {
                    let bytes = selector.parts.last().and_then(|part| part.classes.first()).map_or(0, String::len);
                    meter.work(1 + bytes)?;
                }
            }
            // Every alternative must require this bucket. Collisions only
            // admit extra candidates, which still run the complete matcher.
            let key = rule.selectors.first().and_then(selector_bucket).filter(|&key| {
                rule.selectors.iter().skip(1).all(|selector| selector_bucket(selector) == Some(key))
            });
            match key {
                Some(key) => {
                    candidates.classes[key].push(index);
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
        let before = meter.as_ref().map_or(usize::MAX, |meter| meter.remaining_work());
        let mut work = before;
        let attribute = context.attribute(node, "class", &mut work).unwrap_or("");
        if let Some(meter) = meter.as_deref_mut() {
            meter.work(before - work)?;
            if work == 0 { return Err("CSS selector job work limit reached.".into()); }
        }
        if attribute.len() > MAX_CLASS_BYTES { return Ok(None); }
        if let Some(meter) = meter.as_deref_mut() {
            meter.work(1 + attribute.len() * 2)?;
        }

        let keys = self.occupied & attribute.split_ascii_whitespace()
            .fold(0u64, |keys, class| keys | (1u64 << bucket(class)));
        if keys == 0 { return Ok(Some(Cow::Borrowed(&self.unkeyed))); }
        if let Some(meter) = meter.as_deref_mut() {
            meter.work(4 * keys.count_ones() as usize)?;
        }
        let added: usize = selected_buckets(keys).map(|key| self.classes[key].len()).sum();
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
            let mut keyed = self.classes[keys.trailing_zeros() as usize].iter().copied().peekable();
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
            indices.extend_from_slice(&self.classes[key]);
        }
        indices.sort_unstable();
        Ok(Some(Cow::Owned(indices)))
    }
}
