//! Native DOMString storage. Raw UTF-16 is authoritative when unpaired units
//! exist; the scalar projection is only for existing display consumers.
//! DOM getters/copies/joins use code_units, never that display projection.
use std::fmt;
use std::ops::Deref;

#[derive(Debug,Default)]
pub(crate) struct DomString {
    projection:String,
    units:Option<Vec<u16>>,
}
pub(crate) enum Units<'a> {
    Scalar(std::str::EncodeUtf16<'a>),
    Raw(std::iter::Copied<std::slice::Iter<'a,u16>>),
}
impl Iterator for Units<'_> {
    type Item=u16;
    fn next(&mut self)->Option<u16>{match self {Self::Scalar(i)=>i.next(),Self::Raw(i)=>i.next()}}
    fn size_hint(&self)->(usize,Option<usize>){match self {Self::Scalar(i)=>i.size_hint(),Self::Raw(i)=>i.size_hint()}}
}
impl DomString {
    pub(crate) fn new()->Self{Self::default()}
    pub(crate) fn into_parts(self)->(String,Option<Vec<u16>>){(self.projection,self.units)}
    pub(crate) fn storage_bound(logical_bytes:usize)->Result<usize,String>{
        // One UTF-16 unit owns two bytes and needs at most three UTF-8
        // projection bytes; a valid surrogate pair uses fewer per unit.
        let raw=(logical_bytes/2).checked_mul(std::mem::size_of::<u16>()+3).ok_or("DOMString representation bound overflow.")?;
        Ok(raw.max(logical_bytes))
    }
    pub(crate) fn code_units(&self)->Units<'_>{
        match &self.units {Some(units)=>Units::Raw(units.iter().copied()),None=>Units::Scalar(self.projection.encode_utf16())}
    }
    pub(crate) fn raw_units(&self)->Option<&[u16]>{self.units.as_deref()}
    pub(crate) fn scalar(&self)->Result<&str,String>{
        if self.units.is_some(){Err("This scalar-only consumer cannot yet process unpaired native DOMString code units.".into())}
        else{Ok(&self.projection)}
    }
    pub(crate) fn render_projection(&self)->&str{&self.projection}
    pub(crate) fn append_parser_scalar(&mut self,text:&str)->Result<(),String>{
        if self.units.is_some(){return Err("Scalar parser append cannot replace stored DOMString code units.".into());}
        self.projection.try_reserve(text.len()).map_err(|_|"Cannot reserve parser character data.")?;
        self.projection.push_str(text);Ok(())
    }
    pub(crate) fn capacity(&self)->usize{self.projection.capacity()+self.units.as_ref().map_or(0,|u|u.capacity()*2)}
    pub(crate) fn used_bytes(&self)->usize{self.projection.len()+self.units.as_ref().map_or(0,|u|u.len()*2)}
    pub(crate) fn logical_bytes(&self)->usize{self.units.as_ref().map_or(self.projection.len(),|u|u.len()*2)}
    pub(crate) fn utf16_len(&self)->usize{self.units.as_ref().map_or_else(||self.projection.encode_utf16().count(),Vec::len)}
    pub(crate) fn encode_utf16(&self)->Units<'_>{self.code_units()}
    pub(crate) fn projected_size(units:&[u16])->Result<(usize,bool),String>{
        let mut bytes=0usize;let mut unpaired=false;
        for item in char::decode_utf16(units.iter().copied()) {
            let count=match item {Ok(ch)=>ch.len_utf8(),Err(_)=>{unpaired=true;3}};
            bytes=bytes.checked_add(count).ok_or("DOMString scalar-projection size overflow.")?;
        }
        Ok((bytes,unpaired))
    }
    pub(crate) fn from_units(units:&[u16],available:usize)->Result<Self,String>{
        let (bytes,unpaired)=Self::projected_size(units)?;
        let retained=bytes.checked_add(if unpaired {units.len().checked_mul(2).ok_or("DOMString code-unit storage overflow.")?}else{0})
            .filter(|&n|n<=available).ok_or("DOMString retained projection/code-unit allowance exhausted.")?;
        let mut projection=String::new();projection.try_reserve_exact(bytes).map_err(|_|"Cannot reserve DOMString scalar projection.")?;
        projection.extend(char::decode_utf16(units.iter().copied()).map(|item|item.unwrap_or(char::REPLACEMENT_CHARACTER)));
        let raw=if unpaired {
            let mut raw=Vec::new();raw.try_reserve_exact(units.len()).map_err(|_|"Cannot reserve native DOMString code units.")?;
            raw.extend_from_slice(units);Some(raw)
        }else{None};
        let result=Self {projection,units:raw};
        if result.capacity()>available||result.capacity()<retained{return Err("DOMString returned allocation exceeds its admitted storage.".into());}
        Ok(result)
    }
    pub(crate) fn try_clone(&self)->Result<Self,String>{
        if let Some(units)=&self.units {return Self::from_units(units,self.capacity());}
        let mut text=String::new();text.try_reserve_exact(self.projection.len()).map_err(|_|"Cannot reserve copied DOMString.")?;
        text.push_str(&self.projection);Ok(Self::from(text))
    }
    pub(crate) fn concat_size(parts:&[&Self])->Result<(usize,usize),String>{
        let scalar=parts.iter().try_fold(0usize,|n,p|n.checked_add(p.projection.len()).ok_or("DOMString concatenation size overflow."))?;
        if parts.iter().all(|part|part.units.is_none()){return Ok((scalar,scalar));}
        let count=parts.iter().try_fold(0usize,|n,p|n.checked_add(p.utf16_len()).ok_or("DOMString joined code-unit size overflow."))?;
        let mut bytes=0usize;let mut unpaired=false;
        for item in char::decode_utf16(parts.iter().flat_map(|part|part.code_units())) {
            let length=match item {Ok(ch)=>ch.len_utf8(),Err(_)=>{unpaired=true;3}};
            bytes=bytes.checked_add(length).ok_or("DOMString joined projection size overflow.")?;
        }
        if unpaired {
            let raw=count.checked_mul(2).ok_or("DOMString joined payload size overflow.")?;
            Ok((raw,bytes.checked_add(raw).ok_or("DOMString joined owned size overflow.")?))
        }else{Ok((bytes,bytes))}
    }
    pub(crate) fn concat_copy_work(parts:&[&Self])->Result<usize,String>{
        let bytes=Self::concat_size(parts)?.1;
        if parts.iter().all(|part|part.units.is_none()){return Ok(bytes);}
        let units=parts.iter().try_fold(0usize,|n,p|n.checked_add(p.utf16_len()).ok_or("DOMString joined scratch overflow."))?;
        bytes.checked_add(units.checked_mul(2).ok_or("DOMString joined scratch byte overflow.")?).ok_or("DOMString joined copy work overflow.".into())
    }
    pub(crate) fn concat(parts:&[&Self],available:usize)->Result<Self,String>{
        if Self::concat_size(parts)?.1>available{return Err("DOMString concatenation retained allowance exhausted.".into());}
        if parts.iter().all(|part|part.units.is_none()) {
            let bytes=parts.iter().try_fold(0usize,|n,p|n.checked_add(p.projection.len()).ok_or("DOMString concatenation overflow."))?;
            if bytes>available{return Err("DOMString concatenation allowance exhausted.".into());}
            let mut text=String::new();text.try_reserve_exact(bytes).map_err(|_|"Cannot reserve joined DOMString.")?;
            for part in parts {text.push_str(&part.projection);}
            if text.capacity()>available{return Err("Joined DOMString returned allocation exceeds allowance.".into());}
            return Ok(Self::from(text));
        }
        let length=parts.iter().try_fold(0usize,|n,p|n.checked_add(p.utf16_len()).ok_or("DOMString joined code-unit count overflow."))?;
        let mut units=Vec::new();units.try_reserve_exact(length).map_err(|_|"Cannot reserve joined DOMString code units.")?;
        for part in parts {units.extend(part.code_units());}
        Self::from_units(&units,available)
    }
}
impl From<String> for DomString {fn from(projection:String)->Self{Self {projection,units:None}}}
impl From<&str> for DomString {fn from(text:&str)->Self{Self::from(text.to_owned())}}
impl Deref for DomString {type Target=str;fn deref(&self)->&str{self.render_projection()}}
impl fmt::Display for DomString {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{self.render_projection().fmt(f)}}
impl PartialEq for DomString {fn eq(&self,other:&Self)->bool{self.code_units().eq(other.code_units())}}
impl Eq for DomString {}
impl PartialEq<str> for DomString {fn eq(&self,other:&str)->bool{self.code_units().eq(other.encode_utf16())}}
impl PartialEq<&str> for DomString {fn eq(&self,other:&&str)->bool{self==*other}}
impl PartialEq<String> for DomString {fn eq(&self,other:&String)->bool{self==other.as_str()}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_dom_string_preserves_isolates_and_refuses_scalar_projection_as_data(){
        let text=DomString::from_units(&[0xd800,0x41,0xdfff],32).unwrap();
        assert_eq!(text.code_units().collect::<Vec<_>>(),[0xd800,0x41,0xdfff]);
        assert!(text.scalar().is_err());assert_eq!(text.render_projection(),"\u{fffd}A\u{fffd}");
        assert_ne!(text,"\u{fffd}A\u{fffd}");
        assert_eq!(text.try_clone().unwrap(),text);
    }
    #[test]
    fn native_dom_string_join_can_pair_units_without_losing_the_source_values(){
        let a=DomString::from_units(&[0xd83d],8).unwrap();let b=DomString::from_units(&[0xde00],8).unwrap();
        let joined=DomString::concat(&[&a,&b],4).unwrap();
        assert_eq!(joined,"😀");assert!(joined.raw_units().is_none());
        assert_eq!(a.raw_units(),Some(&[0xd83d][..]));assert_eq!(b.raw_units(),Some(&[0xde00][..]));
    }
    #[test]
    fn native_dom_string_exact_owned_storage_and_refusal_are_distinct_from_copy_length(){
        assert!(DomString::from_units(&[0xd800],4).is_err());
        let text=DomString::from_units(&[0xd800],5).unwrap();
        assert_eq!(text.capacity(),5);assert_eq!(text.used_bytes(),5);assert_eq!(text.logical_bytes(),2);
        let mut scalar=String::with_capacity(64);scalar.push('x');
        let scalar=DomString::from(scalar);assert_eq!(scalar.capacity(),64);assert_eq!(scalar.try_clone().unwrap().capacity(),1);
    }
}
