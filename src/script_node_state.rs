//! Joint unpublished node-record and per-node state backing admission.
use super::*;

#[derive(Default)]
pub(super) struct Growth {
    nodes: Option<Vec<Node>>,
    active: Option<Vec<bool>>,
    parents: Option<Vec<Option<usize>>>,
    connected: Option<Vec<bool>>,
    siblings: Option<Vec<usize>>,
    required: usize,
}

pub(super) struct Rebuilt {
    pub(super) active: Vec<bool>,
    pub(super) parents: Vec<Option<usize>>,
    pub(super) connected: Vec<bool>,
    pub(super) siblings: Vec<usize>,
}
impl Rebuilt {
    pub(super) fn bytes(&self) -> usize {
        self.active.capacity()+self.connected.capacity()+self.parents.capacity()*size_of::<Option<usize>>()+self.siblings.capacity()*size_of::<usize>()
    }
}
fn minimum<T>(values:&Vec<T>,required:usize)->Result<usize,String> {
    required.saturating_sub(values.capacity()).checked_mul(size_of::<T>()).ok_or("DOM node-state backing overflow.".into())
}
fn plan<T>(values:&Vec<T>,required:usize,available:usize)->Result<(Option<Vec<T>>,usize),String> {
    let usable=(NODE_HANDLES as usize).saturating_sub(values.capacity()).checked_mul(size_of::<T>())
        .ok_or("DOM node-state representation backing overflow.")?;
    retained_storage::grow(values,required.saturating_sub(values.len()),available.min(usable))
}
fn commit<T>(values:&mut Vec<T>,growth:Option<Vec<T>>) {
    if let Some(mut next)=growth {next.append(values);*values=next;}
}
fn movement<T>(values:&Vec<T>,growth:&Option<Vec<T>>,added:usize)->Result<(usize,usize),String> {
    let records=added.checked_add(if growth.is_some(){values.len()}else{0}).ok_or("DOM node-state movement overflow.")?;
    Ok((records,records.checked_mul(size_of::<T>()).ok_or("DOM node-state copy work overflow.")?))
}

impl Bridge<'_> {
    pub(super) fn main_node_minimum(&self,count:usize)->Result<usize,String> {
        if count==0 {return Ok(0);}
        let required=self.doc.nodes.len().checked_add(count).ok_or("DOM node count overflow.")?;
        minimum(&self.doc.nodes,required)?.checked_add(minimum(&self.state.tree.active,required)?)
            .and_then(|bytes|bytes.checked_add(minimum(&self.state.tree.parents,required).ok()?))
            .and_then(|bytes|bytes.checked_add(minimum(&self.state.dom.connected,required).ok()?))
            .and_then(|bytes|bytes.checked_add(minimum(&self.state.tree.sibling_slots,required).ok()?))
            .ok_or("Joint DOM node-state backing overflow.".into())
    }

    pub(super) fn prepare_main_nodes(&mut self,count:usize,available:usize)->Result<(Growth,usize),String> {
        if count==0 {return Ok((Growth::default(),0));}
        let required=self.doc.nodes.len().checked_add(count).filter(|&count|count<=NODE_HANDLES as usize)
            .ok_or("24-bit DOM node identity domain exhausted.")?;
        let active_min=minimum(&self.state.tree.active,required)?;
        let parent_min=minimum(&self.state.tree.parents,required)?;
        let connected_min=minimum(&self.state.dom.connected,required)?;
        let sibling_min=minimum(&self.state.tree.sibling_slots,required)?;
        let metadata_min=active_min.checked_add(parent_min).and_then(|bytes|bytes.checked_add(connected_min)).and_then(|bytes|bytes.checked_add(sibling_min))
            .ok_or("DOM node-state backing overflow.")?;
        let budget=available.checked_sub(metadata_min).ok_or("DOM retained joint node-state allowance exhausted.")?;
        let(nodes,node_bytes)=plan(&self.doc.nodes,required,budget)?;
        let budget=available.checked_sub(node_bytes).and_then(|bytes|bytes.checked_sub(parent_min))
            .and_then(|bytes|bytes.checked_sub(connected_min)).and_then(|bytes|bytes.checked_sub(sibling_min)).ok_or("DOM node-state active allowance exhausted.")?;
        let(active,active_bytes)=plan(&self.state.tree.active,required,budget)?;
        let budget=available.checked_sub(node_bytes).and_then(|bytes|bytes.checked_sub(active_bytes))
            .and_then(|bytes|bytes.checked_sub(connected_min)).and_then(|bytes|bytes.checked_sub(sibling_min)).ok_or("DOM node-state parent allowance exhausted.")?;
        let(parents,parent_bytes)=plan(&self.state.tree.parents,required,budget)?;
        let used=node_bytes.checked_add(active_bytes).and_then(|bytes|bytes.checked_add(parent_bytes))
            .ok_or("DOM node-state backing overflow.")?;
        let(connected,connected_bytes)=plan(&self.state.dom.connected,required,available.checked_sub(used).and_then(|bytes|bytes.checked_sub(sibling_min)).ok_or("DOM connection-state allowance exhausted.")?)?;
        let used=used.checked_add(connected_bytes).ok_or("DOM node-state backing overflow.")?;
        let(siblings,sibling_bytes)=plan(&self.state.tree.sibling_slots,required,available.checked_sub(used).ok_or("DOM sibling-state allowance exhausted.")?)?;
        let mut records=0usize;let mut copies=0usize;
        for (work,bytes) in [movement(&self.doc.nodes,&nodes,count)?,movement(&self.state.tree.active,&active,count)?,
            movement(&self.state.tree.parents,&parents,count)?,movement(&self.state.dom.connected,&connected,count)?,
            movement(&self.state.tree.sibling_slots,&siblings,count)?] {
            records=records.checked_add(work).ok_or("DOM node-state movement overflow.")?;
            copies=copies.checked_add(bytes).ok_or("DOM node-state copy overflow.")?;
        }
        self.state.work(records)?;self.state.copies(copies)?;
        Ok((Growth{nodes,active,parents,connected,siblings,required},used.checked_add(sibling_bytes).ok_or("DOM node-state backing overflow.")?))
    }

    pub(super) fn commit_main_nodes(&mut self,growth:Growth) {
        commit(&mut self.doc.nodes,growth.nodes);
        commit(&mut self.state.tree.active,growth.active);
        commit(&mut self.state.tree.parents,growth.parents);
        commit(&mut self.state.dom.connected,growth.connected);
        commit(&mut self.state.tree.sibling_slots,growth.siblings);
        if growth.required>self.state.tree.sibling_slots.len() {self.state.tree.sibling_slots.resize(growth.required,usize::MAX);}
    }
}

