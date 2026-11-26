//! Checked, unpublished native reset replacements. Commit cannot allocate.
use super::*;

struct Update {
    node: usize,
    editor: Option<Editor>,
    selection: Option<Option<usize>>,
    caption: Option<String>,
    checked: bool,
}
pub(crate) struct ResetPlan {
    updates: Vec<Update>,
    bytes: usize,
}
fn reserve<T>(values:&mut Vec<T>,count:usize,available:usize)->Result<usize,String>{
    let minimum=count.checked_mul(std::mem::size_of::<T>()).ok_or("Form reset storage overflow.")?;
    if minimum>available{return Err("Form reset storage allowance exhausted.".into());}
    values.try_reserve_exact(count).map_err(|_|"Form reset allocation failed.")?;
    let actual=values.capacity().checked_mul(std::mem::size_of::<T>()).ok_or("Form reset allocation overflow.")?;
    if actual>available{return Err("Form reset allocation exceeds storage allowance.".into());}
    Ok(actual)
}
impl ResetPlan {
    pub(crate) fn bytes(&self)->usize{self.bytes}
    pub(crate) fn storage_bytes(&self)->Result<usize,String>{
        self.updates.capacity().checked_mul(std::mem::size_of::<Update>()).and_then(|n|n.checked_add(self.bytes)).ok_or_else(||"Form reset plan storage overflow.".into())
    }
    pub(crate) fn prepare(forms:&Forms,doc:&Document,owner:usize,available:usize)->Result<Self,String>{
        if forms.limited{return Err("Form reset requires completely admitted native controls.".into());}
        let count=forms.controls.values().filter(|c|c.owner==Some(owner)).count();
        let mut updates=Vec::new();
        let scratch=reserve(&mut updates,count,available)?;
        let mut remaining=available-scratch;
        let mut bytes=0usize;
        for (&node,c) in forms.controls.iter().filter(|(_,c)|c.owner==Some(owner)) {
            if c.kind==Kind::Unsupported{return Err("This INPUT state requires its reset lifecycle integration.".into());}
            let selected=c.select.as_ref().map(|s|s.initially_selected.or_else(||s.options.iter().position(|o|!o.disabled)));
            if let Some(selected)=selected{forms.check_select_selection_payload(node,selected)?;}
            let mut caption=None;
            let mut editor=None;
            if !c.button_element || selected.is_some(){
                let text=if let Some(selected)=selected{
                    selected.map_or("",|index|c.select.as_ref().unwrap().options[index].value.as_str())
                }else{c.initial.as_str()};
                // Existing defaults have already been admitted. Do not silently
                // truncate a reset or use the authored editing maxlength here.
                let raw=selected.is_some()||matches!(c.kind,Kind::Checkbox|Kind::Radio);
                let chars=||text.chars().count();
                let length=if raw{chars()}else{normalized_chars(text).filter(|ch|c.kind==Kind::TextArea||*ch!='\n').count()};
                let mut replacement=Editor::default();
                let used=reserve(&mut replacement.chars,length,remaining)?;remaining-=used;bytes+=used;
                if raw{replacement.chars.extend(text.chars());}
                else{replacement.chars.extend(normalized_chars(text).filter(|ch|c.kind==Kind::TextArea||*ch!='\n'));}
                replacement.cursor=replacement.chars.len();replacement.anchor=replacement.cursor;
                editor=Some(replacement);
            }
            if let Some(selected)=selected{
                let label=selected.map_or("",|index|c.select.as_ref().unwrap().options[index].label.as_str());
                if label.len()>remaining{return Err("Form reset caption storage allowance exhausted.".into());}
                let mut value=String::new();value.try_reserve_exact(label.len()).map_err(|_|"Form reset caption allocation failed.")?;
                if value.capacity()>remaining{return Err("Form reset caption allocation exceeds storage allowance.".into());}
                remaining-=value.capacity();bytes+=value.capacity();value.push_str(label);caption=Some(value);
            }
            updates.push(Update{node,editor,selection:selected,caption,checked:c.initially_checked});
        }
        if updates.iter().any(|u|forms.controls[&u.node].kind==Kind::Radio && u.checked){
            let mut roots=Vec::new();let root_bytes=reserve(&mut roots,updates.len(),remaining)?;remaining-=root_bytes;
            for u in &updates{
                let c=&forms.controls[&u.node];
                if c.kind==Kind::Radio&&u.checked&&!roots.contains(&c.group_root){roots.push(c.group_root);}
            }
            let mut seen=Vec::new();let used=reserve(&mut seen,doc.nodes.len(),remaining)?;remaining-=used;seen.resize(doc.nodes.len(),false);
            let mut stack=Vec::new();reserve(&mut stack,doc.nodes.len(),remaining)?;
            if roots.len()>doc.nodes.len(){return Err("Invalid form reset roots.".into());}
            stack.extend(roots.iter().rev().copied());
            // Each default-checked radio encountered in actual tree order
            // replaces the prior winner in its current root/name/owner group.
            while let Some(node)=stack.pop(){
                if node>=doc.nodes.len()||seen[node]{return Err("Invalid form reset topology.".into());}seen[node]=true;
                if stack.len().checked_add(doc.nodes[node].children.len()).is_none_or(|n|n>doc.nodes.len()){
                    return Err("Invalid form reset traversal size.".into());
                }
                stack.extend(doc.nodes[node].children.iter().rev().copied());
                if forms.controls.get(&node).is_some_and(|c|c.owner==Some(owner)&&c.kind==Kind::Radio&&c.initially_checked){
                    for u in &mut updates{if forms.same_radio_group(u.node,node){u.checked=u.node==node;}}
                }
            }
        }
        Ok(Self{updates,bytes})
    }
    pub(crate) fn commit(self,forms:&mut Forms){
        for u in self.updates{
            let c=forms.controls.get_mut(&u.node).unwrap();
            if let Some(editor)=u.editor{c.value=editor;}
            if let Some(caption)=u.caption{c.caption=caption;}
            if let Some(selected)=u.selection{
                let s=c.select.as_mut().unwrap();s.selected=selected;
                for o in &mut s.options{o.dirty_selected=false;}
            }
            if matches!(c.kind,Kind::Text|Kind::TextArea){c.dirty_value=false;c.last_value_change_user=false;}
            c.checked=u.checked;c.dirty_checked=false;
        }
    }
}
