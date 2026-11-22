//! Low-level shared normalization entry; VM observation is owned by the caller.
use super::*;
impl Bridge<'_>{pub(super) fn normalize_node(&mut self,object:u32)->Result<Value,String>{self.prepare_shared_normalize(object)?;let result=self.commit_shared_normalize(object);self.abort_shared_normalize(object);result.map(|_|Value::Undefined)}}
