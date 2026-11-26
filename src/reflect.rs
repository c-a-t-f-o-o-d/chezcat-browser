//! Reflect lookup and assignment keep the target and receiver distinct.
use super::*;

#[cfg(test)]
#[path = "reflect_set_tests.rs"]
mod set_tests;
#[cfg(test)]
#[path = "reflect_get_tests.rs"]
mod tests;

impl Runtime {
    pub(super) fn initialize_reflect(&mut self) -> Result<(), Fault> {
        let object = self.new_object(None)?;
        self.prototype_method(object, "get", Builtin::ReflectGet)?;
        self.prototype_method(object, "set", Builtin::ReflectSet)?;
        self.prototype_method(object, "ownKeys", Builtin::ReflectOwnKeys)?;
        let tag = self
            .symbol_to_string_tag
            .expect("initialized well-known symbols");
        self.store_symbol_property(
            object,
            tag,
            Value::String("Reflect".into()),
            Some((false, false, true)),
        )?;
        self.put_binding(
            self.global_environment,
            "Reflect".into(),
            Some(Value::Object(object)),
            Declaration::Var,
            false,
        )?;
        Ok(())
    }

    /// Original bounded own-key reflection; native targets keep the same
    /// owner/origin/lifetime gates as Object name and symbol inspection.
    pub(super) fn reflect_own_keys(&mut self,args:&[Value],host:&mut dyn Host)->Result<Value,Fault> {
        self.step()?;
        let target=args.first().unwrap_or(&Value::Undefined);
        if !matches!(target,Value::Object(_)|Value::Function(_)|Value::Host(_)) {
            return Err(Fault::exception("TypeError","Reflect.ownKeys requires an object target."));
        }
        let names=self.inspection_own_string_keys(target,host)?;
        let symbols=if let Some(object)=self.symbol_backing(target,host)? {
            let id=self.ordinary_id(object)?;self.own_symbol_keys(id,false)?
        } else {Vec::new()};
        let count=names.len().checked_add(symbols.len()).filter(|count|*count<=self.limits.heap.object_properties)
            .ok_or_else(||Fault::new("Own-key reflection exceeds the property enumeration allowance."))?;
        let result=self.new_object(Some(count))?;
        // Snapshot names and Symbols before result construction. Create own
        // data properties directly so prototype setters cannot intercept them.
        for (index,key) in names.into_iter().map(StringKey::into_value).chain(symbols.into_iter().map(Value::Symbol)).enumerate() {
            self.step()?;self.set_property(result,index.to_string(),key)?;
        }
        Ok(Value::Object(result))
    }

