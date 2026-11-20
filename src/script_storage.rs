//! Original transferable ownership charges for script-loader allocations.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize,Ordering};
#[derive(Debug,Default)]
pub(crate) struct Lease {used:Arc<AtomicUsize>,bytes:usize}
impl Lease {
    pub(crate) fn new(used:Arc<AtomicUsize>,bytes:usize,limit:usize)->Result<Self,String> {
        used.try_update(Ordering::AcqRel,Ordering::Acquire,|used|used.checked_add(bytes).filter(|used|*used<=limit))
            .map_err(|_|"Script retained storage allowance exhausted.")?;
        Ok(Self{used,bytes})
    }
    pub(crate) fn empty(used:Arc<AtomicUsize>)->Self{Self{used,bytes:0}}
    pub(crate) fn merge(&mut self,mut other:Self){assert!(Arc::ptr_eq(&self.used,&other.used));self.bytes+=other.bytes;other.bytes=0;}
}
impl Drop for Lease {fn drop(&mut self){if self.bytes!=0{self.used.fetch_sub(self.bytes,Ordering::AcqRel);}}}
