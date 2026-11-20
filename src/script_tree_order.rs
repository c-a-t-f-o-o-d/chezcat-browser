//! Paid document-order and reusable traversal storage; no recursive tree walk.
use super::*;
pub(super) type Visit=(usize,Option<usize>,bool,usize);
impl State {
    pub(super) fn order_bytes(&self)->usize {self.tree.order.capacity()*size_of::<usize>()}
    #[cfg(test)]
    pub(super) fn tree_storage_bytes(&self)->usize {
        self.node_state_bytes()+self.order_bytes()+self.tree.ids.capacity()*size_of::<(usize,usize)>()
            +self.tree_stack.capacity()*size_of::<Visit>()
    }
    pub(super) fn push_tree_order(&mut self,order:&mut Vec<usize>,node:usize,other:usize)->Result<(),String> {
        let owned=order.capacity().checked_mul(size_of::<usize>()).ok_or("DOM order backing overflow.")?;
        let available=self.tree_replacement_available()?.checked_sub(other).and_then(|bytes|bytes.checked_sub(owned))
            .ok_or("DOM retained document-order allowance exhausted.")?;
        let(growth,_)=retained_storage::grow(order,1,available)?;
        let records=1usize.checked_add(if growth.is_some(){order.len()}else{0}).ok_or("DOM order movement overflow.")?;
        self.work(records)?;self.copies(records.checked_mul(size_of::<usize>()).ok_or("DOM order copy overflow.")?)?;
        if let Some(mut next)=growth {next.append(order);*order=next;}
        order.push(node);Ok(())
    }
    pub(super) fn reserve_tree_stack(&mut self,stack:&mut Vec<Visit>,additional:usize,other:usize)->Result<(),String> {
        if additional==0 {return Ok(());}
        let available=self.tree_replacement_available()?.checked_sub(other).ok_or("DOM retained traversal backing allowance exhausted.")?;
        let(growth,bytes)=retained_storage::grow(stack,additional,available)?;
        let records=additional.checked_add(if growth.is_some(){stack.len()}else{0}).ok_or("DOM traversal movement overflow.")?;
        self.work(records)?;self.copies(records.checked_mul(size_of::<Visit>()).ok_or("DOM traversal copy overflow.")?)?;
        let retained=self.retained.checked_add(bytes).ok_or("DOM traversal retained accounting overflow.")?;
        if let Some(mut next)=growth {next.append(stack);*stack=next;}
        // Replacing a smaller tree can fund scratch growth. The wrapper keeps
        // paid reusable scratch on failure, or evicts it if speculative dropped-
        // tree credit would otherwise leave the old tree over its allowance.
        self.retained=retained;Ok(())
    }
}
