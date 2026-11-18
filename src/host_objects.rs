//! Bounded ordinary expandos on explicitly selected native objects and live
//! named-property reflection. Host handles never acquire arbitrary capability.

use super::{Accessors, Fault, Function, Host, Ordinary, Runtime, Value};

#[cfg(test)]
#[path = "host_document_property_limits_tests.rs"]
mod document_property_limits_tests;

impl Runtime {
    /// Publish the real interface only after its complete private prototype is ready.
    pub fn define_event_target_interface(&mut self, window:u32) -> Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let allocation=self.allocate_host_function_with_length(window,"EventTarget",0).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            let prototype=self.new_object(None)?;
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
            for (name,length) in [("addEventListener",2),("removeEventListener",2),("dispatchEvent",1)] {
                let method=self.allocate_host_function_with_length(window,name,length).map_err(Fault::new)?;
                if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(method.id) {*receiver_required=true;}
                self.store_property(prototype,name.into(),Value::Function(method.id),Some((true,true,true)))?;
            }
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("EventTarget".into()),Some((false,false,true)))?;
            self.put_binding(self.global_environment,"EventTarget".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("EventTarget".into(),(false,true,true));
            self.native_constructor_prototypes.insert((window,"EventTarget"),prototype);
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }
    pub(super) fn native_instance_prototype(&mut self,handle:u32,host:&mut dyn Host)->Result<Ordinary,Fault> {
        self.validate_native_access(handle,host)?;
        if !host.exposed_native_prototype(handle) {return Err(Fault::new("Native host prototype inspection is not implemented for this receiver."));}
        self.native_instance_anchor(handle,host)
    }
    pub(super) fn native_instance_anchor(&mut self,handle:u32,host:&mut dyn Host)->Result<Ordinary,Fault> {
        self.validate_native_access(handle,host)?;
        if !host.native_ancestry_inspection(handle) {return Err(Fault::new("Native host ancestry inspection is not implemented for this receiver."));}
        let ancestry=host.instance_ancestry(handle).ok_or_else(||Fault::new("Native prototype provenance is unavailable."))?;
        let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(window,_)|window));
        if caller!=Some(ancestry.window) {host.validate_window_access(caller,ancestry.window).map_err(Fault::new)?;}
        self.ensure_window_realm(ancestry.window,host)?;
        self.native_ancestry_anchor(ancestry)
    }
    /// Base-interface suffixes for genuine DOM nodes. Specialized immediate
    /// interfaces are not implied by these anchors or exposed for inspection.
    pub fn define_node_interfaces(&mut self,window:u32,constants:&[(&str,u16)],image:Option<(u32,&'static str)>)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let mut parent=*self.native_constructor_prototypes.get(&(window,"EventTarget")).ok_or_else(||Fault::new("EventTarget must precede Node interfaces."))?;
            for name in ["Node","Element","HTMLElement"] {
                let parent_constructor=self.objects[parent].properties.get("constructor").ok_or_else(||Fault::new("Missing native parent constructor."))?.value.as_value()?.clone();
                let Value::Function(parent_function)=parent_constructor else {return Err(Fault::new("Invalid native parent constructor."));};
                let allocation=self.allocate_host_function_with_length(window,name,0).map_err(Fault::new)?;
                if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
                let prototype=self.new_object(None)?;
                self.objects[prototype].prototype=Some(Ordinary::Object(parent));
                self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_function));
                let constructor=Value::Function(allocation.id);
                self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
                self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String(name.into()),Some((false,false,true)))?;
                if name=="Node" {
                    let root=self.allocate_host_function_with_length(window,"\0Node.getRootNode",0).map_err(Fault::new)?;
                    if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(root.id) {*receiver_required=true;}
                    self.store_property(self.function_properties[root.id],"name".into(),Value::String("getRootNode".into()),Some((false,false,true)))?;
                    self.store_property(prototype,"getRootNode".into(),Value::Function(root.id),Some((true,true,true)))?;
                    for (key,operation) in [("nodeType","\0Node.get.nodeType"),("nodeName","\0Node.get.nodeName")] {
                        let getter=self.allocate_host_function_with_length(window,operation,0).map_err(Fault::new)?;
                        if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(getter.id) {*receiver_required=true;}
                        self.store_property(self.function_properties[getter.id],"name".into(),Value::String(format!("get {key}")),Some((false,false,true)))?;
                        self.store_accessor_property(prototype,key.into(),Accessors {get:Some(getter.id),set:None},(true,true))?;
                    }
                    for &(key,value) in constants {
                        for object in [prototype,self.function_properties[allocation.id]] {
                            self.store_property(object,key.into(),Value::Number(f64::from(value)),Some((true,false,false)))?;
                        }
                    }
                }
                if name=="Element" {
                    self.define_element_sibling_getters(prototype,window,"Element")?;
                    for (key,operation) in [("querySelector","\0Element.querySelector"),("querySelectorAll","\0Element.querySelectorAll")] {
                        let method=self.allocate_host_function_with_length(window,operation,1).map_err(Fault::new)?;
                        if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(method.id) {*receiver_required=true;}
                        self.store_property(self.function_properties[method.id],"name".into(),Value::String(key.into()),Some((false,false,true)))?;
                        self.store_property(prototype,key.into(),Value::Function(method.id),Some((true,true,true)))?;
                    }
                }
                self.put_binding(self.global_environment,name.into(),Some(constructor),super::Declaration::Var,false)?;
                self.global_attributes.insert(name.into(),(false,true,true));
                self.native_constructor_prototypes.insert((window,name),prototype);
                parent=prototype;
            }
            // CharacterData is a separate genuine Node suffix. Specialized
            // Text/Comment immediate prototypes remain outside this slice.
            let node=self.native_constructor_prototypes[&(window,"Node")];
            let node_constructor=self.objects[node].properties["constructor"].value.as_value()?.clone();
            let Value::Function(node_constructor)=node_constructor else {return Err(Fault::new("Invalid CharacterData parent constructor."));};
            let allocation=self.allocate_host_function_with_length(window,"CharacterData",0).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let character=self.new_object(None)?;
            self.objects[character].prototype=Some(Ordinary::Object(node));
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(node_constructor));
            let constructor=Value::Function(allocation.id);
            self.store_property(character,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(character),Some((false,false,false)))?;
            self.store_symbol_property(character,self.symbol_to_string_tag.unwrap(),Value::String("CharacterData".into()),Some((false,false,true)))?;
            self.define_element_sibling_getters(character,window,"CharacterData")?;
            self.put_binding(self.global_environment,"CharacterData".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("CharacterData".into(),(false,true,true));
            self.native_constructor_prototypes.insert((window,"CharacterData"),character);
            // Fragment is a separate Node subtype, never an Element suffix.
            let node=*self.native_constructor_prototypes.get(&(window,"Node")).ok_or_else(||Fault::new("Missing Node prototype."))?;
            let Value::Function(node_constructor)=self.objects[node].properties["constructor"].value.as_value()?.clone() else {return Err(Fault::new("Invalid Node constructor."));};
            let allocation=self.allocate_host_function_with_length(window,"DocumentFragment",0).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let fragment=self.new_object(None)?;
            self.objects[fragment].prototype=Some(Ordinary::Object(node));
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(node_constructor));
            let constructor=Value::Function(allocation.id);
            self.store_property(fragment,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(fragment),Some((false,false,false)))?;
            self.store_symbol_property(fragment,self.symbol_to_string_tag.unwrap(),Value::String("DocumentFragment".into()),Some((false,false,true)))?;
            self.put_binding(self.global_environment,"DocumentFragment".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("DocumentFragment".into(),(false,true,true));
            self.native_constructor_prototypes.insert((window,"DocumentFragment"),fragment);
            if let Some(key)=image {
                let image=*self.native_constructor_prototypes.get(&key).ok_or_else(||Fault::new("Missing canonical Image prototype."))?;
                self.objects[image].prototype=Some(Ordinary::Object(parent));
                self.store_symbol_property(image,self.symbol_to_string_tag.unwrap(),Value::String("HTMLImageElement".into()),Some((false,false,true)))?;
            }
            self.define_node_list_iteration_interface(window)?;
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    fn define_node_list_iteration_interface(&mut self,window:u32)->Result<(),Fault> {
        // This finite interface promotes indexed iteration and length only.
        // The existing guarded item/forEach paths remain; forEach is not yet
        // the Array intrinsic alias specified for a complete IDL iterable.
        let allocation=self.allocate_host_function_with_length(window,"NodeList",0).map_err(Fault::new)?;
        if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
        let constructor=Value::Function(allocation.id);
        let prototype=self.new_object(None)?;
        self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
        self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
        self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("NodeList".into()),Some((false,false,true)))?;
        // Web IDL indexed value iterables use these exact realm intrinsics.
        for (key,mode) in [("keys",super::collections::Mode::Keys),("values",super::collections::Mode::Values),("entries",super::collections::Mode::Entries)] {
            let method=self.array_iterator_intrinsic(mode);
            self.store_property(prototype,key.into(),method,Some((true,true,true)))?;
        }
        let values=self.array_iterator_intrinsic(super::collections::Mode::Values);
        self.store_symbol_property(prototype,self.symbol_iterator.unwrap(),values,Some((false,true,true)))?;
        let getter=self.allocate_host_function_with_length(window,"\0NodeList.get.length",0).map_err(Fault::new)?;
        if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(getter.id) {*receiver_required=true;}
        self.store_property(self.function_properties[getter.id],"name".into(),Value::String("get length".into()),Some((false,false,true)))?;
        self.store_accessor_property(prototype,"length".into(),Accessors {get:Some(getter.id),set:None},(true,true))?;
        self.put_binding(self.global_environment,"NodeList".into(),Some(constructor),super::Declaration::Var,false)?;
        self.global_attributes.insert("NodeList".into(),(false,true,true));
        self.native_constructor_prototypes.insert((window,"NodeList"),prototype);
        Ok(())
    }

    fn define_element_sibling_getters(&mut self,prototype:usize,window:u32,family:&str)->Result<(),Fault> {
        let operations=match family {
            "Element"=>[("nextElementSibling","\0Element.get.nextElementSibling"),("previousElementSibling","\0Element.get.previousElementSibling")],
            "CharacterData"=>[("nextElementSibling","\0CharacterData.get.nextElementSibling"),("previousElementSibling","\0CharacterData.get.previousElementSibling")],
            _=>return Err(Fault::new("Unknown element-sibling declaring interface.")),
        };
        for (key,operation) in operations {
            let getter=self.allocate_host_function_with_length(window,operation,0).map_err(Fault::new)?;
            if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(getter.id) {*receiver_required=true;}
            self.store_property(self.function_properties[getter.id],"name".into(),Value::String(format!("get {key}")),Some((false,false,true)))?;
            self.store_accessor_property(prototype,key.into(),Accessors {get:Some(getter.id),set:None},(true,true))?;
        }
        Ok(())
    }
    pub(super) fn validate_native_access(&self, handle: u32, host: &dyn Host) -> Result<(), Fault> {
        if let Some(owner) = host.native_access_owner(handle) {
            let caller = self.native_access_caller.unwrap_or_else(|| self.browser_global.map(|(window, _)| window));
            if caller != Some(owner) { host.validate_window_access(caller, owner).map_err(Fault::new)?; }
        }
        Ok(())
    }
    /// The captured native caller remains distinct from a borrowed builtin's
    /// owner realm. Validate it before the host performs exactly one getter.
    pub(super) fn host_get_checked(&mut self, handle:u32, name:&str, host:&mut dyn Host)->Result<Value,Fault> {
        let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(window,_)|window));
        self.validate_native_access(handle,host)?;
        host.get_with_caller(caller,handle,name).map_err(Fault::new)
    }
    pub(super) fn host_lookup_tail(&mut self,handle:u32,name:&str,host:&mut dyn Host)->Result<Option<Ordinary>,Fault> {
        if !host.ordinary_lookup_tail_property(handle,name) || host.owns_property(handle,name) { return Ok(None); }
        self.host_inherited_read_tail(handle, name, host)
    }
    pub(super) fn host_inherited_read_tail(&mut self,handle:u32,name:&str,host:&mut dyn Host)->Result<Option<Ordinary>,Fault> {
        if !host.ordinary_lookup_tail_property(handle,name) { return Ok(None); }
        let ancestry=host.instance_ancestry(handle).ok_or_else(||Fault::new("Native lookup prototype provenance is unavailable."))?;
        let caller=self.browser_global.map(|(window,_)|window);
        if caller!=Some(ancestry.window) { host.validate_window_access(caller,ancestry.window).map_err(Fault::new)?; }
        // Native child identity reads deliberately do not allocate ES roots.
        // Initialize the actual owner's roots only after origin validation.
        self.ensure_window_realm(ancestry.window,host)?;
        self.native_ancestry_anchor(ancestry).map(Some)
    }
    pub(super) fn ensure_native_expando(&mut self,handle:u32,host:&dyn Host,create:bool)->Result<(),Fault> {
        self.validate_native_access(handle, host)?;
        let Some(owner)=host.expando_owner(handle) else {
            if self.native_expando_owners.contains_key(&handle) {return Err(Fault::new("Native expando receiver has expired or is no longer accessible."));}
            return Ok(());
        };
        self.reject_foreign_window(&Value::Host(handle),host)?;
        let caller=self.browser_global.map(|(window,_)|window);
        if caller!=Some(owner) {host.validate_window_access(caller,owner).map_err(Fault::new)?;}
        if let Some(&previous)=self.native_expando_owners.get(&handle) {
            if previous!=owner {return Err(Fault::new("Native expando owner realm changed."));}
            return Ok(());
        }
        if !create {return Ok(());}
        if self.host_objects.contains_key(&handle) {return Err(Fault::new("Native expando store conflicts with an existing host binding."));}
        let ancestry=host.instance_ancestry(handle).ok_or_else(||Fault::new("Native expando prototype provenance is unavailable."))?;
        if ancestry.window!=owner {return Err(Fault::new("Native expando prototype owner mismatch."));}
        let base_event=matches!(ancestry.constructor,Some((window,"Event"|"UIEvent"|"MouseEvent"|"CustomEvent")) if window==owner);
        let prototype=self.native_ancestry_anchor(ancestry)?;
        let id=self.new_object(None)?;
        self.objects[id].prototype=Some(prototype);
        if base_event {
            self.event_unforgeable(id,owner)?;
        }
        self.host_objects.insert(handle,id);
        self.native_expando_owners.insert(handle,owner);
        Ok(())
    }

    pub fn bind_host_object(
        &mut self,
        handle: u32,
        methods: &[(&str, Value)],
    ) -> Result<(), String> {
        for (_, value) in methods { self.pin_exported_value(value); }
        if self.host_objects.contains_key(&handle) {
            return Err("Native object already has an expando store.".into());
        }
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let prototype = self.new_object(None)?;
            for (name, value) in methods {
                self.set_property(prototype, (*name).into(), value.clone())?;
            }
            let object = self.new_object(None)?;
            self.objects[object].prototype = Some(Ordinary::Object(prototype));
            self.host_objects.insert(handle, object);
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub fn define_host_constructor(
        &mut self,
        name: &str,
        object: u32,
        method: &'static str,
    ) -> Result<Value, String> {
        let value = self.define_host_function(name, object, method)?;
        let Value::Function(index) = value else {
            unreachable!();
        };
        if let Some(Function::Host { constructible, .. }) = self.functions.get_mut(index) {
            *constructible = true;
        }
        let prototype = self.new_object(None).map_err(Fault::display)?;
        self.store_property(
            prototype,
            "constructor".into(),
            Value::Function(index),
            Some((false, true, true)),
        )
        .map_err(Fault::display)?;
        self.store_property(
            self.function_properties[index],
            "prototype".into(),
            Value::Object(prototype),
            Some((false, false, false)),
        )
        .map_err(Fault::display)?;
        self.native_constructor_prototypes.insert((object,method),prototype);
        Ok(value)
    }

    pub(super) fn host_ordinary(
        &mut self,
        handle: u32,
        name: &str,
        host: &dyn Host,
    ) -> Result<Option<Ordinary>,Fault> {
        let native=host.owns_property(handle,name);
        self.ensure_native_expando(handle,host,!native)?;
        Ok((!native)
            .then(|| self.host_objects.get(&handle).copied())
            .flatten()
            .map(Ordinary::Object))
    }

    pub(super) fn host_read(
        &mut self,
        handle: u32,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.foreign_window(handle, host) {
            return self.read_foreign_window(handle, name, host);
        }
        if let Some(object) = self.host_ordinary(handle, name, host)? {
            if self.host_native_fallback_required(handle, object, name, host)? {
                return self.host_read_native_fallback(handle, object, name, host);
            }
            return self.get_ordinary_property_with_receiver(
                object,
                name,
                &Value::Host(handle),
                host,
            );
        }
        if let Some(method) = host.method(handle, name) {
            // An opted-in method's owner/origin remains authoritative even
            // after its function wrapper has been cached. A prior same-origin
            // read must not authorize a later foreign reader of this handle.
            let method_realm = host.native_method_owner(handle,method)
                .map(|owner|self.ensure_window_realm(owner,host)).transpose()?;
            if let Some(value) = self.host_method_values.get(&(handle, method)).cloned() {
                return self.copy_value(&value);
            }
            let value = if let Some(realm) = method_realm {
                self.with_intrinsic_realm(realm,|runtime|runtime.host_method(handle,method).map_err(Fault::new))?
            } else if host.promise_returning_operation(handle,method) {
                let owner=host.native_access_owner(handle).ok_or_else(||Fault::new("Promise operation owner is unavailable."))?;
                let realm=self.ensure_window_realm(owner,host)?;
                self.with_intrinsic_realm(realm,|runtime|runtime.host_method(handle,method).map_err(Fault::new))?
            } else {self.host_method(handle, method).map_err(Fault::new)?};
            if let Some(length) = host.method_length(handle,method) {
                let Value::Function(id) = value else { unreachable!() };
                self.store_property(self.function_properties[id],"length".into(),Value::Number(length as f64),Some((false,false,true)))?;
            }
            self.host_method_values
                .insert((handle, method), value.clone());
            return Ok(value);
        }
        if let Some(tail) = self.host_lookup_tail(handle,name,host)? {
            return self.get_ordinary_property_with_receiver(tail,name,&Value::Host(handle),host);
        }
        let value = self.host_get_checked(handle, name, host)?;
        if matches!(value, Value::Undefined)
            && (host.ordinary_prototype(handle) || host.ordinary_lookup_tail_property(handle,name))
            && !host.has(handle, name).map_err(Fault::new)?
        {
            let prototype = self.host_inherited_read_tail(handle, name, host)?
                .unwrap_or(Ordinary::Object(self.object_prototype.unwrap()));
            return self.get_ordinary_property_with_receiver(
                prototype,
                name,
                &Value::Host(handle),
                host,
            );
        }
        self.checked_value(value)
    }

    #[inline(never)]
    fn host_native_fallback_required(&mut self, handle:u32, object:Ordinary,
        name:&str, host:&dyn Host) -> Result<bool,Fault>
    {
        Ok(host.ordinary_native_fallback(handle,name)
            && !self.owns_property(self.ordinary_id(object)?, name)?)
    }

    #[inline(never)]
    fn host_read_native_fallback(&mut self, handle:u32, object:Ordinary,
        name:&str, host:&mut dyn Host) -> Result<Value,Fault>
    {
        // A named native value is not a fabricated ordinary own key.
        // Keep this Value-producing branch out of ordinary own-accessor reads.
        let value = self.host_get_checked(handle, name, host)?;
        if !matches!(value, Value::Undefined) { return self.checked_value(value); }
        self.get_ordinary_property_with_receiver(object, name, &Value::Host(handle), host)
    }

    pub(super) fn host_ordinary_has(&mut self, handle:u32, object:Ordinary,
        name:&str, host:&mut dyn Host) -> Result<bool,Fault>
    {
        if host.ordinary_native_fallback(handle,name) {
            if self.owns_property(self.ordinary_id(object)?,name)? {return Ok(true);}
            if host.has(handle,name).map_err(Fault::new)? {return Ok(true);}
        }
        Ok(self.property_owner(object,name)?.is_some())
    }

    pub(super) fn host_write(
        &mut self,
        handle: u32,
        name: String,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if let Some(object) = self.host_ordinary(handle, &name, host)? {
            return self.assign_ordinary_property_with_receiver(
                object,
                name,
                value,
                Value::Host(handle),
                host,
            );
        }
        if host.readonly_property(handle, &name) {
            return if self.strict {
                Err(Fault::exception(
                    "TypeError",
                    format!("Native property {name:?} is read-only."),
                ))
            } else {
                Ok(())
            };
        }
        self.host_set_native(handle,&name,value,host)
    }

    /// Shared native conversion/validation/selection transaction. A canonical
    /// accessor calls this directly, so own shadows cannot intercept its slots.
    pub(super) fn host_set_native(&mut self,handle:u32,name:&str,value:Value,host:&mut dyn Host)
        ->Result<(),Fault>
    {
        self.validate_native_access(handle,host)?;
        self.reject_foreign_window(&Value::Host(handle),host)?;
        let value = if host.string_setter(handle, name) {
            let value = if host.nullish_string_setter(handle, name) && matches!(value, Value::Null | Value::Undefined)
                || host.null_string_setter(handle, name) && matches!(value, Value::Null) {
                Value::String(String::new())
            } else { value };
            let primitive = self.primitive_with_hint(value, true, host)?;
            self.work(primitive.bytes())?;
            self.checked_value(Value::String(primitive.script_text()?))?
        } else if host.number_setter(handle, name) {
            let primitive = self.primitive_with_hint(value, false, host)?;
            self.work(primitive.bytes())?;
            Value::Number(primitive.script_number()?)
        } else {
            self.checked_value(value)?
        };
        self.pin_exported_value(&value);
        host.validate_set(handle, name, &value)
            .map_err(|(kind, message)| Fault::exception(kind, message))?;
        let ticket=self.prepare_host_microtask(host,handle,name,std::slice::from_ref(&value),true)?;
        host.native_operation_caller(self.native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(window, _)| window)));
        let result=host.set(handle,name,value).map_err(Fault::new);
        if self.drain_native_promise_effects(host)?!=0 {
            self.defer_host_microtask_after_effects(ticket)?;
        }
        self.finish_host_microtask(host,ticket)?;
        result
    }

    pub(super) fn host_keys(
        &mut self,
        handle: u32,
        inherited: bool,
        host: &mut dyn Host,
    ) -> Result<Vec<String>, Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if self.is_browser_global(handle) { return self.window_keys(inherited, true, host); }
        // Fresh ordinary native instances already own their real unforgeable
        // slots before any string/symbol read. Publish the metered store for
        // own inspection too; ensure still requires explicit owner/provenance.
        self.ensure_native_expando(handle,host,host.exposed_native_prototype(handle))?;
        if host.indexed_iteration_owner(handle).is_some() {
            return self.indexed_host_keys(handle,inherited,true,host);
        }
        if inherited && host.expando_owner(handle).is_some() && !host.exposed_native_prototype(handle) {
            return Err(Fault::new("Inherited native Element enumeration is not implemented."));
        }
        let keys = if let Some(&id) = self.host_objects.get(&handle) {
            if inherited {
                self.inherited_keys(Ordinary::Object(id))?
            } else {
                self.enumerable_keys(id)?
            }
        } else {
            if inherited {host.inherited_keys(handle)} else {host.own_keys(handle)}.map_err(Fault::new)?
        };
        if keys.len() > self.limits.heap.object_properties {
            return Err(Fault::new(
                format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
            ));
        }
        for key in &keys {
            self.property_name(key)?;
        }
        Ok(keys)
    }

    /// Only the approved indexed family merges native own slots and its real
    /// ordinary backing. Other Host key/inspection policies remain guarded.
    #[inline(never)]
    pub(super) fn indexed_host_keys(&mut self,handle:u32,inherited:bool,enumerable_only:bool,host:&mut dyn Host)
        ->Result<Vec<String>,Fault>
    {
        self.validate_indexed_host_iteration(handle,None,host)?;
        let native=host.own_keys(handle).map_err(Fault::new)?;
        let ordinary=if let Some(&id)=self.host_objects.get(&handle) {
            if inherited {self.inherited_keys(Ordinary::Object(id))?}
            else {self.own_string_keys(id,enumerable_only)?}
        } else {Vec::new()};
        let mut seen=std::collections::BTreeSet::new();
        let mut keys=Vec::new();
        for key in native.into_iter().chain(ordinary) {
            self.property_name(&key)?;
            self.work_as(key.len().saturating_mul(2).saturating_add(std::mem::size_of::<String>()*2),super::WorkKind::PropertyEnumeration)?;
            if !seen.insert(key.clone()) {continue;}
            if keys.len()==self.limits.heap.object_properties {
                return Err(Fault::new(format!("Property enumeration limit reached ({} keys).",self.limits.heap.object_properties)));
            }
            keys.try_reserve(1).map_err(|_|Fault::new("Native indexed key merge reservation failed."))?;
            keys.push(key);
        }
        // NodeList's native own indices already arrive in ascending order.
        // Keep the backing/prototype traversal order after them: hoisting a
        // prototype numeric name ahead of an own string would break for-in.
        Ok(keys)
    }

    pub(super) fn host_owned(
        &mut self,
        handle: u32,
        name: &str,
        enumerable: bool,
        host: &mut dyn Host,
    ) -> Result<bool, Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if self.is_browser_global(handle) {
            let own = self.window_own_enumerable(name, host)?;
            return Ok(own.is_some_and(|is_enumerable| !enumerable || is_enumerable));
        }
        if let Some(object) = self.host_ordinary(handle, name, host)? {
            let id = self.ordinary_id(object)?;
            return if enumerable {
                self.own_enumerable(id, name)
            } else {
                self.owns_property(id, name)
            };
        }
        Ok(host.own_data(handle, name).map_err(Fault::new)?.is_some())
    }

    pub(super) fn host_descriptor(
        &mut self,
        handle: u32,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if let Some(object) = self.host_ordinary(handle, name, host)? {
            return self.own_descriptor(object, name);
        }
        let Some(value) = host.own_data(handle, name).map_err(Fault::new)? else {
            return Ok(Value::Undefined);
        };
        let id = self.new_object(None)?;
        for (key, value) in [
            ("value", value),
            (
                "writable",
                Value::Bool(!host.readonly_property(handle, name)),
            ),
            ("enumerable", Value::Bool(true)),
            ("configurable", Value::Bool(true)),
        ] {
            self.set_property(id, key.into(), value)?;
        }
        Ok(Value::Object(id))
    }
}

