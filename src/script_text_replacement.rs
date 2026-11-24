//! Unpublished scratch for main-document text replacement.
use super::*;

pub(super) fn bytes(vectors: &[&Vec<usize>], text: usize) -> Result<usize, String> {
    vectors.iter().try_fold(text, |total, values| {
        values.capacity().checked_mul(size_of::<usize>()).and_then(|bytes| total.checked_add(bytes))
            .ok_or("textContent traversal storage overflow.".into())
    })
}

pub(super) fn push(state: &mut State, values: &mut Vec<usize>, value: usize, other: usize) -> Result<(), String> {
    if state.execution_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err("DOM computation time limit reached.".into());
    }
    let slot = size_of::<usize>();
    let old = values.capacity().checked_mul(slot).ok_or("textContent traversal storage overflow.")?;
    let available = state.retained_limit.checked_sub(state.retained).and_then(|bytes| bytes.checked_sub(other))
        .ok_or("textContent traversal storage allowance exhausted.")?;
    if old > available { return Err("textContent traversal storage allowance exhausted.".into()); }
    if values.len() == values.capacity() {
        // Both old and replacement backing coexist until the copy completes.
        let slots = available.saturating_sub(old) / slot;
        let required = values.len().checked_add(1).ok_or("textContent traversal capacity overflow.")?;
        if required > slots { return Err("textContent traversal storage allowance exhausted.".into()); }
        let target = values.capacity().saturating_mul(2).max(4).min(slots).max(required);
        state.copies(values.len().checked_mul(slot).ok_or("textContent traversal copy overflow.")?)?;
        let mut next = Vec::new();
        next.try_reserve_exact(target).map_err(|_| "textContent traversal reservation failed.")?;
        if next.capacity().checked_mul(slot).and_then(|bytes| bytes.checked_add(old)).is_none_or(|bytes| bytes > available) {
            return Err("textContent traversal storage allowance exhausted.".into());
        }
        next.append(values);
        *values = next;
    }
    state.copies(slot)?;
    values.push(value);
    Ok(())
}
