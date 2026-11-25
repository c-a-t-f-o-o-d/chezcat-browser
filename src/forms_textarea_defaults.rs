//! Prepared scalar TEXTAREA defaults, published only with checked child text.
use super::*;
// Native defaults/editors retain the separate finite Forms payload bound.
// bytes admits their peak scratch with the DOM transaction; no uncharged
// native storage is refunded from the DOM retained-byte ledger.
pub(crate) struct TextareaDefaultPlan { node:usize, initial:String, value:Option<Editor>, pub(crate) bytes:usize }
impl TextareaDefaultPlan {
 pub(crate) fn commit(self,forms:&mut Forms){let c=forms.controls.get_mut(&self.node).expect("Prepared TEXTAREA identity");debug_assert_eq!(c.kind,Kind::TextArea);c.initial=self.initial;if let Some(v)=self.value{c.value=v;}}
}
impl Forms {
 pub(crate) fn prepare_textarea_default(&self,node:usize,text:&str,available:usize)->Result<TextareaDefaultPlan,String>{
  if self.limited||self.controls.len()>MAX_CONTROLS{return Err("Native form limits already exceeded; textarea default was not changed.".into());}
  let c=self.controls.get(&node).filter(|c|c.kind==Kind::TextArea).ok_or("Default update requires a native TEXTAREA.")?;
  let mut units=0usize;let mut count=0usize;let mut length=0usize;
  for ch in normalized_chars(text){units=units.checked_add(ch.len_utf16()).ok_or("Textarea default length overflow.")?;if units>MAX_VALUE_UNITS{return Err("A textarea default is limited to 4096 normalized UTF-16 units.".into());}count+=1;length+=ch.len_utf8();}
  let mut total=0usize;
  for (&other,item) in &self.controls {
   if item.name.len()>MAX_QUERY{return Err("A form control name exceeds 8192 bytes; textarea default was not changed.".into());}
   let live=if other==node&&!c.dirty_value{length}else{item.value.chars.iter().map(|ch|ch.len_utf8()).sum()};
   let default=if other==node{length}else{item.initial.len()};
   total=total.checked_add(item.name.len()).and_then(|n|n.checked_add(live.max(default))).ok_or("Textarea default payload overflow.")?;
   if let Some(select)=&item.select{for option in &select.options{total=total.checked_add(option.value.len()).and_then(|n|n.checked_add(option.label.len())).ok_or("Textarea default option payload overflow.")?;}for row in &select.rows{if let SelectRow::Group{label,..}=row{total=total.checked_add(label.len()).ok_or("Textarea default group payload overflow.")?;}}}
   if total>MAX_TOTAL_VALUE_BYTES{return Err("Combined form names, values and defaults exceed 128 KiB; no textarea default was changed.".into());}
  }
  let replace=!c.dirty_value&&!c.value.chars.iter().copied().eq(normalized_chars(text));
  let editor_bytes=if replace{count.checked_mul(std::mem::size_of::<char>()).ok_or("Textarea default editor overflow.")?}else{0};
  if length.checked_add(editor_bytes).is_none_or(|n|n>available){return Err("Textarea default scratch allowance exhausted.".into());}
  let mut initial=String::new();initial.try_reserve_exact(length).map_err(|_|"Cannot reserve textarea reset default.")?;initial.extend(normalized_chars(text));
  let mut bytes=initial.capacity();if bytes.checked_add(editor_bytes).is_none_or(|n|n>available){return Err("Textarea default scratch allowance exhausted.".into());}
  let value=if replace{let mut chars=Vec::new();chars.try_reserve_exact(count).map_err(|_|"Cannot reserve textarea default editor.")?;bytes=bytes.checked_add(chars.capacity().checked_mul(std::mem::size_of::<char>()).ok_or("Textarea default editor overflow.")?).ok_or("Textarea default storage overflow.")?;if bytes>available{return Err("Textarea default scratch allowance exhausted.".into());}chars.extend(normalized_chars(text));Some(Editor{chars,cursor:count,anchor:count,offset:0})}else{None};
  Ok(TextareaDefaultPlan{node,initial,value,bytes})
 }
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn textarea_default_value_native_plan_normalized_admission_commit_and_dirty_selection(){let d=crate::html::parse("<form><textarea id=area>seed</textarea><input value=other></form>");let mut f=Forms::new(&d);let n=(0..d.nodes.len()).find(|n|d.attr(*n,"id")==Some("area")).unwrap();let initial=f.controls[&n].initial.clone();let plan=f.prepare_textarea_default(n,"A\r\n😀\rZ",usize::MAX).unwrap();assert_eq!(f.controls[&n].initial,initial);assert_eq!(f.controls[&n].value.text(),"seed");plan.commit(&mut f);assert_eq!(f.controls[&n].initial,"A\n😀\nZ");assert_eq!(f.controls[&n].value.text(),"A\n😀\nZ");assert!(!f.controls[&n].dirty_value);f.script_set_value(n,"dirty").unwrap();{let e=&mut f.controls.get_mut(&n).unwrap().value;e.cursor=3;e.anchor=1;e.offset=2;}f.prepare_textarea_default(n,"next\r\n",usize::MAX).unwrap().commit(&mut f);let c=&f.controls[&n];assert_eq!(c.initial,"next\n");assert_eq!((c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value),("dirty".into(),3,1,2,true));assert!(f.prepare_text_default(n,Some("input only")).is_err());}
 #[test]
 fn textarea_default_value_native_plan_counts_select_payload_and_retained_defaults(){let mut html="<form><textarea id=target></textarea>".to_string();for _ in 0..30{html.push_str(&format!("<textarea>{}</textarea>","a".repeat(4096)));}html.push_str(&format!("<select><option value='{}'>L</option></select></form>","b".repeat(3900)));let d=crate::html::parse(&html);let mut f=Forms::new(&d);assert!(!f.limited);let n=(0..d.nodes.len()).find(|n|d.attr(*n,"id")==Some("target")).unwrap();let before=f.controls.iter().map(|(n,c)|(*n,c.initial.clone(),c.value.text())).collect::<Vec<_>>();assert!(f.prepare_textarea_default(n,&"z".repeat(500),usize::MAX).is_err());assert_eq!(f.controls.iter().map(|(n,c)|(*n,c.initial.clone(),c.value.text())).collect::<Vec<_>>(),before);let other=*f.controls.iter().find(|(node,c)|**node!=n&&c.kind==Kind::TextArea).unwrap().0;f.script_set_value(other,"").unwrap();assert!(f.prepare_textarea_default(n,&"z".repeat(500),usize::MAX).is_err(),"dirty live text cannot hide retained reset default");assert!(f.prepare_textarea_default(n,"new",0).is_err());assert!(f.prepare_textarea_default(n,&"😀".repeat(2049),usize::MAX).is_err());assert!(f.prepare_textarea_default(usize::MAX,"",usize::MAX).is_err());f.limited=true;assert!(f.prepare_textarea_default(n,"",usize::MAX).is_err());}
}