#[cfg(test)]
mod expando_tests {
    use super::*;
    struct Native { enabled:bool, reads:usize }
    impl Host for Native {
        fn expando_owner(&self,object:u32)->Option<u32> {(self.enabled&&object==42).then_some(77)}
        fn instance_ancestry(&self,object:u32)->Option<super::super::NativeAncestry> {
            self.expando_owner(object).map(|window|super::super::NativeAncestry {window,constructor:None})
        }
        fn owns_property(&self,_:u32,name:&str)->bool {name=="nativeField"}
        fn own_keys(&mut self,_:u32)->Result<Vec<String>,String> {Ok(Vec::new())}
        fn own_data(&mut self,_:u32,_:&str)->Result<Option<Value>,String> {Ok(None)}
        fn get(&mut self,_:u32,name:&str)->Result<Value,String> {self.reads+=1;if name=="nativeField" {Ok(Value::Number(7.0))} else {Err("Unknown native read".into())}}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String> {Err("No native write".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String> {Err("No native call".into())}
    }
    #[test]
    fn native_expando_allocation_failure_is_atomic_and_recovery_preserves_native_paths() {
        let mut runtime=Runtime::for_test();runtime.bind_browser_global(77).unwrap();
        runtime.define_global("element",Value::Host(42)).unwrap();
        let mut host=Native {enabled:true,reads:0};
        let old_limit=runtime.limits.heap.objects;
        runtime.limits.heap.objects=runtime.objects.len();
        let error=runtime.execute("element.customProbe=1;",&mut host).unwrap_err();
        assert!(error.contains("limit"),"{error}");
        assert!(!runtime.host_objects.contains_key(&42));
        assert!(!runtime.native_expando_owners.contains_key(&42));
        assert_eq!(host.reads,0);
        runtime.limits.heap.objects=old_limit;runtime.begin_job();
        assert_eq!(runtime.execute("if(element.customProbe!==undefined)throw 'failed write leaked';element.customProbe=2;element.customProbe+element.nativeField;",&mut host).unwrap().to_number(),9.0);
        assert_eq!(host.reads,1);
    }
    #[test]
    fn cached_native_expando_reads_and_inspection_revalidate_capability() {
        let mut runtime=Runtime::for_test();runtime.bind_browser_global(77).unwrap();
        runtime.define_global("element",Value::Host(42)).unwrap();
        let mut host=Native {enabled:true,reads:0};
        runtime.execute("element.customProbe=17;element[Symbol('own')]=23;",&mut host).unwrap();
        host.enabled=false;
        for source in ["element.customProbe;","Object.keys(element);","Object.getOwnPropertyNames(element);","Object.getOwnPropertySymbols(element);","Object.assign({},element);"] {
            runtime.begin_job();
            let error=runtime.execute(source,&mut host).unwrap_err();
            assert!(error.contains("no longer accessible"),"{source}: {error}");
        }
        assert_eq!(host.reads,0);
        host.enabled=true;runtime.begin_job();
        assert_eq!(runtime.execute("element.customProbe;",&mut host).unwrap().to_number(),17.0);
    }
}