    #[inline(never)]
    pub(super) fn reflect_get(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let target = args.first().unwrap_or(&Value::Undefined);
        match target {
            Value::Object(id) => {
                self.object(*id)?;
            }
            Value::Function(id) => {
                self.ordinary_id(Ordinary::Function(*id))?;
            }
            Value::Host(handle) => self.validate_native_access(*handle, host)?,
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "Reflect.get requires an object target.",
                ))
            }
        }
        let key = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let key = self.property_key(key, host)?;
        let receiver = args.get(2).unwrap_or(target);
        if let Value::Host(handle) = target {
            // Author key conversion can change a document's access state.
            self.validate_native_access(*handle, host)?;
            if !strict_equal(target, receiver) {
                if host.named_units_setter(*handle) {
                    // Native named data retains its value regardless of the
                    // receiver. Inherited accessors use the actual receiver.
                    self.reject_foreign_window(receiver,host)?;
                    return match key {
                        PropertyKey::String(name)=>{
                            self.property_name(&name)?;
                            let present=host.named_scalar_presence(*handle,&name).map_err(Fault::new)?
                                .ok_or_else(||Fault::new("Native named presence is unavailable."))?;
                            if present {self.read_property(target,&name,host)} else {
                                let prototype=self.native_instance_prototype(*handle,host)?;
                                self.get_ordinary_property_with_receiver(prototype,&name,receiver,host)
                            }
                        }
                        PropertyKey::StringUnits(units)=>{
                            self.property_name_units(&units)?;
                            let present=host.named_units_presence(*handle,&units).map_err(Fault::new)?
                                .ok_or_else(||Fault::new("Native raw named presence is unavailable."))?;
                            if present {self.read_units_property(target,&units,host)} else {
                                let prototype=self.native_instance_prototype(*handle,host)?;
                                self.get_units_property_with_receiver(prototype,&units,receiver,host)
                            }
                        }
                        PropertyKey::Symbol(key)=>{
                            let backing=self.symbol_backing(target,host)?.ok_or_else(||Fault::new("Native Symbol backing is unavailable."))?;
                            self.reflect_get_symbol(backing,key,receiver,host)
                        }
                    };
                }
                // Opaque native slots do not yet expose descriptor/receiver
                // metadata. Never silently substitute target for receiver.
                return Err(Fault::new(
                    "Reflect.get with a different receiver on a native target is not implemented.",
                ));
            }
            return match key {
                PropertyKey::String(name) => self.read_property(target, &name, host),
                PropertyKey::StringUnits(name) => self.read_units_property(target, &name, host),
                PropertyKey::Symbol(key) => {
                    self.get_symbol_property(Value::Host(*handle), key, host)
                }
            };
        }
        let object = Ordinary::from_value(target).expect("validated ordinary target");
        // A borrowed builtin's realm never grants access to its actual native
        // receiver. Preserve the same callback gate as ordinary super reads.
        self.reject_foreign_window(receiver, host)?;
        match key {
            PropertyKey::String(name) => {
                self.get_ordinary_property_with_receiver(object, &name, receiver, host)
            }
            PropertyKey::StringUnits(name) => {
                self.get_units_property_with_receiver(object, &name, receiver, host)
            }
            PropertyKey::Symbol(key) => self.reflect_get_symbol(object, key, receiver, host),
        }
    }

    pub(super) fn reflect_get_symbol(
        &mut self,
        object: Ordinary,
        key: usize,
        receiver: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Some(owner) = self.symbol_dispatch_owner(object, key)? else {
            return Ok(Value::Undefined);
        };
        if self.objects[owner].proxy.is_some() {
            return self.proxy_get(owner, PropertyKey::Symbol(key), receiver, host);
        }
        let property = &self.objects[owner].symbol_properties[&key];
        if let Some(accessor) = property.accessor {
            let receiver = if accessor.get.is_some() {
                self.copy_value(receiver)?
            } else {
                Value::Undefined
            };
            return self.call_accessor(accessor.get, receiver, &[], host);
        }
        if let Some(getter) = property.getter {
            let function = property.value.as_value()?.clone();
            let receiver = self.copy_value(receiver)?;
            return self.native_getter_in_realm(&function, getter, receiver);
        }
        let bytes = property.value.bytes();
        self.work(bytes)?;
        Ok(self.objects[owner].symbol_properties[&key]
            .value
            .as_value()?
            .clone())
    }

    // These families have virtual keys, aliasing or specialized definitions.
    // Their existing assignment routines can silently refuse in sloppy mode.
    pub(super) fn uses_stored_property_descriptors(&self, id: usize) -> bool {
        let object = &self.objects[id];
        object.length.is_none()
            && object.arguments.is_none()
            && object.regexp.is_none()
            && !matches!(
                &object.primitive,
                Some(primitive_wrappers::Data::String { .. })
            )
            && !matches!(&object.binary, Some(binary_data::Data::Typed(_)))
    }

    // OrdinarySetWithOwnDescriptor's data case. super already owns its value;
    // Reflect copies a borrowed argument only after the receiver accepts it.
    pub(super) fn set_stored_receiver_data(
        &mut self,
        id: usize,
        key: PropertyKey,
        take_value: impl FnOnce(&mut Self) -> Result<Value, Fault>,
    ) -> Result<bool, Fault> {
        match self.stored_descriptor_property(id, &key) {
            Some(property)
                if property.accessor.is_some()
                    || property.getter.is_some()
                    || !property.writable =>
            {
                return Ok(false);
            }
            None if !self.objects[id].extensible => return Ok(false),
            _ => {}
        }
        let value = take_value(self)?;
        match key {
            PropertyKey::String(name) => self.store_property(id, name, value, None)?,
            PropertyKey::StringUnits(name) => self.store_units_property(id, name, value, None)?,
            PropertyKey::Symbol(key) => self.store_symbol_property(id, key, value, None)?,
        }
        Ok(true)
    }

    fn reflect_native_named_set(&mut self,handle:u32,key:PropertyKey,value:&Value,host:&mut dyn Host)->Result<bool,Fault>{
        self.validate_native_access(handle,host)?;
        let value=self.copy_value(value)?;
        match key {
            PropertyKey::String(name)=>self.host_write(handle,name,value,host)?,
            PropertyKey::StringUnits(units)=>self.assign_units_property(Value::Host(handle),units,value,host)?,
            PropertyKey::Symbol(_)=>return Err(Fault::new("Native symbol data receivers are not implemented.")),
        }
        Ok(true)
    }

    #[inline(never)]
    pub(super) fn reflect_set(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let target = args.first().unwrap_or(&Value::Undefined);
        let base = Ordinary::from_value(target);
        if let Some(base) = base {
            self.ordinary_id(base)?;
        } else if let Value::Host(handle) = target {
            self.validate_native_access(*handle, host)?;
        } else {
            return Err(Fault::exception(
                "TypeError",
                "Reflect.set requires an object target.",
            ));
        }
        let key = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let key = self.property_key(key, host)?;
        let value = args.get(2).unwrap_or(&Value::Undefined);
        let receiver = args.get(3).unwrap_or(target);
        self.reject_foreign_window(receiver, host)?;
        let Some(base) = base else {
            self.reject_foreign_window(target, host)?;
            if let Value::Host(handle)=target {
                if matches!(key,PropertyKey::Symbol(_)) && host.symbol_property_owner(*handle).is_some() {
                    let backing=self.symbol_backing(target,host)?.ok_or_else(||Fault::new("Native Symbol backing is unavailable."))?;
                    return self.reflect_set_key(backing,key,value,receiver,host).map(Value::Bool);
                }
                if host.named_units_setter(*handle) {
                    // Web IDL's named setter wins only for the same receiver.
                    // Otherwise its ordinary own descriptor excludes named
                    // attributes, so continue at the genuine prototype chain.
                    if matches!(receiver,Value::Host(other) if other==handle)&&!matches!(key,PropertyKey::Symbol(_)) {
                        return self.reflect_native_named_set(*handle,key,value,host).map(Value::Bool);
                    }
                    let prototype=self.native_instance_prototype(*handle,host)?;
                    return self.reflect_set_key(prototype,key,value,receiver,host).map(Value::Bool);
                }
            }
            return Err(Fault::new(
                "Reflect.set on a native target is not implemented.",
            ));
        };
        Ok(Value::Bool(self.reflect_set_key(base, key, value, receiver, host)?))
    }

    #[inline(never)]
    pub(super) fn reflect_set_key(&mut self, mut current: Ordinary, key: PropertyKey, value: &Value,
                                 receiver: &Value, host: &mut dyn Host) -> Result<bool, Fault> {
        loop {
            self.step()?;
            let id = self.ordinary_id(current)?;
            if self.objects[id].proxy.is_some() {
                return self.proxy_set(id, key, value, receiver, host);
            }
            if !self.uses_stored_property_descriptors(id) {
                return Err(Fault::new(
                    "Reflect.set on this exotic prototype or target is not implemented.",
                ));
            }
            if let PropertyKey::String(name) = &key {
                self.materialize_host_accessor(id, name, true)?;
            }
            if let Some(property) = self.stored_descriptor_property(id, &key) {
                if let Some(accessor) = property.accessor {
                    let Some(setter) = accessor.set else {
                        return Ok(false);
                    };
                    let receiver = self.copy_value(receiver)?;
                    self.call_accessor(Some(setter), receiver, std::slice::from_ref(value), host)?;
                    return Ok(true);
                }
                if property.getter.is_some() || !property.writable {
                    return Ok(false);
                }
                break;
            }
            let Some(parent) = self.objects[id].prototype else {
                break;
            };
            current = parent;
        }
        if let Value::Host(handle)=receiver {
            self.validate_native_access(*handle,host)?;
            if host.named_units_setter(*handle)&&!matches!(key,PropertyKey::Symbol(_)) {
                return self.reflect_native_named_set(*handle,key,value,host);
            }
            if matches!(key,PropertyKey::Symbol(_)) && host.symbol_property_owner(*handle).is_some() {
                let backing=self.symbol_backing(receiver,host)?.ok_or_else(||Fault::new("Native Symbol backing is unavailable."))?;
                let id=self.ordinary_id(backing)?;
                return self.set_stored_receiver_data(id,key,|runtime|runtime.copy_value(value));
            }
            return Err(Fault::new(
                "Reflect.set cannot define data on a native receiver.",
            ));
        }
        let Some(receiver) = Ordinary::from_value(receiver) else {
            return Ok(false);
        };
        let id = self.ordinary_id(receiver)?;
        if self.objects[id].proxy.is_some() {
            return self.proxy_receiver_set_data(id, key, value, host);
        }
        if !self.uses_stored_property_descriptors(id) {
            return Err(Fault::new(
                "Reflect.set on this exotic data receiver is not implemented.",
            ));
        }
        self.set_stored_receiver_data(
            id,
            key,
            |runtime| runtime.copy_value(value),
        )
    }
}

