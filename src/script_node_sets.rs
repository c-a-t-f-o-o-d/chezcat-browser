//! Paid bitmap sets for dense, stable main-document node identities.
use super::*;
#[derive(Default)]
#[cfg_attr(test,derive(Clone,Debug,PartialEq,Eq))]
pub(super) struct NodeSet {words:Vec<u64>,count:usize}
impl NodeSet {
    #[cfg(test)]
    pub(super) fn len(&self)->usize{self.count}
    #[cfg(test)]
    pub(super) fn is_empty(&self)->bool{self.count==0}
    pub(super) fn storage_bytes(&self)->usize{self.words.capacity()*size_of::<u64>()}
    pub(super) fn contains(&self,node:&usize)->bool{self.words.get(*node/64).is_some_and(|word|word&(1u64<<(*node%64))!=0)}
    pub(super) fn insert(&mut self,node:usize)->bool {
        let word=self.words.get_mut(node/64).expect("Node identity domain must be admitted before publication");
        let mask=1u64<<(node%64);let added=*word&mask==0;*word|=mask;
        if added{self.count+=1;}added
    }
    pub(super) fn remove(&mut self,node:&usize)->bool {
        let Some(word)=self.words.get_mut(*node/64)else{return false;};let mask=1u64<<(*node%64);
        let removed=*word&mask!=0;*word&=!mask;if removed{self.count-=1;}removed
    }
    pub(super) fn scan_work(&self)->usize{if self.count==0{0}else{self.words.len()+self.count}}
    pub(super) fn iter(&self)->Iter<'_>{Iter{words:if self.count==0{&[]}else{&self.words},next:0,pending:0,remaining:self.count}}
    fn commit(&mut self,growth:Option<Vec<u64>>,required:usize){
        if let Some(mut next)=growth{next.append(&mut self.words);self.words=next;}
        if required>self.words.len(){self.words.resize(required,0);}
    }
}
impl Extend<usize> for NodeSet {fn extend<T:IntoIterator<Item=usize>>(&mut self,values:T){for value in values{self.insert(value);}}}
pub(super) struct Iter<'a>{words:&'a [u64],next:usize,pending:u64,remaining:usize}
impl Iterator for Iter<'_>{
    type Item=usize;
    fn next(&mut self)->Option<usize>{
        while self.pending==0 {self.pending=*self.words.get(self.next)?;self.next+=1;}
        let bit=self.pending.trailing_zeros() as usize;self.pending&=self.pending-1;self.remaining-=1;
        Some((self.next-1)*64+bit)
    }
    fn size_hint(&self)->(usize,Option<usize>){(self.remaining,Some(self.remaining))}
}
impl ExactSizeIterator for Iter<'_>{}
#[derive(Default)]
pub(super) struct Growth {words:[Option<Vec<u64>>;5],required:usize}
fn words(nodes:usize)->Result<usize,String>{
    nodes.checked_add(63).filter(|_|nodes<=NODE_HANDLES as usize).map(|count|count/64).ok_or("DOM node bitmap domain overflow.".into())
}
impl State {
    fn node_sets(&self)->[&NodeSet;5]{[&self.dom.created,&self.detached_nodes,&self.detached_images,&self.dom.fragment_scripts,&self.dom.force_async]}
    pub(super) fn node_sets_storage_bytes(&self)->usize{self.node_sets().iter().map(|set|set.storage_bytes()).sum()}
    pub(super) fn node_sets_minimum(&self,nodes:usize)->Result<usize,String>{
        let required=words(nodes)?;self.node_sets().iter().try_fold(0usize,|total,set|{
            required.saturating_sub(set.words.capacity()).checked_mul(size_of::<u64>()).and_then(|bytes|total.checked_add(bytes))
                .ok_or("DOM node bitmap backing overflow.".into())
        })
    }
    pub(super) fn prepare_node_sets(&self,nodes:usize,available:usize)->Result<(Growth,usize,usize,usize),String>{
        let required=words(nodes)?;let sets=self.node_sets();
        let mut minimums=[0usize;5];let mut remaining_min=0usize;
        for (slot,set) in sets.iter().enumerate(){
            minimums[slot]=required.saturating_sub(set.words.capacity()).checked_mul(size_of::<u64>()).ok_or("DOM node bitmap backing overflow.")?;
            remaining_min=remaining_min.checked_add(minimums[slot]).ok_or("DOM node bitmap backing overflow.")?;
        }
        let mut growth=Growth{words:std::array::from_fn(|_|None),required};let(mut used,mut records)=(0usize,0usize);
        for (slot,set) in sets.iter().enumerate(){
            remaining_min-=minimums[slot];
            let budget=available.checked_sub(used).and_then(|bytes|bytes.checked_sub(remaining_min)).ok_or("DOM retained joint bitmap allowance exhausted.")?;
            let added=required.saturating_sub(set.words.len());
            let usable=(NODE_HANDLES as usize/64).saturating_sub(set.words.capacity()).checked_mul(size_of::<u64>())
                .ok_or("DOM bitmap representation backing overflow.")?;
            let(next,bytes)=retained_storage::grow(&set.words,added,budget.min(usable))?;
            let moved=if next.is_some(){set.words.len()}else{0};
            records=records.checked_add(added).and_then(|count|count.checked_add(moved)).ok_or("DOM bitmap movement overflow.")?;
            used=used.checked_add(bytes).ok_or("DOM bitmap backing overflow.")?;growth.words[slot]=next;
        }
        Ok((growth,used,records,records.checked_mul(size_of::<u64>()).ok_or("DOM bitmap copy overflow.")?))
    }
    pub(super) fn commit_node_sets(&mut self,growth:Growth){
        let[a,b,c,d,e]=growth.words;
        self.dom.created.commit(a,growth.required);self.detached_nodes.commit(b,growth.required);
        self.detached_images.commit(c,growth.required);self.dom.fragment_scripts.commit(d,growth.required);self.dom.force_async.commit(e,growth.required);
    }
    pub(super) fn ensure_node_sets(&mut self,nodes:usize)->Result<(),String>{
        let required=words(nodes)?;
        if self.node_sets().iter().all(|set|set.words.len()>=required){return Ok(());}
        let before=self.node_sets_storage_bytes();
        let(growth,bytes,work,copies)=self.prepare_node_sets(nodes,self.retained_limit.saturating_sub(self.retained))?;
        self.work(work)?;self.copies(copies)?;
        self.retained=self.retained.checked_add(bytes).ok_or("DOM bitmap retained accounting overflow.")?;
        self.commit_node_sets(growth);debug_assert_eq!(before+bytes,self.node_sets_storage_bytes());Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn node_identity_bitmap_unit_keeps_word_boundaries_counts_and_empty_scans() {
        let mut set=NodeSet::default();let mut backing=Vec::new();backing.try_reserve_exact(3).unwrap();set.commit(Some(backing),3);
        assert!(set.is_empty());assert_eq!(set.scan_work(),0);assert_eq!(set.iter().next(),None);
        for node in [129,0,64,63]{assert!(set.insert(node));assert!(!set.insert(node));}
        assert_eq!(set.len(),4);assert_eq!(set.iter().collect::<Vec<_>>(),[0,63,64,129]);assert_eq!(set.scan_work(),7);
        assert!(!set.contains(&128));assert!(!set.contains(&usize::MAX));assert!(!set.remove(&usize::MAX));
        assert!(set.remove(&63));assert!(!set.remove(&63));assert_eq!(set.iter().collect::<Vec<_>>(),[0,64,129]);
        for node in [0,64,129]{assert!(set.remove(&node));}
        assert_eq!(set.scan_work(),0);assert!(set.is_empty());assert_eq!(set.storage_bytes(),3*size_of::<u64>());
    }
}
