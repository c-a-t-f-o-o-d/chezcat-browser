//! Original bounded forward and reverse search over exact UTF-16 code units.
//! https://tc39.es/ecma262/#sec-string.prototype.lastindexof
//! https://tc39.es/ecma262/#sec-string.prototype.indexof
use super::{bounded_index, Fault, Host, Runtime, Value, WorkKind};

#[derive(Clone,Copy)]
enum Input<'a> { Scalar(&'a str), Units(&'a [u16]) }
impl Input<'_> {
    fn bytes(self)->usize {match self {Self::Scalar(s)=>s.len(),Self::Units(s)=>s.len().saturating_mul(2)}}
    fn len(self)->usize {match self {Self::Scalar(s)=>s.encode_utf16().count(),Self::Units(s)=>s.len()}}
}

impl Runtime {
    #[cfg(test)]
    pub(super) fn string_last_index_of(&mut self,receiver:&str,args:&[Value],host:&mut dyn Host)->Result<Value,Fault> {
        self.string_search_input(Input::Scalar(receiver),args,true,host)
    }

    pub(super) fn string_last_index_value(&mut self,receiver:&Value,args:&[Value],host:&mut dyn Host)->Result<Value,Fault> {
        self.string_index_value(receiver,args,true,host)
    }

    pub(super) fn string_index_value(&mut self,receiver:&Value,args:&[Value],reverse:bool,host:&mut dyn Host)->Result<Value,Fault> {
        let input=match receiver {
            Value::String(s)=>Input::Scalar(s),Value::StringUnits(s)=>Input::Units(s),
            _=>return Err(Fault::new("String search requires an already converted string.")),
        };
        self.string_search_input(input,args,reverse,host)
    }

    fn search_workspace(&mut self,bytes:usize,owned:&mut usize)->Result<(),Fault> {
        let next=self.value_bytes.checked_add(bytes).filter(|next|*next<=self.limits.heap.retained_values)
            .ok_or_else(||Fault::new("String search workspace limit reached."))?;
        let next_owned=owned.checked_add(bytes).ok_or_else(||Fault::new("String search workspace accounting overflow."))?;
        self.value_bytes=next;*owned=next_owned;Ok(())
    }

    fn search_units(&mut self,input:Input<'_>,owned:&mut usize)->Result<Vec<u16>,Fault> {
        let count=input.len();
        let bytes=count.checked_mul(2).ok_or_else(||Fault::new("String search workspace size overflow."))?;
        self.work_as(bytes,WorkKind::StringMethodInput)?;
        self.search_workspace(bytes,owned)?;
        let mut units=Vec::new();
        units.try_reserve_exact(count).map_err(|_|Fault::new("String search input reservation failed."))?;
        let actual=units.capacity().checked_mul(2).ok_or_else(||Fault::new("String search allocation size overflow."))?;
        if actual>bytes {self.search_workspace(actual-bytes,owned)?;}
        match input {
            Input::Scalar(text)=>for unit in text.encode_utf16(){self.step()?;units.push(unit);},
            Input::Units(source)=>{self.step()?;units.extend_from_slice(source);}
        }
        Ok(units)
    }

    fn string_search_input(&mut self,receiver:Input<'_>,args:&[Value],reverse:bool,host:&mut dyn Host)->Result<Value,Fault> {
        self.work_as(receiver.bytes().saturating_mul(3),WorkKind::StringMethodInput)?;
        let mut owned=0usize;
        let result=(|| {
            // Receiver conversion is complete. Preserve its immutable exact
            // value while the search and numeric-position hooks run in order.
            let value=self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
            let value=self.primitive_with_hint(value,true,host)?;
            let bytes=if value.is_string(){value.bytes()}else{64};
            self.work_as(bytes,WorkKind::ConversionOutput)?;
            let search=if value.is_string(){value}else{Value::String(value.script_text()?)};
            let search=self.checked_value(search)?;
            let backing=match &search {
                Value::String(s)=>s.capacity(),Value::StringUnits(s)=>s.capacity().checked_mul(2)
                    .ok_or_else(||Fault::new("String search capture size overflow."))?,_=>unreachable!(),
            };
            // This owned conversion survives position callbacks and nested
            // searches, so its actual backing shares retained admission.
            self.search_workspace(backing,&mut owned)?;
            let position=self.binary_number(args.get(1).unwrap_or(&Value::Undefined),host)?;
            self.work_as(search.bytes().saturating_mul(2),WorkKind::StringMethodInput)?;
            let pattern=match &search {Value::String(s)=>Input::Scalar(s),Value::StringUnits(s)=>Input::Units(s),_=>unreachable!()};
            let Some(max_start)=receiver.len().checked_sub(pattern.len())else{return Ok(Value::Number(-1.0));};
            let start=if reverse {
                if position.is_nan(){max_start}else{bounded_index(position,max_start)}
            } else { bounded_index(position,receiver.len()) };
            if start>max_start{return Ok(Value::Number(-1.0));}
            if pattern.len()==0{return Ok(Value::Number(start as f64));}
            let units=self.search_units(receiver,&mut owned)?;
            let needle=self.search_units(pattern,&mut owned)?;
            let candidates=if reverse {start.saturating_add(1)}else{max_start-start+1};
            for offset in 0..candidates {
                let at=if reverse {start-offset}else{start+offset};
                self.step()?;
                self.work(needle.len().saturating_mul(2))?;
                if units[at..at+needle.len()]==needle{return Ok(Value::Number(at as f64));}
            }
            Ok(Value::Number(-1.0))
        })();
        // Refund this operation's captures and workspaces only. Author
        // publications and nested-call accounting remain committed.
        self.value_bytes=self.value_bytes.checked_sub(owned)
            .ok_or_else(||Fault::new("String search workspace refund failed."))?;
        result
    }
}
