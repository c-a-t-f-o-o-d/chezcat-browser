//! Exact read snapshots over the existing native token attribute record.
use super::*;
use crate::dom_string::DomStringRef;

pub(super) enum Read<'a>{Length,Index(usize,bool),Contains(DomStringRef<'a>)}
pub(super) fn argument(value:&Value)->Result<DomStringRef<'_>,String>{match value{
    Value::String(text)=>Ok(DomStringRef::Scalar(text)),Value::StringUnits(units)=>Ok(DomStringRef::Raw(units)),
    _=>Err("Token lookup requires an already converted DOMString.".into()),
}}

impl Bridge<'_>{
    pub(super) fn read_class_tokens(&mut self,node:usize,attribute:&str,read:Read<'_>)->Result<Value,String>{
        let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{return Err("Token list has no Element attribute source.".into());};
        read_attributes(self.state,attrs,attribute,read)
    }
}

pub(super) fn read_attributes(state:&mut State,attrs:&[crate::html::Attribute],attribute:&str,read:Read<'_>)->Result<Value,String>{
        state.work(attrs.len())?;state.copies(attrs.iter().map(|a|a.name.bytes()).sum())?;
        let source=attrs.iter().find(|a|a.name.html_is(attribute)).map(|a|a.value.view()).unwrap_or(DomStringRef::Scalar(""));
        let bytes=source.logical_bytes();if bytes>state.limits.host.string_bytes{return Err("Token attribute exceeds the host string bound.".into());}
        state.work(bytes)?;
        if let Read::Contains(needle)=read{
            state.work(bytes.saturating_add(needle.logical_bytes()))?;
            return Ok(Value::Bool(dom_collections::class_tokens(source).any(|token|token.same_units(needle))));
        }
        let count=dom_collections::class_tokens(source).count();
        let available=state.retained_limit.saturating_sub(state.retained);
        let required=count.checked_mul(size_of::<DomStringRef<'_>>()).ok_or("Token read snapshot storage overflow.")?;
        if required>available{return Err("Token read snapshot retained allowance exhausted.".into());}
        state.copies(required)?;
        let mut unique=Vec::new();unique.try_reserve_exact(count).map_err(|_|"Cannot reserve token read snapshot.")?;
        let storage=unique.capacity().checked_mul(size_of::<DomStringRef<'_>>()).filter(|&n|n<=available).ok_or("Token read snapshot returned allocation exceeds allowance.")?;
        for token in dom_collections::class_tokens(source){
            // Comparing the existing unique spellings may inspect every unit;
            // admit that work before exact comparison, without token copies.
            state.work(unique.len().saturating_add(1))?;
            let cost=unique.iter().fold(token.logical_bytes(),|n,prior:&DomStringRef<'_>|n.saturating_add(prior.logical_bytes()).saturating_add(1));
            state.work(cost)?;
            if !unique.iter().any(|prior|prior.same_units(token)){unique.push(token);}
        }
        match read{
            Read::Length=>Ok(Value::Number(unique.len()as f64)),
            Read::Index(index,undefined)=>{
                let Some(token)=unique.get(index)else{return Ok(if undefined{Value::Undefined}else{Value::Null});};
                state.work(token.logical_bytes())?;
                let bytes=token.copy_bytes()?;
                if bytes>available-storage{return Err("Token read output retained allowance exhausted.".into());}
                state.copies(bytes)?;
                let output=token.try_owned()?;
                if output.capacity()>available-storage{return Err("Token read output returned allocation exceeds allowance.".into());}
                Ok(Bridge::dom_string_owned(output))
            }
            Read::Contains(_)=>unreachable!(),
        }
}