impl State {
    pub(super) fn node_state_bytes(&self)->usize {
        self.tree.active.capacity()+self.dom.connected.capacity()+self.tree.parents.capacity()*size_of::<Option<usize>>()+self.tree.sibling_slots.capacity()*size_of::<usize>()
    }
    pub(super) fn tree_replacement_available(&self)->Result<usize,String> {
        let old=self.node_state_bytes().checked_add(self.tree.ids.capacity()*size_of::<(usize,usize)>())
            .ok_or("DOM tree backing overflow.")?;
        self.retained_limit.checked_add(old).and_then(|limit|limit.checked_sub(self.retained))
            .ok_or("DOM retained tree replacement allowance exhausted.".into())
    }
    pub(super) fn prepare_rebuilt_states(&mut self,count:usize)->Result<Rebuilt,String> {
        let available=self.tree_replacement_available()?;
        let parents_min=count.checked_mul(size_of::<Option<usize>>()).ok_or("DOM parent-state backing overflow.")?;
        let sibling_min=count.checked_mul(size_of::<usize>()).ok_or("DOM sibling backing overflow.")?;
        let minimum=parents_min.checked_add(sibling_min).and_then(|bytes|bytes.checked_add(count)).and_then(|bytes|bytes.checked_add(count)).ok_or("DOM node-state backing overflow.")?;
        if minimum>available {return Err("DOM retained rebuilt node-state allowance exhausted.".into());}
        let(active,active_bytes)=retained_storage::capacity(&Vec::<bool>::new(),count,available-parents_min-count-sibling_min)?;
        let(parents,parent_bytes)=retained_storage::capacity(&Vec::<Option<usize>>::new(),count,available-active_bytes-count-sibling_min)?;
        let(connected,connected_bytes)=retained_storage::capacity(&Vec::<bool>::new(),count,available-active_bytes-parent_bytes-sibling_min)?;
        let(siblings,_)=retained_storage::capacity(&Vec::<usize>::new(),count,available-active_bytes-parent_bytes-connected_bytes)?;
        let mut result=Rebuilt{active:active.unwrap_or_default(),parents:parents.unwrap_or_default(),connected:connected.unwrap_or_default(),siblings:siblings.unwrap_or_default()};
        self.work(count.checked_mul(4).ok_or("DOM node-state initialization overflow.")?)?;
        self.copies(minimum)?;
        result.active.resize(count,false);result.parents.resize(count,None);result.connected.resize(count,false);result.siblings.resize(count,usize::MAX);
        Ok(result)
    }
}
