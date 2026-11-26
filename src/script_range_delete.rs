//! Selected deletion over the shared native Range topology and mutations.
//! https://dom.spec.whatwg.org/#dom-range-deletecontents
use super::*;
use crate::javascript::range_interface::DeleteStep;
impl Bridge<'_> {
    fn deletion_scratch(&mut self,plan:&Vec<DeleteStep>,pending:&Vec<usize>,extra:usize)->Result<(),String>{
        self.state.work(1)?;
        let bytes=plan.capacity().checked_mul(size_of::<DeleteStep>())
            .and_then(|bytes|pending.capacity().checked_mul(size_of::<usize>()).and_then(|n|bytes.checked_add(n)))
            .and_then(|bytes|bytes.checked_add(extra)).ok_or("Range deletion scratch overflow.")?;
        if bytes>self.state.retained_limit.saturating_sub(self.state.retained){return Err("Range deletion scratch allowance exhausted.".into());}
        Ok(())
    }
    fn deletion_push(&mut self,plan:&mut Vec<DeleteStep>,pending:&Vec<usize>,step:DeleteStep)->Result<(),String>{
        self.deletion_scratch(plan,pending,size_of::<DeleteStep>())?;
        plan.try_reserve_exact(1).map_err(|_|"Cannot reserve Range deletion steps.")?;
        self.deletion_scratch(plan,pending,0)?;plan.push(step);Ok(())
    }
    pub(in crate::scripting) fn range_delete_plan(&mut self,receiver:u32)->Result<Vec<DeleteStep>,Error>{
        use Error::{Exception,Fatal};
        let record=*self.range_record(receiver).filter(|record|!record.fixed).ok_or_else(||Exception("TypeError","A genuine live Range is required.".into()))?;
        let mut plan=Vec::new();let mut pending=Vec::new();
        if record.start==record.end{return Ok(plan);}
        let start=self.range_node(record.start.node).map_err(Fatal)?;
        let end=self.range_node(record.end.node).map_err(Fatal)?;
        let start_data=self.doc.nodes[start].kind.character_dom_string().is_some();
        let end_data=self.doc.nodes[end].kind.character_dom_string().is_some();
        if start==end&&start_data{
            self.deletion_push(&mut plan,&pending,DeleteStep::Data{node:record.start.node,offset:record.start.offset,count:record.end.offset-record.start.offset}).map_err(Fatal)?;
        }else{
            let mut common=record.start.node;
            while common!=record.end.node&&self.range_ancestor_child(common,record.end.node).map_err(Fatal)?.is_none(){
                common=self.range_parent(common).map_err(Fatal)?.ok_or_else(||Fatal("Range deletion has no common ancestor.".into()))?;
            }
            let collapsed=if common==record.start.node{record.start}else{
                let reference=self.range_ancestor_child(common,record.start.node).map_err(Fatal)?.ok_or_else(||Fatal("Range deletion start ancestry disappeared.".into()))?;
                let(parent,index)=self.range_index(reference).map_err(Fatal)?;Point{node:parent,offset:index+1}
            };
            self.deletion_push(&mut plan,&pending,DeleteStep::Collapse{node:collapsed.node,offset:collapsed.offset}).map_err(Fatal)?;
            if start_data{
                let length=self.range_length(record.start.node).map_err(Fatal)?;
                self.deletion_push(&mut plan,&pending,DeleteStep::Data{node:record.start.node,offset:record.start.offset,count:length-record.start.offset}).map_err(Fatal)?;
            }
            self.deletion_scratch(&plan,&pending,size_of::<usize>()).map_err(Fatal)?;
            pending.try_reserve_exact(1).map_err(|_|Fatal("Cannot reserve Range deletion traversal.".into()))?;
            pending.push(self.range_node(common).map_err(Fatal)?);
            while let Some(node)=pending.pop(){
                self.deletion_scratch(&plan,&pending,0).map_err(Fatal)?;
                let handle=if node==0{DOCUMENT}else{node_handle(node)};
                if let Some(parent_index)=self.state.tree.parents.get(node).copied().flatten(){
                    self.state.work(self.doc.nodes[parent_index].children.len()).map_err(Fatal)?;
                    let index=self.doc.nodes[parent_index].children.iter().position(|&child|child==node).ok_or_else(||Fatal("Range deletion topology is inconsistent.".into()))?;
                    let parent=if parent_index==0{DOCUMENT}else{node_handle(parent_index)};
                    let before=Point{node:parent,offset:index};let after=Point{node:parent,offset:index+1};
                    if matches!(self.range_compare(after,record.start).map_err(Fatal)?,Some(Ordering::Less|Ordering::Equal))
                        ||matches!(self.range_compare(before,record.end).map_err(Fatal)?,Some(Ordering::Greater|Ordering::Equal)){
                        continue;
                    }
                    if matches!(self.range_compare(record.start,before).map_err(Fatal)?,Some(Ordering::Less|Ordering::Equal))
                        &&matches!(self.range_compare(after,record.end).map_err(Fatal)?,Some(Ordering::Less|Ordering::Equal)){
                        if parent==DOCUMENT{return Err(Exception("NotSupportedError","Deletion of displayed Document children requires the broader Document mutation producer.".into()));}
                        self.deletion_push(&mut plan,&pending,DeleteStep::Remove{parent,node:handle}).map_err(Fatal)?;
                        continue;
                    }
                }
                let count=self.doc.nodes[node].children.len();
                self.state.work(count).map_err(Fatal)?;
                let bytes=count.checked_mul(size_of::<usize>()).ok_or_else(||Fatal("Range deletion traversal overflow.".into()))?;
                self.deletion_scratch(&plan,&pending,bytes).map_err(Fatal)?;
                pending.try_reserve_exact(count).map_err(|_|Fatal("Cannot reserve Range deletion children.".into()))?;
                pending.extend(self.doc.nodes[node].children.iter().rev().copied());
            }
            if end_data{
                self.deletion_push(&mut plan,&pending,DeleteStep::Data{node:record.end.node,offset:0,count:record.end.offset}).map_err(Fatal)?;
            }
        }
        self.deletion_scratch(&plan,&pending,0).map_err(Fatal)?;
        self.state.copies(plan.capacity().checked_mul(size_of::<DeleteStep>()).ok_or_else(||Fatal("Range deletion plan copy overflow.".into()))?).map_err(Fatal)?;
        let bytes=plan.capacity()*size_of::<DeleteStep>();
        self.state.retained+=bytes;
        Ok(plan)
    }
    pub(in crate::scripting) fn range_delete_collapse(&mut self,receiver:u32,node:u32,offset:usize)->Result<(),String>{
        self.range_node(node)?;
        if offset>self.range_length(node)?{return Err("Range deletion collapse exceeds the current node length.".into());}
        let index=receiver.checked_sub(BASE).ok_or("Range deletion receiver is invalid.")? as usize;
        let record=self.state.ranges.get_mut(index).filter(|record|!record.fixed).ok_or("Range deletion requires a live record.")?;
        record.start=Point{node,offset};record.end=record.start;Ok(())
    }
}