#[cfg(test)]
#[path = "reflect_own_keys_tests.rs"]
mod own_keys_tests;

#[cfg(test)]
mod dataset_reflect_get_security_tests {
    use super::*;
    struct NativeNamed { window:super::tests::WindowReceiverHost, probes:usize, gets:usize }
    impl Host for NativeNamed {
        fn native_access_owner(&self,handle:u32)->Option<u32>{
            if handle==42 {Some(77)} else {self.window.native_access_owner(handle)}
        }
        fn window_context(&self,handle:u32)->Option<WindowContext>{self.window.window_context(handle)}
        fn validate_window_access(&self,caller:Option<u32>,window:u32)->Result<(),String>{self.window.validate_window_access(caller,window)}
        fn named_units_setter(&self,handle:u32)->bool{handle==42}
        fn named_scalar_presence(&mut self,handle:u32,_:&str)->Result<Option<bool>,String>{assert_eq!(handle,42);self.probes+=1;Ok(Some(true))}
        fn named_units_presence(&mut self,handle:u32,_:&[u16])->Result<Option<bool>,String>{assert_eq!(handle,42);self.probes+=1;Ok(Some(true))}
        fn get(&mut self,handle:u32,name:&str)->Result<Value,String>{assert_eq!((handle,name),(42,"value"));self.gets+=1;Ok(Value::Number(37.0))}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native write")}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("unexpected native call")}
    }
    #[test]
    fn dataset_reflect_get_foreign_receiver_guards_precede_named_data_and_symbol_store(){
        let mut r=Runtime::for_test();r.bind_browser_global(77).unwrap();
        let mut h=NativeNamed{window:super::tests::WindowReceiverHost{same_origin:true},probes:0,gets:0};
        let symbol=r.execute("Symbol('receiver')",&mut h).unwrap();
        for same_origin in [true,false]{h.window.same_origin=same_origin;
            for key in [Value::String("value".into()),Value::StringUnits(vec![120,0xd800]),symbol.clone()]{
                let stores=r.native_symbol_backings.len();r.begin_job();
                let error=r.reflect_get(&[Value::Host(42),key,Value::Host(88)],&mut h).unwrap_err();
                assert!(error.message.contains(if same_origin{"foreign Window operation is not implemented"}else{"same-origin"}),"{}",error.message);
                assert_eq!((h.probes,h.gets),(0,0));assert_eq!(r.native_symbol_backings.len(),stores);assert!(r.transient_registration_is_empty());
            }
        }
        r.begin_job();assert_eq!(r.reflect_get(&[Value::Host(42),Value::String("value".into()),Value::Number(3.0)],&mut h).unwrap().to_number(),37.0);assert_eq!((h.probes,h.gets),(1,1));
    }
}
