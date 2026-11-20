//! Original deadline heap. Equal deadlines retain creation
//! order through a separate compactable scheduling sequence; payload is never copied during heap maintenance or cancellation.
use super::Timer;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Queue { entries: Vec<Timer>, next_order: usize }
impl Queue {
    pub fn len(&self)->usize { self.entries.len() }
    #[cfg(test)]
    pub fn is_empty(&self)->bool { self.entries.is_empty() }
    pub fn iter(&self)->std::slice::Iter<'_,Timer> { self.entries.iter() }
    pub fn deadline(&self)->Option<Instant> { self.entries.first().map(|timer|timer.due) }
    pub fn storage_bytes(&self)->usize { self.entries.capacity()*std::mem::size_of::<Timer>() }
    pub fn maintenance_work(&self)->usize { 3*(usize::BITS-self.entries.len().leading_zeros()) as usize+1 }
    pub fn order_work(&self)->usize {
        if self.next_order==usize::MAX && !self.entries.is_empty() {
            self.len().saturating_mul((usize::BITS-self.len().leading_zeros()) as usize+1)
        } else {0}
    }
    pub fn prepare_order(&self,available:usize)->Result<Option<Vec<usize>>,String> {
        if self.order_work()==0{return Ok(None);}
        let slot=std::mem::size_of::<usize>();
        if self.len()>available/slot{return Err("DOM retained timer order scratch allowance exhausted.".into());}
        let mut indices=Vec::new();
        indices.try_reserve_exact(self.len()).map_err(|_|"Could not reserve timer scheduling order scratch.")?;
        if indices.capacity()>available/slot{return Err("DOM retained timer order scratch allocation exceeds allowance.".into());}
        indices.extend(0..self.len());
        indices.sort_unstable_by_key(|index|self.entries[*index].order);
        Ok(Some(indices))
    }
    pub fn commit_order(&mut self,indices:Option<Vec<usize>>) {
        if let Some(indices)=indices {
            self.next_order=indices.len();
            for (order,index) in indices.into_iter().enumerate(){self.entries[index].order=order;}
        }
    }
    pub fn prepare_growth(&self,available:usize)->Result<(Option<Vec<Timer>>,usize),String> {
        let before=self.entries.capacity();
        if self.entries.len()<before {return Ok((None,0));}
        let slot=std::mem::size_of::<Timer>();
        let growth=before.max(4).min(available/slot);
        if growth==0 {return Err("DOM retained timer storage allowance exhausted.".into());}
        let capacity=before.checked_add(growth).ok_or("Timer queue capacity overflow.")?;
        let mut next=Vec::new();
        next.try_reserve_exact(capacity).map_err(|_|"Could not reserve timer queue storage.")?;
        let bytes=next.capacity().checked_sub(before).and_then(|slots|slots.checked_mul(slot))
            .filter(|bytes|*bytes<=available).ok_or("DOM retained timer allocation exceeds allowance.")?;
        Ok((Some(next),bytes))
    }
    pub fn commit_growth(&mut self,next:Option<Vec<Timer>>) {
        if let Some(mut next)=next {next.append(&mut self.entries);self.entries=next;}
    }
    fn before(&self,a:usize,b:usize)->bool {
        (self.entries[a].due,self.entries[a].order)<(self.entries[b].due,self.entries[b].order)
    }
    fn sift_up(&mut self,mut index:usize) {
        while index>0 {
            let parent=(index-1)/2;
            if !self.before(index,parent){break;}
            self.entries.swap(index,parent);index=parent;
        }
    }
    fn sift_down(&mut self,mut index:usize) {
        while let Some(left)=index.checked_mul(2).and_then(|index|index.checked_add(1)).filter(|left|*left<self.len()) {
            let right=left+1;
            let next=if right<self.len()&&self.before(right,left){right}else{left};
            if !self.before(next,index){break;}
            self.entries.swap(index,next);index=next;
        }
    }
    pub fn push_reserved(&mut self,mut timer:Timer) {
        if self.entries.is_empty(){self.next_order=0;}
        timer.order=self.next_order;
        self.next_order=self.next_order.checked_add(1).expect("prepared timer scheduling order");
        debug_assert!(self.len()<self.entries.capacity());
        self.entries.push(timer);self.sift_up(self.len()-1);
    }
    pub fn remove(&mut self,index:usize)->Timer {
        let timer=self.entries.swap_remove(index);
        if index<self.len() {
            if index>0&&self.before(index,(index-1)/2){self.sift_up(index);}else{self.sift_down(index);}
        }
        timer
    }
}
#[cfg(test)]
impl std::ops::Index<usize> for Queue {
    type Output=Timer;
    fn index(&self,index:usize)->&Timer {&self.entries[index]}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timer_heap_deadlines_ties_and_interior_removal_match_independent_order() {
        let base=Instant::now();
        let mut queue=Queue::default();
        let mut reference=Vec::new();
        for id in 1..=2048u32 {
            let due=base+std::time::Duration::from_millis((id*37%127) as u64);
            let (growth,_)=queue.prepare_growth(usize::MAX).unwrap();queue.commit_growth(growth);
            queue.push_reserved(Timer{id,due,order:0,callback:crate::javascript::Value::Undefined,arguments:Vec::new(),bytes:0,nesting:0});
            reference.push((due,id));
        }
        for id in [3,16,2048,1025] {
            let index=queue.iter().position(|timer|timer.id==id).unwrap();
            assert_eq!(queue.remove(index).id,id);
            reference.retain(|(_,candidate)|*candidate!=id);
        }
        reference.sort();
        for expected in reference {
            assert_eq!(queue.deadline(),Some(expected.0));
            let timer=queue.remove(0);assert_eq!((timer.due,timer.id),expected);
        }
        assert!(queue.is_empty());
        assert_eq!(queue.deadline(),None);
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    #[test]
    fn timer_heap_reused_ids_and_order_compaction_preserve_equal_deadline_creation_order() {
        let due=Instant::now();
        let mut queue=Queue::default();
        for id in [8,3,12] {
            let (growth,_)=queue.prepare_growth(usize::MAX).unwrap();queue.commit_growth(growth);
            queue.push_reserved(Timer{id,due,order:0,callback:crate::javascript::Value::Undefined,arguments:Vec::new(),bytes:0,nesting:0});
        }
        queue.next_order=usize::MAX;
        assert!(queue.order_work()>queue.len());
        assert!(queue.prepare_order(0).is_err());
        assert_eq!(queue.next_order,usize::MAX);
        let order=queue.prepare_order(usize::MAX).unwrap();queue.commit_order(order);
        queue.push_reserved(Timer{id:2,due,order:0,callback:crate::javascript::Value::Undefined,arguments:Vec::new(),bytes:0,nesting:0});
        for expected in [8,3,12,2] {assert_eq!(queue.remove(0).id,expected);}
        assert!(queue.is_empty());
    }
}
