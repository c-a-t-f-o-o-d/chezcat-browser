//! Required-field candidates shared by static and native interactive validation.
use super::*;
/// Eligibility is metadata, not a claim that every constraint is implemented.
/// Callers supply actual tree context and the native disabled/first-legend rule.
pub(crate) fn validation_candidate(doc:&Document,node:usize,disabled:bool,in_datalist:bool,parent_select:bool)->bool{
    if disabled||in_datalist||doc.nodes.get(node).is_none_or(|n|n.namespace!=crate::html::Namespace::Html){return false;}
    let NodeKind::Element{attrs,..}=&doc.nodes[node].kind else{return false};
    validation_candidate_attributes(tag(doc,node),attrs,disabled,in_datalist,parent_select)
}
pub(crate) fn validation_candidate_attributes(name:&str,attrs:&[crate::html::Attribute],disabled:bool,in_datalist:bool,parent_select:bool)->bool{
    if disabled||in_datalist{return false;}
    let attr=|key:&str|attrs.iter().find(|a|a.name.html_is(key)).map(|a|a.value.as_str());
    let state=attr("type").unwrap_or("");
    match name{
        "input"=>{
            if ["hidden","reset","button"].iter().any(|s|state.eq_ignore_ascii_case(s)){return false;}
            let readonly_applies=!["checkbox","radio","file","range","color","image","submit"].iter().any(|s|state.eq_ignore_ascii_case(s));
            !readonly_applies||attr("readonly").is_none()
        }
        "button"=>{
            if state.eq_ignore_ascii_case("submit"){true}
            else if state.eq_ignore_ascii_case("reset")||state.eq_ignore_ascii_case("button"){false}
            else{!parent_select&&attr("command").is_none()&&attr("commandfor").is_none()}
        }
        "select"=>true,
        "textarea"=>attr("readonly").is_none(),
        _=>false,
    }
}
/// Charge repeated exact attribute-name and state comparisons without copying
/// authored values or allocating a lowercased INPUT/BUTTON state string.
pub(crate) fn validation_attribute_work(doc:&Document,node:usize)->Result<usize,String>{
    let NodeKind::Element{attrs,..}=&doc.nodes.get(node).ok_or("Candidate node is unavailable.")?.kind else{return Err("Candidate is not an element.".into())};
    attrs.iter().try_fold(1usize,|total,a|a.name.bytes().checked_add(a.value.logical_bytes()).and_then(|n|n.checked_add(1)).and_then(|n|n.checked_mul(8)).and_then(|n|total.checked_add(n)).ok_or("Candidate attribute work overflow.".into()))
}
pub(crate) struct ValidationSnapshot{
    pub(crate) nodes:Vec<usize>,
    pub(crate) peak_bytes:usize,
}
impl ValidationSnapshot{
    pub(crate) fn candidate(node:usize,missing:bool,mut available:usize)->Result<Self,String>{
        let mut nodes=Vec::new();let peak_bytes=reserve(&mut nodes,usize::from(missing),&mut available)?;
        if missing{nodes.push(node);}
        Ok(Self{nodes,peak_bytes})
    }
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
    pub(crate) fn validation_metadata_work(&self,doc:&Document)->Result<usize,String>{
        self.controls.keys().try_fold(0usize,|total,&node|validation_attribute_work(doc,node)?.checked_add(self.length_validation_work(doc,node)).and_then(|work|total.checked_add(work)).ok_or("Validation metadata work overflow.".into()))
    }
    pub(super) fn validation_enabled(&self,doc:&Document,owner:usize,submitter:Option<usize>)->Result<bool,String>{
        if owner>=doc.nodes.len()||doc.nodes[owner].namespace!=crate::html::Namespace::Html||tag(doc,owner)!="form"{return Err("Submission requires an existing form.".into());}
        if self.limited{return Err("This page exceeds the form value/control limits; nothing was submitted.".into());}
        if submitter.is_some_and(|node|!self.controls.get(&node).is_some_and(|c|c.owner==Some(owner)&&matches!(c.kind,Kind::Submit|Kind::Image)&&!c.disabled)){
            return Err("The submit button is not available.".into());
        }
        Ok(doc.attr(owner,"novalidate").is_none()&&!submitter.is_some_and(|node|doc.attr(node,"formnovalidate").is_some()))
    }
    pub(super) fn required_value_missing(&self,owner:usize,control:&Control)->bool{
        if control.owner!=Some(owner){return false;}
        self.control_value_missing(control)
    }
    pub(crate) fn control_value_missing(&self,control:&Control)->bool{
        if control.disabled{return false;}
        self.constraint_value_missing(control)
    }
    /// Constraint flags remain independent of candidate eligibility. SELECT,
    /// checkbox and radio missingness does not depend on being mutable.
    pub(crate) fn constraint_value_missing(&self,control:&Control)->bool{
        let owner=control.owner;
        if matches!(control.kind,Kind::Text|Kind::TextArea)&&(control.disabled||control.readonly){return false;}
        let required=control.required||control.kind==Kind::Radio&&!control.name.is_empty()&&self.controls.values().any(|c|c.owner==owner&&c.group_root==control.group_root&&c.kind==Kind::Radio&&c.name==control.name&&c.required);
        if !required{return false;}
        match control.kind{
            Kind::Text|Kind::TextArea=>control.value.chars.is_empty(),
            Kind::Select=>control.select.as_ref().and_then(|state|state.selected.and_then(|index|state.options.get(index))).is_none_or(|option|option.placeholder),
            Kind::Checkbox=>!control.checked,
            Kind::Radio if control.name.is_empty()=>!control.checked,
            Kind::Radio=>!self.controls.values().any(|c|c.owner==owner&&c.group_root==control.group_root&&c.kind==Kind::Radio&&c.name==control.name&&c.checked),
            _=>false,
        }
    }
    pub(crate) fn required_message(&self,node:usize)->String{
        format!("Complete the required field: {}.",self.controls.get(&node).map(|c|c.name.as_str()).filter(|s|!s.is_empty()).unwrap_or("unnamed field"))
    }
    /// Snapshot actual connected tree order before the first invalid listener.
    /// All scratch and result capacities coexist inside the caller's allowance.
    pub(crate) fn validation_snapshot(&self,doc:&Document,owner:usize,submitter:Option<usize>,available:usize)->Result<ValidationSnapshot,String>{
        self.validation_snapshot_with(doc,owner,submitter,available,&mut |_|false)
    }
    pub(crate) fn validation_snapshot_with(&self,doc:&Document,owner:usize,submitter:Option<usize>,available:usize,custom:&mut impl FnMut(usize)->bool)->Result<ValidationSnapshot,String>{
        let mut work=crate::javascript::limits::BROWSER.regexp_work;let mut copies=usize::MAX;
        self.validation_snapshot_with_budget(doc,owner,submitter,available,custom,&mut work,&mut copies)
    }
    pub(crate) fn validation_snapshot_with_budget(&self,doc:&Document,owner:usize,submitter:Option<usize>,available:usize,custom:&mut impl FnMut(usize)->bool,work:&mut usize,copies:&mut usize)->Result<ValidationSnapshot,String>{
        if !self.validation_enabled(doc,owner,submitter)?{return Ok(ValidationSnapshot{nodes:Vec::new(),peak_bytes:0});}
        self.required_snapshot_with_budget(doc,owner,0,available,custom,work,copies)
    }
    /// Explicit validation methods ignore novalidate and use the form's actual
    /// tree root, including a retained detached ordinary native form.
    pub(crate) fn required_snapshot(&self,doc:&Document,owner:usize,root:usize,available:usize)->Result<ValidationSnapshot,String>{
        self.required_snapshot_with(doc,owner,root,available,&mut |_|false)
    }
    pub(crate) fn required_snapshot_with(&self,doc:&Document,owner:usize,root:usize,available:usize,custom:&mut impl FnMut(usize)->bool)->Result<ValidationSnapshot,String>{
        let mut work=crate::javascript::limits::BROWSER.regexp_work;let mut copies=usize::MAX;
        self.required_snapshot_with_budget(doc,owner,root,available,custom,&mut work,&mut copies)
    }
    pub(crate) fn required_snapshot_with_budget(&self,doc:&Document,owner:usize,root:usize,mut available:usize,custom:&mut impl FnMut(usize)->bool,work:&mut usize,copies:&mut usize)->Result<ValidationSnapshot,String>{
        self.validation_enabled(doc,owner,None)?;
        if root>=doc.nodes.len(){return Err("Validation tree root is unavailable.".into());}
        let mut nodes=Vec::new();let mut seen=Vec::new();let mut stack=Vec::new();
        let result_bytes=reserve(&mut nodes,self.controls.len(),&mut available)?;
        let seen_bytes=reserve(&mut seen,doc.nodes.len(),&mut available)?;
        let stack_bytes=reserve(&mut stack,doc.nodes.len(),&mut available)?;
        let peak_bytes=result_bytes.checked_add(seen_bytes).and_then(|n|n.checked_add(stack_bytes)).ok_or("Validation peak storage overflow.")?;
        seen.resize(doc.nodes.len(),false);stack.push((root,false,false));
        while let Some((node,in_datalist,parent_select))=stack.pop(){
            if node>=doc.nodes.len()||seen[node]{return Err("Invalid validation tree topology; nothing was submitted.".into());}seen[node]=true;
            if let Some(c)=self.controls.get(&node){
                if validation_candidate(doc,node,c.disabled,in_datalist,parent_select)&&c.owner==Some(owner)
                    &&(self.required_value_missing(owner,c)||{let(long,short)=self.length_validation_flags(doc,node);long||short}||custom(node)||self.pattern_mismatch(doc,node,work,copies,available)?){nodes.push(node);}
            }
            if stack.len().checked_add(doc.nodes[node].children.len()).is_none_or(|n|n>doc.nodes.len()){return Err("Invalid validation traversal size; nothing was submitted.".into());}
            let html=doc.nodes[node].namespace==crate::html::Namespace::Html;
            let barred=in_datalist||html&&tag(doc,node)=="datalist";
            let select=html&&tag(doc,node)=="select";
            stack.extend(doc.nodes[node].children.iter().rev().map(|&child|(child,barred,select)));
        }
        Ok(ValidationSnapshot{nodes,peak_bytes})
    }
}
