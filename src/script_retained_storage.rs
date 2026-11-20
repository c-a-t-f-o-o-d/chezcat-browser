//! Fallible unpublished reservations for original host metadata vectors.
pub(super) fn capacity<T>(values:&Vec<T>,capacity:usize,available:usize)->Result<(Option<Vec<T>>,usize),String> {
    if capacity<=values.capacity(){return Ok((None,0));}
    let mut next=Vec::new();
    next.try_reserve_exact(capacity).map_err(|_|"Could not reserve host metadata storage.")?;
    let bytes=next.capacity().checked_sub(values.capacity()).and_then(|slots|slots.checked_mul(std::mem::size_of::<T>()))
        .filter(|bytes|*bytes<=available).ok_or("DOM retained host metadata allocation exceeds allowance.")?;
    Ok((Some(next),bytes))
}

pub(super) fn grow<T>(values:&Vec<T>,additional:usize,available:usize)->Result<(Option<Vec<T>>,usize),String> {
    let required=values.len().checked_add(additional).ok_or("Host metadata capacity overflow.")?;
    let before=values.capacity();
    if required<=before{return Ok((None,0));}
    let slot=std::mem::size_of::<T>();
    let growth=before.max(4).max(required-before).min(available/slot);
    let target=before.checked_add(growth).filter(|target|*target>=required).ok_or("DOM retained host metadata allowance exhausted.")?;
    capacity(values,target,available)
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

/// Preserve primitive and native/object identity while copying mutable string
/// backing fallibly. The caller enforces the operation's accepted value kinds.
pub(super) fn copy_value(value:&super::Value)->Result<super::Value,String> {
    use super::Value;
    match value {
        Value::String(text)=>{
            let mut copied=String::new();copied.try_reserve_exact(text.len()).map_err(|_|"Could not reserve native value text.")?;
            copied.push_str(text);Ok(Value::String(copied))
        }
        Value::StringUnits(units)=>{
            let mut copied=Vec::new();copied.try_reserve_exact(units.len()).map_err(|_|"Could not reserve native value code units.")?;
            copied.extend_from_slice(units);Ok(Value::StringUnits(copied))
        }
        _=>Ok(value.clone()),
    }
}
