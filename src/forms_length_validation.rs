//! Native text constraints share user-change state and original HTML prefixes.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#limiting-user-input-length:-the-maxlength-attribute
use super::*;
impl Forms{
    pub(crate) fn length_validation_work(&self,doc:&Document,node:usize)->usize{
        self.controls.get(&node).filter(|c|matches!(c.kind,Kind::Text|Kind::TextArea)&&c.dirty_value&&c.last_value_change_user
            &&(doc.attr(node,"maxlength").is_some()||doc.attr(node,"minlength").is_some())).map_or(0,|c|c.value.chars.len())
    }
    pub(crate) fn length_validation_flags(&self,doc:&Document,node:usize)->(bool,bool){
        let Some(c)=self.controls.get(&node).filter(|c|matches!(c.kind,Kind::Text|Kind::TextArea)&&c.dirty_value&&c.last_value_change_user)else{return(false,false)};
        let maximum=doc.attr(node,"maxlength");let minimum=doc.attr(node,"minlength");
        if maximum.is_none()&&minimum.is_none(){return(false,false);}
        // A real Vec<char> fits at most isize::MAX / size_of::<char>() slots;
        // its UTF-16 unit count and the next comparison bound fit in usize.
        let length=c.value.chars.iter().map(|ch|ch.len_utf16()).sum::<usize>();
        let bound=length+1;
        let maximum=maximum.and_then(|s|crate::decimal_number::html_nonnegative_bounded(s,bound));
        let minimum=minimum.and_then(|s|crate::decimal_number::html_nonnegative_bounded(s,bound));
        (maximum.is_some_and(|n|length>n),length!=0&&minimum.is_some_and(|n|length<n))
    }
    pub(crate) fn validation_message_parts<'a>(&'a self,doc:&Document,node:usize)->(&'static str,&'a str){
        let name=self.controls.get(&node).map(|c|c.name.as_str()).filter(|s|!s.is_empty()).unwrap_or("unnamed field");
        if self.controls.get(&node).is_none_or(|c|!matches!(c.kind,Kind::Text|Kind::TextArea)||self.control_value_missing(c)){return("Complete the required field: ",name);}
        let(long,short)=self.length_validation_flags(doc,node);
        (if long{"Shorten this field: "}else if short{"Lengthen this field: "}else{"Complete the required field: "},name)
    }
    pub(crate) fn validation_message(&self,doc:&Document,node:usize)->String{
        let(prefix,name)=self.validation_message_parts(doc,node);format!("{prefix}{name}.")
    }
}
