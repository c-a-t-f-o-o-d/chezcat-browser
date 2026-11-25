//! Submission-only wrapping at Unicode scalar columns; native API text is intact.
//! https://html.spec.whatwg.org/multipage/form-elements.html#textarea-wrapping-transformation
use super::*;
struct HardWrap<I:Iterator<Item=char>>{chars:std::iter::Peekable<I>,width:usize,column:usize}
impl<I:Iterator<Item=char>> Iterator for HardWrap<I>{
 type Item=char;
 fn next(&mut self)->Option<char>{
  let ch=*self.chars.peek()?;
  if matches!(ch,'\r'|'\n'){self.column=0;return self.chars.next();}
  if self.column==self.width{self.column=0;return Some('\n');}
  self.column+=1;self.chars.next()
 }
}
pub(super) fn append_textarea_entry(doc:&Document,node:usize,control:&Control,output:&mut String,limit:usize)->Result<(),String>{
 if !output.is_empty(){output.push('&');}encode_into(&control.name,output,limit)?;output.push('=');
 let chars=control.value.chars.iter().copied();
 if doc.attr(node,"wrap").is_some_and(|v|v.eq_ignore_ascii_case("hard")){
  let width=doc.attr(node,"cols").and_then(|v|crate::decimal_number::html_nonnegative_bounded(v,MAX_VALUE_UNITS+1)).filter(|v|*v>0).unwrap_or(20);
  encode_chars(HardWrap{chars:chars.peekable(),width,column:0},output,limit)
 }else{encode_chars(chars,output,limit)}
}
