//! Original exact String.fromCharCode conversion and bounded unit staging.
//! https://tc39.es/ecma262/#sec-string.fromcharcode
use super::{Fault,Host,Runtime,Value,WorkKind};

impl Runtime {
    fn char_code_workspace(&mut self,bytes:usize,owned:&mut usize)->Result<(),Fault> {
        let next=self.value_bytes.checked_add(bytes).filter(|next|*next<=self.limits.heap.retained_values)
            .ok_or_else(||Fault::new("String.fromCharCode workspace limit reached."))?;
        let next_owned=owned.checked_add(bytes).ok_or_else(||Fault::new("String.fromCharCode workspace accounting overflow."))?;
        self.value_bytes=next;*owned=next_owned;Ok(())
    }

    pub(super) fn string_from_char_code(&mut self,args:&[Value],host:&mut dyn Host)->Result<Value,Fault> {
        self.step()?;
        if args.len()>self.limits.capacity.arguments{return Err(self.argument_limit_fault());}
        let bytes=args.len().checked_mul(2).ok_or_else(||self.string_limit_fault())?;
        self.work_as(bytes,WorkKind::ConversionOutput)?;
        let mut owned=0usize;
        let result=(|| {
            // This staging buffer survives every numeric hook, including a
            // nested producer call, so it shares retained admission with
            // author publications and the independently admitted final output.
            self.char_code_workspace(bytes,&mut owned)?;
            let mut units=Vec::new();
            units.try_reserve_exact(args.len()).map_err(|_|Fault::new("String.fromCharCode staging reservation failed."))?;
            let actual=units.capacity().checked_mul(2).ok_or_else(||Fault::new("String.fromCharCode staging size overflow."))?;
            if actual>bytes {self.char_code_workspace(actual-bytes,&mut owned)?;}
            for value in args {
                self.step()?;
                let number=self.binary_number(value,host)?;
                let unit=if number.is_finite(){number.trunc().rem_euclid(65536.0) as u16}else{0};
                self.work_as(2,WorkKind::ConversionOutput)?;
                units.push(unit);
            }
            // Canonical scalar/raw representation, output storage and copy
            // work use the same original builder as exact string slicing.
            self.string_units_output(units.iter().copied(),units.len(),bytes,"String.fromCharCode")
        })();
        self.value_bytes=self.value_bytes.checked_sub(owned)
            .ok_or_else(||Fault::new("String.fromCharCode workspace refund failed."))?;
        result
    }
}
