//! Original sorted native identity index with explicitly admitted backing.
//! Keys/handles are copied integers; insertion/removal never allocates after
//! the caller admits reservation, binary lookup and element movement work.
use super::*;
pub(super) struct IdentityMap<K> { entries:Vec<(K,u32)> }
impl<K> Default for IdentityMap<K> {fn default()->Self{Self{entries:Vec::new()}}}
impl<K:Ord+Copy> IdentityMap<K> {
    pub(super) fn len(&self)->usize{self.entries.len()}
    #[cfg(test)]
    pub(super) fn is_empty(&self)->bool{self.entries.is_empty()}
    pub(super) fn storage_bytes(&self)->usize{self.entries.capacity()*size_of::<(K,u32)>()}
    pub(super) fn minimum_growth(&self, additional:usize)->Result<usize,String>{
        self.len().checked_add(additional)
            .and_then(|required|required.saturating_sub(self.entries.capacity()).checked_mul(size_of::<(K,u32)>()))
            .ok_or("Native identity index capacity overflow.".into())
    }
    pub(super) fn lookup_work(&self)->usize{(usize::BITS-self.entries.len().leading_zeros()) as usize+1}
    pub(super) fn get(&self,key:&K)->Option<&u32>{self.entries.binary_search_by_key(key,|entry|entry.0).ok().map(|index|&self.entries[index].1)}
    pub(super) fn contains_key(&self,key:&K)->bool{self.get(key).is_some()}
    pub(super) fn insert_work(&self,key:&K)->usize {
        let shifted=match self.entries.binary_search_by_key(key,|entry|entry.0){Ok(_)=>0,Err(index)=>self.len()-index};
        self.lookup_work()+shifted
    }
    pub(super) fn movement_bytes(&self,work:usize)->usize{work.saturating_mul(size_of::<(K,u32)>())}
    pub(super) fn prepare(&self,additional:usize,available:usize)->Result<(Option<Vec<(K,u32)>>,usize),String>{retained_storage::grow(&self.entries,additional,available)}
    pub(super) fn commit(&mut self,replacement:Option<Vec<(K,u32)>>) {
        if let Some(mut next)=replacement{next.append(&mut self.entries);self.entries=next;}
    }
    pub(super) fn insert(&mut self,key:K,value:u32) {
        match self.entries.binary_search_by_key(&key,|entry|entry.0) {
            Ok(index)=>self.entries[index].1=value,
            Err(index)=>{assert!(self.len()<self.entries.capacity(),"Identity index insertion requires an admitted slot");self.entries.insert(index,(key,value));}
        }
    }
    pub(super) fn remove(&mut self,key:&K)->Option<u32> {
        let index=self.entries.binary_search_by_key(key,|entry|entry.0).ok()?;
        Some(self.entries.remove(index).1)
    }
}
#[cfg(test)]
impl<K:Ord+Copy> std::ops::Index<&K> for IdentityMap<K> {
    type Output=u32;
    fn index(&self,key:&K)->&u32{self.get(key).expect("Existing native identity")}
}

#[cfg(test)]
#[path = "script_child_list_map_tests.rs"]
mod tests;
