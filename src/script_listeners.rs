//! Creation-ordered listeners with direct stable-slot lookup. Slots are not
//! reused while a dispatch snapshot can still refer to a removed listener.
use super::Listener;

#[derive(Default)]
pub(super) struct Store {
    entries: Vec<Listener>,
    slots: Vec<Option<usize>>,
    free: Vec<usize>,
}
pub(super) struct Growth {
    entries: Option<Vec<Listener>>,
    slots: Option<Vec<Option<usize>>>,
    free: Option<Vec<usize>>,
    pub added: usize,
    pub copies: usize,
    pub serial: usize,
    reused: bool,
}
use super::retained_storage::capacity as reserve;
impl Store {
    pub fn len(&self)->usize {self.entries.len()}
    #[cfg(test)]
    pub fn is_empty(&self)->bool {self.entries.is_empty()}
    pub fn iter(&self)->std::slice::Iter<'_,Listener> {self.entries.iter()}
    pub fn iter_mut(&mut self)->std::slice::IterMut<'_,Listener> {self.entries.iter_mut()}
    pub fn get(&self,serial:usize)->Option<&Listener> {
        self.slots.get(serial).and_then(|index|*index).and_then(|index|self.entries.get(index))
    }
    #[cfg(test)]
    pub fn storage_bytes(&self)->usize {
        self.entries.capacity()*std::mem::size_of::<Listener>()
            +self.slots.capacity()*std::mem::size_of::<Option<usize>>()
            +self.free.capacity()*std::mem::size_of::<usize>()
    }
    #[cfg(test)]
    pub fn slot_counts(&self)->(usize,usize) {(self.slots.len(),self.free.len())}
    pub fn prepare(&self,available:usize,reuse:bool)->Result<Growth,String> {
        let reused=reuse&&!self.free.is_empty();
        let serial=if reused {*self.free.last().unwrap()}else{self.slots.len()};
        let entry_slot=std::mem::size_of::<Listener>();
        let metadata_slot=std::mem::size_of::<Option<usize>>()+std::mem::size_of::<usize>();
        let need_entries=self.entries.len()==self.entries.capacity();
        let need_slots=!reused&&self.slots.len()==self.slots.capacity();
        let minimum=usize::from(need_entries)*entry_slot+usize::from(need_slots)*metadata_slot;
        if minimum>available{return Err("DOM retained listener storage allowance exhausted.".into());}
        let capacity=if need_entries {
            let growth=self.entries.capacity().max(4).min((available-usize::from(need_slots)*metadata_slot)/entry_slot);
            self.entries.capacity().checked_add(growth).ok_or("Listener entry capacity overflow.")?
        }else{self.entries.capacity()};
        let (entries,entry_bytes)=reserve(&self.entries,capacity,available)?;
        let remaining=available-entry_bytes;
        let capacity=if need_slots {
            let growth=self.slots.capacity().max(4).min(remaining/metadata_slot);
            self.slots.capacity().checked_add(growth).ok_or("Listener slot capacity overflow.")?
        }else{self.slots.capacity()};
        let (slots,slot_bytes)=reserve(&self.slots,capacity,remaining)?;
        let capacity=slots.as_ref().map_or(self.slots.capacity(),Vec::capacity);
        let (free,free_bytes)=reserve(&self.free,capacity,remaining-slot_bytes)?;
        let copies=usize::from(entries.is_some())*self.entries.len()*entry_slot
            +usize::from(slots.is_some())*self.slots.len()*std::mem::size_of::<Option<usize>>()
            +usize::from(free.is_some())*self.free.len()*std::mem::size_of::<usize>();
        Ok(Growth{entries,slots,free,added:entry_bytes+slot_bytes+free_bytes,copies,serial,reused})
    }
    pub fn retain(&mut self,mut keep:impl FnMut(&Listener)->bool)->usize {
        let mut released=0;
        let mut index=0;
        self.entries.retain(|listener| {
            if keep(listener) {self.slots[listener.serial]=Some(index);index+=1;true}
            else {
                self.slots[listener.serial]=None;
                self.free.push(listener.serial);
                released+=listener.event.capacity();
                false
            }
        });
        released
    }
}
impl Growth {
    pub fn commit(self,store:&mut Store,mut listener:Listener) {
        if let Some(mut next)=self.entries {next.append(&mut store.entries);store.entries=next;}
        if let Some(mut next)=self.slots {next.append(&mut store.slots);store.slots=next;}
        if let Some(mut next)=self.free {next.append(&mut store.free);store.free=next;}
        if self.reused {let found=store.free.pop();debug_assert_eq!(found,Some(self.serial));}
        else {store.slots.push(None);}
        listener.serial=self.serial;
        store.slots[self.serial]=Some(store.entries.len());
        store.entries.push(listener);
    }
}
impl<'a> IntoIterator for &'a Store {
    type Item=&'a Listener;
    type IntoIter=std::slice::Iter<'a,Listener>;
    fn into_iter(self)->Self::IntoIter {self.entries.iter()}
}
#[cfg(test)]
impl std::ops::Index<usize> for Store {
    type Output=Listener;
    fn index(&self,index:usize)->&Listener {&self.entries[index]}
}
