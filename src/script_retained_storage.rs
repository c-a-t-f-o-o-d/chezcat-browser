//! Fallible unpublished reservations for original host metadata vectors.
pub(super) fn capacity<T>(values:&Vec<T>,capacity:usize,available:usize)->Result<(Option<Vec<T>>,usize),String> {
    if capacity<=values.capacity(){return Ok((None,0));}
    let mut next=Vec::new();
    next.try_reserve_exact(capacity).map_err(|_|"Could not reserve host metadata storage.")?;
    let bytes=next.capacity().checked_sub(values.capacity()).and_then(|slots|slots.checked_mul(std::mem::size_of::<T>()))
        .filter(|bytes|*bytes<=available).ok_or("DOM retained host metadata allocation exceeds allowance.")?;
    Ok((Some(next),bytes))
}

/// Capacity for copied strings/code units; immutable BigInt shares retain their
/// full payload allowance even though cloning the reference allocates no text.
pub(super) fn value_bytes(value:&super::Value)->usize {
    match value {
        super::Value::String(text)=>text.capacity(),
        super::Value::StringUnits(units)=>units.capacity().saturating_mul(2),
        super::Value::BigInt(text)=>text.len(),
        _=>0,
    }
}
