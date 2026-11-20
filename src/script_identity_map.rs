//! Original sorted native identity index with explicitly admitted backing.
//! Keys/handles are copied integers; insertion/removal never allocates after
//! the caller admits reservation, binary lookup and element movement work.
use super::*;
pub(super) struct IdentityMap<K,V=u32> { entries:Vec<(K,V)> }
impl<K,V> Default for IdentityMap<K,V> {fn default()->Self{Self{entries:Vec::new()}}}
impl<K:Ord+Copy,V> IdentityMap<K,V> {
    pub(super) fn len(&self)->usize{self.entries.len()}
    #[cfg(test)]
    pub(super) fn is_empty(&self)->bool{self.entries.is_empty()}
    pub(super) fn storage_bytes(&self)->usize{self.entries.capacity()*size_of::<(K,V)>()}
    pub(super) fn minimum_growth(&self, additional:usize)->Result<usize,String>{
        self.len().checked_add(additional)
            .and_then(|required|required.saturating_sub(self.entries.capacity()).checked_mul(size_of::<(K,V)>()))
            .ok_or("Native identity index capacity overflow.".into())
    }
    pub(super) fn lookup_work(&self)->usize{(usize::BITS-self.entries.len().leading_zeros()) as usize+1}
    pub(super) fn get(&self,key:&K)->Option<&V>{self.entries.binary_search_by_key(key,|entry|entry.0).ok().map(|index|&self.entries[index].1)}
    pub(super) fn get_mut(&mut self,key:&K)->Option<&mut V>{
        let index=self.entries.binary_search_by_key(key,|entry|entry.0).ok()?;
        Some(&mut self.entries[index].1)
    }
    pub(super) fn retention_movement(&self,mut keep:impl FnMut(&K)->bool)->usize{
        let(mut removed,mut moved)=(false,0usize);
        for (key,_) in &self.entries {
            if !keep(key){removed=true;}else if removed{moved+=1;}
        }
        moved
    }
    pub(super) fn retain(&mut self,mut keep:impl FnMut(&K)->bool){
        self.entries.retain(|entry|keep(&entry.0));
    }
    // The caller admits record writes, sorting and any required backing, and
    // validates that every incoming key is distinct from the existing keys.
    pub(super) fn append_and_sort(&mut self, additional:usize, entries:impl IntoIterator<Item=(K,V)>){
        assert!(self.len().checked_add(additional).is_some_and(|length|length<=self.entries.capacity()),
            "Batched identity insertion requires admitted backing");
        let mut inserted=0usize;
        for entry in entries {
            assert!(inserted<additional,"Batched identity count must match its admission");
            self.entries.push(entry);inserted+=1;
        }
        assert_eq!(inserted,additional,"Batched identity count must match its admission");
        if additional!=0{self.entries.sort_unstable_by_key(|entry|entry.0);}
    }
    pub(super) fn contains_key(&self,key:&K)->bool{self.get(key).is_some()}
    pub(super) fn insert_work(&self,key:&K)->usize {
        let shifted=match self.entries.binary_search_by_key(key,|entry|entry.0){Ok(_)=>0,Err(index)=>self.len()-index};
        self.lookup_work()+shifted
    }
    pub(super) fn movement_bytes(&self,work:usize)->usize{work.saturating_mul(size_of::<(K,V)>())}
    pub(super) fn prepare(&self,additional:usize,available:usize)->Result<(Option<Vec<(K,V)>>,usize),String>{retained_storage::grow(&self.entries,additional,available)}
    pub(super) fn commit(&mut self,replacement:Option<Vec<(K,V)>>) {
        if let Some(mut next)=replacement{next.append(&mut self.entries);self.entries=next;}
    }
    pub(super) fn insert(&mut self,key:K,value:V) {
        match self.entries.binary_search_by_key(&key,|entry|entry.0) {
            Ok(index)=>self.entries[index].1=value,
            Err(index)=>{assert!(self.len()<self.entries.capacity(),"Identity index insertion requires an admitted slot");self.entries.insert(index,(key,value));}
        }
    }
    pub(super) fn remove(&mut self,key:&K)->Option<V> {
        let index=self.entries.binary_search_by_key(key,|entry|entry.0).ok()?;
        Some(self.entries.remove(index).1)
    }
}
#[cfg(test)]
impl<K:Ord+Copy,V> std::ops::Index<&K> for IdentityMap<K,V> {
    type Output=V;
    fn index(&self,key:&K)->&V{self.get(key).expect("Existing native identity")}
}

#[cfg(test)]
#[path = "script_child_list_map_tests.rs"]
mod tests;
