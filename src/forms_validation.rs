//! Required-field candidates shared by static and native interactive validation.
use super::*;
pub(crate) struct ValidationSnapshot{
    pub(crate) nodes:Vec<usize>,
    pub(crate) peak_bytes:usize,
}
impl ValidationSnapshot{
    pub(crate) fn storage_bytes(&self)->usize{self.nodes.capacity()*std::mem::size_of::<usize>()}
}
fn reserve<T>(values:&mut Vec<T>,count:usize,available:&mut usize)->Result<usize,String>{
    let slot=std::mem::size_of::<T>();
    let minimum=count.checked_mul(slot).ok_or("Validation storage overflow.")?;
    if minimum>*available{return Err("Validation snapshot storage allowance exhausted.".into());}
    values.try_reserve_exact(count).map_err(|_|"Validation snapshot allocation failed.")?;
    let bytes=values.capacity().checked_mul(slot).ok_or("Validation allocation overflow.")?;
    *available=available.checked_sub(bytes).ok_or("Validation allocation exceeds storage allowance.")?;
    Ok(bytes)
}
impl Forms{
    pub(super) fn validation_enabled(&self,doc:&Document,owner:usize,submitter:Option<usize>)->Result<bool,String>{
        if owner>=doc.nodes.len()||doc.nodes[owner].namespace!=crate::html::Namespace::Html||tag(doc,owner)!="form"{return Err("Submission requires an existing form.".into());}
        if self.limited{return Err("This page exceeds the form value/control limits; nothing was submitted.".into());}
        if submitter.is_some_and(|node|!self.controls.get(&node).is_some_and(|c|c.owner==Some(owner)&&matches!(c.kind,Kind::Submit|Kind::Image)&&!c.disabled)){
            return Err("The submit button is not available.".into());
        }
        Ok(doc.attr(owner,"novalidate").is_none()&&!submitter.is_some_and(|node|doc.attr(node,"formnovalidate").is_some()))
    }
    pub(super) fn required_value_missing(&self,owner:usize,control:&Control)->bool{
        if control.owner!=Some(owner)||control.disabled||matches!(control.kind,Kind::Text|Kind::TextArea)&&control.readonly{return false;}
        let required=control.required||control.kind==Kind::Radio&&!control.name.is_empty()&&self.controls.values().any(|c|c.owner==Some(owner)&&c.group_root==control.group_root&&c.kind==Kind::Radio&&c.name==control.name&&c.required);
        if !required{return false;}
        match control.kind{
            Kind::Text|Kind::TextArea=>control.value.chars.is_empty(),
            Kind::Select=>control.select.as_ref().and_then(|state|state.selected.and_then(|index|state.options.get(index))).is_none_or(|option|option.placeholder),
            Kind::Checkbox=>!control.checked,
            Kind::Radio if control.name.is_empty()=>!control.checked,
            Kind::Radio=>!self.controls.values().any(|c|c.owner==Some(owner)&&c.group_root==control.group_root&&c.kind==Kind::Radio&&c.name==control.name&&c.checked),
            _=>false,
        }
    }
    pub(crate) fn required_message(&self,node:usize)->String{
        format!("Complete the required field: {}.",self.controls.get(&node).map(|c|c.name.as_str()).filter(|s|!s.is_empty()).unwrap_or("unnamed field"))
    }
    /// Snapshot actual connected tree order before the first invalid listener.
    /// All scratch and result capacities coexist inside the caller's allowance.
    pub(crate) fn validation_snapshot(&self,doc:&Document,owner:usize,submitter:Option<usize>,mut available:usize)->Result<ValidationSnapshot,String>{
        if !self.validation_enabled(doc,owner,submitter)?{return Ok(ValidationSnapshot{nodes:Vec::new(),peak_bytes:0});}
        let mut nodes=Vec::new();let mut seen=Vec::new();let mut stack=Vec::new();
        let result_bytes=reserve(&mut nodes,self.controls.len(),&mut available)?;
        let seen_bytes=reserve(&mut seen,doc.nodes.len(),&mut available)?;
        let stack_bytes=reserve(&mut stack,doc.nodes.len(),&mut available)?;
        let peak_bytes=result_bytes.checked_add(seen_bytes).and_then(|n|n.checked_add(stack_bytes)).ok_or("Validation peak storage overflow.")?;
        seen.resize(doc.nodes.len(),false);if !doc.nodes.is_empty(){stack.push(0);}
        while let Some(node)=stack.pop(){
            if node>=doc.nodes.len()||seen[node]{return Err("Invalid validation tree topology; nothing was submitted.".into());}seen[node]=true;
            if self.controls.get(&node).is_some_and(|c|self.required_value_missing(owner,c)){nodes.push(node);}
            if stack.len().checked_add(doc.nodes[node].children.len()).is_none_or(|n|n>doc.nodes.len()){return Err("Invalid validation traversal size; nothing was submitted.".into());}
            stack.extend(doc.nodes[node].children.iter().rev().copied());
        }
        Ok(ValidationSnapshot{nodes,peak_bytes})
    }
}
