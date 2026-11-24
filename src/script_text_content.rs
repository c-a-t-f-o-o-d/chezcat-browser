//! Admission for original DOM descendant-text joins, including UTF-16 scratch.
use super::*;
pub(super) fn deadline(state:&State)->Result<(),String>{
    if state.execution_deadline.is_some_and(|deadline|Instant::now()>=deadline){return Err("DOM computation time limit reached.".into());}
    Ok(())
}
pub(super) fn available(state:&State,scratch:usize)->Result<usize,String>{
    state.retained_limit.checked_sub(state.retained).and_then(|bytes|bytes.checked_sub(scratch))
        .ok_or("textContent traversal storage allowance exhausted.".into())
}
pub(super) fn parts<T>(count:usize,available:usize)->Result<Vec<T>,String>{
    let minimum=count.checked_mul(size_of::<T>()).ok_or("textContent parts storage overflow.")?;
    if minimum>available{return Err("textContent parts storage allowance exhausted.".into());}
    let mut parts=Vec::new();parts.try_reserve_exact(count).map_err(|_|"Cannot reserve textContent parts.")?;
    if parts.capacity().checked_mul(size_of::<T>()).is_none_or(|bytes|bytes>available){return Err("textContent parts storage allowance exhausted.".into());}
    Ok(parts)
}
pub(super) fn plan(parts:&[&DomString],string_limit:usize,available:usize)->Result<(usize,usize,usize),String>{
    let (logical,owned)=DomString::concat_size(parts)?;
    if logical>string_limit{return Err("textContent result exceeds the host string allowance.".into());}
    // concat builds a temporary code-unit vector before constructing the owned
    // projection and, when needed, its independent exact code-unit backing.
    let raw=if parts.iter().any(|part|part.raw_units().is_some()){
        parts.iter().try_fold(0usize,|count,part|count.checked_add(part.utf16_len()).ok_or("textContent joined scratch overflow."))?
            .checked_mul(2).ok_or("textContent joined scratch overflow.")?
    }else{0};
    let budget=available.checked_sub(raw).ok_or("textContent joined scratch storage allowance exhausted.")?
        .min(DomString::storage_bound(string_limit)?);
    if owned>budget{return Err("textContent returned text storage allowance exhausted.".into());}
    Ok((DomString::concat_copy_work(parts)?,budget,raw))
}
