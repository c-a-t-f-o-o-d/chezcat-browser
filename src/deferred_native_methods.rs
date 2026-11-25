//! Private data-slot deferral for native method identities. No lazy getter or
//! placeholder is exposed through JavaScript Value, descriptors or reflection.
use super::*;


#[derive(Clone, Copy)]
enum State { Pending, Ready(usize), Failed }

#[derive(Clone, Copy)]
enum Definition { Native(Builtin), Host { window:u32, operation:&'static str, length:usize }, ParamConstructor { window:u32, prototype:usize, parent:usize }, DataListConstructor { window:u32, prototype:usize, parent:usize }, OptionConstructor { window:u32, prototype:usize, parent:usize }, OptGroupConstructor { window:u32, prototype:usize, parent:usize } }

pub(super) struct NativeMethod {
    definition: Definition,
    name: String,
    realm: Option<usize>,
    state: State,
}

impl Runtime {
    pub(super) fn deferred_native_method(&mut self, owner: usize, name: &str, builtin: Builtin) -> Result<(), Fault> {
        self.defer_method(owner,name,Definition::Native(builtin),false)
    }

    pub(super) fn deferred_host_method(&mut self, owner:usize, window:u32, name:&'static str, operation:&'static str, length:usize)->Result<(),Fault>{
        self.defer_method(owner,name,Definition::Host{window,operation,length},true)
    }

    pub(super) fn defer_param_constructor(&mut self, window:u32, prototype:usize, parent:usize)->Result<(),Fault>{
        self.defer_method(prototype,"constructor",Definition::ParamConstructor{window,prototype,parent},false)?;
        let StoredValue::DeferredNative(index)=self.objects[prototype].properties["constructor"].value else {unreachable!()};
        self.deferred_param_constructor=Some(index);
        self.reserve_deferred_global_orders(&["HTMLParamElement"])
    }

    pub(super) fn ensure_param_constructor(&mut self)->Result<(),Fault>{
        let Some(index)=self.deferred_param_constructor else{return Ok(())};
        let function=self.materialize_native_identity(index)?;
        if let Err(error)=self.put_binding(self.global_environment,"HTMLParamElement".into(),Some(Value::Function(function)),Declaration::Var,false){
            self.deferred_native_methods[index].state=State::Failed;
            return Err(error);
        }
        self.deferred_param_constructor=None;
        Ok(())
    }

    pub(super) fn defer_datalist_constructor(&mut self, window:u32, prototype:usize, parent:usize)->Result<(),Fault>{
        self.defer_method(prototype,"constructor",Definition::DataListConstructor{window,prototype,parent},false)?;
        let StoredValue::DeferredNative(index)=self.objects[prototype].properties["constructor"].value else {unreachable!()};
        self.deferred_datalist_constructor=Some(index);
        self.reserve_deferred_global_orders(&["HTMLDataListElement"])
    }

    pub(super) fn ensure_datalist_constructor(&mut self)->Result<(),Fault>{
        let Some(index)=self.deferred_datalist_constructor else{return Ok(())};
        let function=self.materialize_native_identity(index)?;
        if let Err(error)=self.put_binding(self.global_environment,"HTMLDataListElement".into(),Some(Value::Function(function)),Declaration::Var,false){
            self.deferred_native_methods[index].state=State::Failed;
            return Err(error);
        }
        self.deferred_datalist_constructor=None;
        Ok(())
    }

    pub(super) fn defer_option_constructor(&mut self, window:u32, prototype:usize, parent:usize)->Result<(),Fault>{
        self.defer_method(prototype,"constructor",Definition::OptionConstructor{window,prototype,parent},false)?;
        let StoredValue::DeferredNative(index)=self.objects[prototype].properties["constructor"].value else {unreachable!()};
        self.deferred_option_constructor=Some(index);
        self.reserve_deferred_global_orders(&["HTMLOptionElement"])
    }

    pub(super) fn ensure_option_constructor(&mut self)->Result<(),Fault>{
        let Some(index)=self.deferred_option_constructor else{return Ok(())};
        let function=self.materialize_native_identity(index)?;
        if let Err(error)=self.put_binding(self.global_environment,"HTMLOptionElement".into(),Some(Value::Function(function)),Declaration::Var,false){
            self.deferred_native_methods[index].state=State::Failed;
            return Err(error);
        }
        self.deferred_option_constructor=None;
        Ok(())
    }

    pub(super) fn defer_optgroup_constructor(&mut self, window:u32, prototype:usize, parent:usize)->Result<(),Fault>{
        self.defer_method(prototype,"constructor",Definition::OptGroupConstructor{window,prototype,parent},false)?;
        let StoredValue::DeferredNative(index)=self.objects[prototype].properties["constructor"].value else {unreachable!()};
        self.deferred_optgroup_constructor=Some(index);
        self.reserve_deferred_global_orders(&["HTMLOptGroupElement"])
    }

    pub(super) fn ensure_optgroup_constructor(&mut self)->Result<(),Fault>{
        let Some(index)=self.deferred_optgroup_constructor else{return Ok(())};
        let function=self.materialize_native_identity(index)?;
        if let Err(error)=self.put_binding(self.global_environment,"HTMLOptGroupElement".into(),Some(Value::Function(function)),Declaration::Var,false){
            self.deferred_native_methods[index].state=State::Failed;
            return Err(error);
        }
        self.deferred_optgroup_constructor=None;
        Ok(())
    }

    pub(super) fn deferred_param_constructor_edge(&self,index:usize)->Option<usize>{
        match self.deferred_native_methods[index].state{State::Ready(function)=>Some(function),_=>None}
    }

    fn defer_method(&mut self, owner:usize, name:&str, definition:Definition, enumerable:bool)->Result<(),Fault>{
        #[cfg(test)]
        if self.limits.test_entry_arenas && self.deferred_native_methods.len() >= self.limits.capacity.functions {
            return Err(Fault::new("Deferred native method metadata limit reached (1024)."));
        }
        let available = self.limits.heap.retained_values.checked_sub(self.value_bytes)
            .and_then(|bytes| bytes.checked_sub(name.len()))
            .ok_or_else(|| Fault::new("Retained native method storage exhausted."))?;
        let growth = super::limits::prepare_retained_growth(&self.deferred_native_methods, 1, available)?;
        let bytes = growth.added + name.len();
        self.work(bytes.saturating_add(if growth.added != 0 { self.deferred_native_methods.len() } else { 0 }))?;
        growth.commit(&mut self.deferred_native_methods);
        let index = self.deferred_native_methods.len();
        self.deferred_native_methods.push(NativeMethod { definition, name: name.into(), realm: self.active_realm, state: State::Pending });
        self.value_bytes += bytes;
        // There are no callbacks between registration and replacing the private
        // slot. Failed registrations retain only bounded, charged metadata.
        self.store_property(owner, name.into(), Value::Undefined, Some((enumerable, true, true)))?;
        self.objects[owner].properties.get_mut(name).unwrap().value = StoredValue::DeferredNative(index);
        Ok(())
    }

    pub(super) fn materialize_native_property(&mut self, owner: usize, name: &str) -> Result<(), Fault> {
        let Some(StoredValue::DeferredNative(index)) = self.object(owner)?.properties.get(name).map(|property| &property.value) else { return Ok(()); };
        let function = self.materialize_native_identity(*index)?;
        // Preserve descriptor flags/order when realizing a conceptual data value.
        self.objects[owner].properties.get_mut(name).unwrap().value = StoredValue::Value(Value::Function(function));
        Ok(())
    }

    fn materialize_native_identity(&mut self,index:usize)->Result<usize,Fault>{
        let function = match self.deferred_native_methods[index].state {
            State::Ready(function) => function,
            State::Failed => return Err(Fault::new("Native method initialization is unavailable after an earlier bounded failure.")),
            State::Pending => {
                let method = &self.deferred_native_methods[index];
                let (definition, realm, bytes) = (method.definition, method.realm, method.name.len());
                self.work(bytes)?;
                let name = self.deferred_native_methods[index].name.clone();
                let result = self.with_intrinsic_realm(realm, |runtime| match definition {
                    Definition::Native(builtin)=>runtime.native_function(builtin,&name),
                    Definition::ParamConstructor{window,prototype,parent}=>{
                        let allocation=runtime.allocate_host_function_with_length(window,"HTMLParamElement",0).map_err(Fault::new)?;
                        if let Some(Function::Host{constructible,illegal_interface_constructor,..})=runtime.functions.get_mut(allocation.id){
                            *constructible=true;
                            *illegal_interface_constructor=true;
                        }
                        runtime.objects[runtime.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent));
                        runtime.store_property(runtime.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                        Ok(Value::Function(allocation.id))
                    },
                    Definition::DataListConstructor{window,prototype,parent}=>{
                        let allocation=runtime.allocate_host_function_with_length(window,"HTMLDataListElement",0).map_err(Fault::new)?;
                        if let Some(Function::Host{constructible,illegal_interface_constructor,..})=runtime.functions.get_mut(allocation.id){
                            *constructible=true;
                            *illegal_interface_constructor=true;
                        }
                        runtime.objects[runtime.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent));
                        runtime.store_property(runtime.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                        Ok(Value::Function(allocation.id))
                    },
                    Definition::OptionConstructor{window,prototype,parent}=>{
                        let allocation=runtime.allocate_host_function_with_length(window,"HTMLOptionElement",0).map_err(Fault::new)?;
                        if let Some(Function::Host{constructible,illegal_interface_constructor,..})=runtime.functions.get_mut(allocation.id){
                            *constructible=true;
                            *illegal_interface_constructor=true;
                        }
                        runtime.objects[runtime.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent));
                        runtime.store_property(runtime.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                        Ok(Value::Function(allocation.id))
                    },
                    Definition::OptGroupConstructor{window,prototype,parent}=>{
                        let allocation=runtime.allocate_host_function_with_length(window,"HTMLOptGroupElement",0).map_err(Fault::new)?;
                        if let Some(Function::Host{constructible,illegal_interface_constructor,..})=runtime.functions.get_mut(allocation.id){
                            *constructible=true;
                            *illegal_interface_constructor=true;
                        }
                        runtime.objects[runtime.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent));
                        runtime.store_property(runtime.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                        Ok(Value::Function(allocation.id))
                    },
                    Definition::Host{window,operation,length}=>{
                        let method=runtime.allocate_host_function_with_length(window,operation,length).map_err(Fault::new)?;
                        if let Some(Function::Host{receiver_required,..})=runtime.functions.get_mut(method.id){*receiver_required=true;}
                        runtime.store_property(runtime.function_properties[method.id],"name".into(),Value::String(name.clone()),Some((false,false,true)))?;
                        Ok(Value::Function(method.id))
                    }
                });
                match result {
                    Ok(Value::Function(function)) => {
                        self.deferred_native_methods[index].state = State::Ready(function);
                        function
                    }
                    Ok(_) => unreachable!(),
                    Err(error) => { self.deferred_native_methods[index].state = State::Failed; return Err(error); }
                }
            }
        };
        Ok(function)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn param_runtime()->(Runtime,usize){
        let mut runtime=Runtime::for_capacity_test();
        runtime.define_global("window",Value::Host(77)).unwrap();
        runtime.bind_browser_global(77).unwrap();
        let prototype=runtime.new_object(None).unwrap();
        let Value::Function(parent)=runtime.native_function(Builtin::IsNaN,"parent").unwrap() else{unreachable!()};
        runtime.put_binding(runtime.global_environment,"paramPrototype".into(),Some(Value::Object(prototype)),Declaration::Var,false).unwrap();
        let before=runtime.functions.len();
        runtime.defer_param_constructor(77,prototype,parent).unwrap();
        runtime.global_attributes.insert("HTMLParamElement".into(),(false,true,true));
        assert_eq!(runtime.functions.len(),before);
        (runtime,prototype)
    }

    #[test]
    fn param_name_constructor_prototype_first_freeze_preserves_one_identity(){
        let(mut runtime,prototype)=param_runtime();
        runtime.execute("var freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor;freeze(paramPrototype);",&mut NoHost).unwrap();
        let before=runtime.functions.len();
        runtime.execute("var C=paramPrototype.constructor;if(C!==HTMLParamElement||C!==window.HTMLParamElement||C.prototype!==paramPrototype||C.name!=='HTMLParamElement'||C.length!==0)throw 'one identity';var d=inspect(paramPrototype,'constructor'),g=inspect(window,'HTMLParamElement'),p=inspect(C,'prototype');if(d.value!==C||d.get!==undefined||d.enumerable||d.writable||d.configurable||g.value!==C||g.get!==undefined||g.enumerable||!g.writable||!g.configurable||p.value!==paramPrototype||p.writable||p.configurable||p.enumerable)throw 'data flags';",&mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(),before+1);
        assert!(matches!(runtime.own_property(prototype,"constructor").unwrap(),Value::Function(_)));
    }

    #[test]
    fn param_name_constructor_global_first_override_delete_never_resurrect(){
        for change in ["paramPrototype.constructor=17;","delete paramPrototype.constructor;"]{
            let(mut runtime,_)=param_runtime();
            runtime.execute(change,&mut NoHost).unwrap();
            runtime.execute("var C=HTMLParamElement;if(C.prototype!==paramPrototype||paramPrototype.constructor===C)throw 'prototype mutation';window.HTMLParamElement=19;if(HTMLParamElement!==19)throw 'global override';delete window.HTMLParamElement;var names=Object.getOwnPropertyNames(window);if(typeof HTMLParamElement!=='undefined'||names.indexOf('HTMLParamElement')>=0||paramPrototype.constructor===C)throw 'deletion resurrected';",&mut NoHost).unwrap();
        }
        let(mut runtime,_)=param_runtime();
        runtime.execute("var C=HTMLParamElement;if(paramPrototype.constructor!==C)throw 'global first';",&mut NoHost).unwrap();
    }

    #[test]
    fn param_name_constructor_failure_is_bounded_across_both_observation_routes(){
        let(mut runtime,prototype)=param_runtime();
        while runtime.functions.len()<MAX_FUNCTIONS{runtime.native_function(Builtin::IsNaN,"fixture").unwrap();}
        let before=(runtime.functions.len(),runtime.objects.len(),runtime.bindings);
        assert!(runtime.ensure_param_constructor().unwrap_err().display().contains("Function limit reached"));
        runtime.begin_job();
        assert!(runtime.materialize_native_property(prototype,"constructor").unwrap_err().display().contains("earlier bounded failure"));
        assert!(runtime.ensure_param_constructor().unwrap_err().display().contains("earlier bounded failure"));
        assert_eq!((runtime.functions.len(),runtime.objects.len(),runtime.bindings),before);
    }

    fn datalist_runtime()->(Runtime,usize){
        let mut runtime=Runtime::for_capacity_test();
        runtime.define_global("window",Value::Host(77)).unwrap();
        runtime.bind_browser_global(77).unwrap();
        let prototype=runtime.new_object(None).unwrap();
        let Value::Function(parent)=runtime.native_function(Builtin::IsNaN,"parent").unwrap() else{unreachable!()};
        runtime.put_binding(runtime.global_environment,"datalistPrototype".into(),Some(Value::Object(prototype)),Declaration::Var,false).unwrap();
        let before=runtime.functions.len();
        runtime.defer_datalist_constructor(77,prototype,parent).unwrap();
        runtime.global_attributes.insert("HTMLDataListElement".into(),(false,true,true));
        assert_eq!(runtime.functions.len(),before);
        (runtime,prototype)
    }

    #[test]
    fn datalist_interface_constructor_prototype_first_freeze_preserves_one_identity(){
        let(mut runtime,prototype)=datalist_runtime();
        runtime.execute("var freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor;freeze(datalistPrototype);",&mut NoHost).unwrap();
        let before=runtime.functions.len();
        runtime.execute("var C=datalistPrototype.constructor;if(C!==HTMLDataListElement||C!==window.HTMLDataListElement||C.prototype!==datalistPrototype||C.name!=='HTMLDataListElement'||C.length!==0)throw 'one identity';var d=inspect(datalistPrototype,'constructor'),g=inspect(window,'HTMLDataListElement'),p=inspect(C,'prototype');if(d.value!==C||d.get!==undefined||d.enumerable||d.writable||d.configurable||g.value!==C||g.get!==undefined||g.enumerable||!g.writable||!g.configurable||p.value!==datalistPrototype||p.writable||p.configurable||p.enumerable)throw 'data flags';",&mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(),before+1);
        assert!(matches!(runtime.own_property(prototype,"constructor").unwrap(),Value::Function(_)));
    }

    #[test]
    fn datalist_interface_constructor_global_first_override_delete_never_resurrect(){
        for change in ["datalistPrototype.constructor=17;","delete datalistPrototype.constructor;"]{
            let(mut runtime,_)=datalist_runtime();
            runtime.execute(change,&mut NoHost).unwrap();
            runtime.execute("var C=HTMLDataListElement;if(C.prototype!==datalistPrototype||datalistPrototype.constructor===C)throw 'prototype mutation';window.HTMLDataListElement=19;if(HTMLDataListElement!==19)throw 'global override';delete window.HTMLDataListElement;var names=Object.getOwnPropertyNames(window);if(typeof HTMLDataListElement!=='undefined'||names.indexOf('HTMLDataListElement')>=0||datalistPrototype.constructor===C)throw 'deletion resurrected';",&mut NoHost).unwrap();
        }
        let(mut runtime,_)=datalist_runtime();
        runtime.execute("var C=HTMLDataListElement;if(datalistPrototype.constructor!==C)throw 'global first';",&mut NoHost).unwrap();
    }

    #[test]
    fn datalist_interface_constructor_failure_is_bounded_across_both_observation_routes(){
        let(mut runtime,prototype)=datalist_runtime();
        while runtime.functions.len()<MAX_FUNCTIONS{runtime.native_function(Builtin::IsNaN,"fixture").unwrap();}
        let before=(runtime.functions.len(),runtime.objects.len(),runtime.bindings);
        assert!(runtime.ensure_datalist_constructor().unwrap_err().display().contains("Function limit reached"));
        runtime.begin_job();
        assert!(runtime.materialize_native_property(prototype,"constructor").unwrap_err().display().contains("earlier bounded failure"));
        assert!(runtime.ensure_datalist_constructor().unwrap_err().display().contains("earlier bounded failure"));
        assert_eq!((runtime.functions.len(),runtime.objects.len(),runtime.bindings),before);
    }

    fn option_runtime()->(Runtime,usize){
        let mut runtime=Runtime::for_capacity_test();
        runtime.define_global("window",Value::Host(77)).unwrap();
        runtime.bind_browser_global(77).unwrap();
        let prototype=runtime.new_object(None).unwrap();
        let Value::Function(parent)=runtime.native_function(Builtin::IsNaN,"parent").unwrap() else{unreachable!()};
        runtime.put_binding(runtime.global_environment,"optionPrototype".into(),Some(Value::Object(prototype)),Declaration::Var,false).unwrap();
        let before=runtime.functions.len();
        runtime.defer_option_constructor(77,prototype,parent).unwrap();
        runtime.global_attributes.insert("HTMLOptionElement".into(),(false,true,true));
        assert_eq!(runtime.functions.len(),before);
        (runtime,prototype)
    }

    #[test]
    fn option_interface_constructor_prototype_first_freeze_preserves_one_identity(){
        let(mut runtime,prototype)=option_runtime();
        runtime.execute("var freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor;freeze(optionPrototype);",&mut NoHost).unwrap();
        let before=runtime.functions.len();
        runtime.execute("var C=optionPrototype.constructor;if(C!==HTMLOptionElement||C!==window.HTMLOptionElement||C.prototype!==optionPrototype||C.name!=='HTMLOptionElement'||C.length!==0)throw 'one identity';var d=inspect(optionPrototype,'constructor'),g=inspect(window,'HTMLOptionElement'),p=inspect(C,'prototype');if(d.value!==C||d.get!==undefined||d.enumerable||d.writable||d.configurable||g.value!==C||g.get!==undefined||g.enumerable||!g.writable||!g.configurable||p.value!==optionPrototype||p.writable||p.configurable||p.enumerable)throw 'data flags';",&mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(),before+1);
        assert!(matches!(runtime.own_property(prototype,"constructor").unwrap(),Value::Function(_)));
    }

    #[test]
    fn option_interface_constructor_global_first_override_delete_never_resurrect(){
        for change in ["optionPrototype.constructor=17;","delete optionPrototype.constructor;"]{
            let(mut runtime,_)=option_runtime();
            runtime.execute(change,&mut NoHost).unwrap();
            runtime.execute("var C=HTMLOptionElement;if(C.prototype!==optionPrototype||optionPrototype.constructor===C)throw 'prototype mutation';window.HTMLOptionElement=19;if(HTMLOptionElement!==19)throw 'global override';delete window.HTMLOptionElement;var names=Object.getOwnPropertyNames(window);if(typeof HTMLOptionElement!=='undefined'||names.indexOf('HTMLOptionElement')>=0||optionPrototype.constructor===C)throw 'deletion resurrected';",&mut NoHost).unwrap();
        }
        let(mut runtime,_)=option_runtime();
        runtime.execute("var C=HTMLOptionElement;if(optionPrototype.constructor!==C)throw 'global first';",&mut NoHost).unwrap();
    }

    #[test]
    fn option_interface_constructor_failure_is_bounded_across_both_observation_routes(){
        let(mut runtime,prototype)=option_runtime();
        while runtime.functions.len()<MAX_FUNCTIONS{runtime.native_function(Builtin::IsNaN,"fixture").unwrap();}
        let before=(runtime.functions.len(),runtime.objects.len(),runtime.bindings);
        assert!(runtime.ensure_option_constructor().unwrap_err().display().contains("Function limit reached"));
        runtime.begin_job();
        assert!(runtime.materialize_native_property(prototype,"constructor").unwrap_err().display().contains("earlier bounded failure"));
        assert!(runtime.ensure_option_constructor().unwrap_err().display().contains("earlier bounded failure"));
        assert_eq!((runtime.functions.len(),runtime.objects.len(),runtime.bindings),before);
    }

    fn optgroup_runtime()->(Runtime,usize){
        let mut runtime=Runtime::for_capacity_test();
        runtime.define_global("window",Value::Host(77)).unwrap();
        runtime.bind_browser_global(77).unwrap();
        let prototype=runtime.new_object(None).unwrap();
        let Value::Function(parent)=runtime.native_function(Builtin::IsNaN,"parent").unwrap() else{unreachable!()};
        runtime.put_binding(runtime.global_environment,"optgroupPrototype".into(),Some(Value::Object(prototype)),Declaration::Var,false).unwrap();
        let before=runtime.functions.len();
        runtime.defer_optgroup_constructor(77,prototype,parent).unwrap();
        runtime.global_attributes.insert("HTMLOptGroupElement".into(),(false,true,true));
        assert_eq!(runtime.functions.len(),before);
        (runtime,prototype)
    }

    #[test]
    fn optgroup_interface_constructor_prototype_first_freeze_preserves_one_identity(){
        let(mut runtime,prototype)=optgroup_runtime();
        runtime.execute("var freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor;freeze(optgroupPrototype);",&mut NoHost).unwrap();
        let before=runtime.functions.len();
        runtime.execute("var C=optgroupPrototype.constructor;if(C!==HTMLOptGroupElement||C!==window.HTMLOptGroupElement||C.prototype!==optgroupPrototype||C.name!=='HTMLOptGroupElement'||C.length!==0)throw 'one identity';var d=inspect(optgroupPrototype,'constructor'),g=inspect(window,'HTMLOptGroupElement'),p=inspect(C,'prototype');if(d.value!==C||d.get!==undefined||d.enumerable||d.writable||d.configurable||g.value!==C||g.get!==undefined||g.enumerable||!g.writable||!g.configurable||p.value!==optgroupPrototype||p.writable||p.configurable||p.enumerable)throw 'data flags';",&mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(),before+1);
        assert!(matches!(runtime.own_property(prototype,"constructor").unwrap(),Value::Function(_)));
    }

    #[test]
    fn optgroup_interface_constructor_global_first_override_delete_never_resurrect(){
        for change in ["optgroupPrototype.constructor=17;","delete optgroupPrototype.constructor;"]{
            let(mut runtime,_)=optgroup_runtime();
            runtime.execute(change,&mut NoHost).unwrap();
            runtime.execute("var C=HTMLOptGroupElement;if(C.prototype!==optgroupPrototype||optgroupPrototype.constructor===C)throw 'prototype mutation';window.HTMLOptGroupElement=19;if(HTMLOptGroupElement!==19)throw 'global override';delete window.HTMLOptGroupElement;var names=Object.getOwnPropertyNames(window);if(typeof HTMLOptGroupElement!=='undefined'||names.indexOf('HTMLOptGroupElement')>=0||optgroupPrototype.constructor===C)throw 'deletion resurrected';",&mut NoHost).unwrap();
        }
        let(mut runtime,_)=optgroup_runtime();
        runtime.execute("var C=HTMLOptGroupElement;if(optgroupPrototype.constructor!==C)throw 'global first';",&mut NoHost).unwrap();
    }

    #[test]
    fn optgroup_interface_constructor_failure_is_bounded_across_both_observation_routes(){
        let(mut runtime,prototype)=optgroup_runtime();
        while runtime.functions.len()<MAX_FUNCTIONS{runtime.native_function(Builtin::IsNaN,"fixture").unwrap();}
        let before=(runtime.functions.len(),runtime.objects.len(),runtime.bindings);
        assert!(runtime.ensure_optgroup_constructor().unwrap_err().display().contains("Function limit reached"));
        runtime.begin_job();
        assert!(runtime.materialize_native_property(prototype,"constructor").unwrap_err().display().contains("earlier bounded failure"));
        assert!(runtime.ensure_optgroup_constructor().unwrap_err().display().contains("earlier bounded failure"));
        assert_eq!((runtime.functions.len(),runtime.objects.len(),runtime.bindings),before);
    }

    struct NoHost;
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected native get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected native set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected native call".into()) }
    }


    #[test]
    fn host_data_method_preserves_flags_identity_and_no_eager_callable() {
        let mut runtime=Runtime::for_capacity_test();
        runtime.execute("var freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor;",&mut NoHost).unwrap();
        let owner=runtime.new_object(None).unwrap();
        runtime.put_binding(runtime.global_environment,"pendingHost".into(),Some(Value::Object(owner)),Declaration::Var,false).unwrap();
        let count=runtime.functions.len();
        runtime.deferred_host_method(owner,77,"check","\0test.check",0).unwrap();
        runtime.execute("if(!('check' in pendingHost))throw 'presence';freeze(pendingHost);",&mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(),count);
        runtime.execute("var a=pendingHost.check,b=inspect(pendingHost,'check');if(a!==pendingHost.check||a!==b.value||b.get!==undefined||!b.enumerable||b.writable||b.configurable||a.name!=='check'||a.length!==0)throw 'host data descriptor';var caught=false;try{new a();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'method constructor';",&mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(),count+1);
    }

    #[test]
    fn host_data_method_failure_is_bounded_and_never_retries_or_resurrects() {
        let mut runtime=Runtime::for_capacity_test();
        let owner=runtime.new_object(None).unwrap();
        runtime.deferred_host_method(owner,77,"check","\0test.check",0).unwrap();
        while runtime.functions.len()<MAX_FUNCTIONS{runtime.native_function(Builtin::IsNaN,"fixture").unwrap();}
        let before=(runtime.functions.len(),runtime.objects.len());
        assert!(runtime.materialize_native_property(owner,"check").unwrap_err().display().contains("Function limit reached"));
        runtime.begin_job();
        assert!(runtime.materialize_native_property(owner,"check").unwrap_err().display().contains("earlier bounded failure"));
        assert_eq!((runtime.functions.len(),runtime.objects.len()),before);
        runtime.set_property(owner,"check".into(),Value::Number(17.0)).unwrap();
        assert_eq!(runtime.own_property(owner,"check").unwrap().to_number(),17.0);
        runtime.remove_property(owner,"check").unwrap();
        assert!(matches!(runtime.own_property(owner,"check").unwrap(),Value::Undefined));
        assert_eq!((runtime.functions.len(),runtime.objects.len()),before);
    }

    #[test]
    fn first_method_observation_creates_one_stable_data_value() {
        let mut runtime = Runtime::for_capacity_test();
        println!("Demand-driven baseline: {} callable identities; {} bounded native method records.", runtime.functions.len(), runtime.deferred_native_methods.len());
        runtime.execute("var inspect=Object.getOwnPropertyDescriptor;var p=Array.prototype;", &mut NoHost).unwrap();
        let functions = runtime.functions.len();
        runtime.execute("var a=p.map,b=p.map,d=inspect(p,'map');if(a!==b||a!==d.value||d.get!==undefined||d.enumerable||!d.configurable||!d.writable||a.name!=='map'||a.length!==1)throw 'native data identity';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), functions + 1);
    }

    #[test]
    fn names_presence_freeze_and_flag_updates_do_not_create_callables() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var names=Object.getOwnPropertyNames,freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor,define=Object.defineProperty;var p=Array.prototype;", &mut NoHost).unwrap();
        let functions = runtime.functions.len();
        runtime.execute("if(!('map' in p)||names(p).indexOf('map')<0)throw 'method presence';define(p,'map',{writable:false});freeze(p);", &mut NoHost).unwrap();
        // indexOf is the only observed method during the presence check.
        assert_eq!(runtime.functions.len(), functions + 1);
        let functions = runtime.functions.len();
        runtime.execute("var method=p.map,d=inspect(p,'map');if(d.value!==method||d.writable||d.configurable||d.enumerable)throw 'frozen data';define(p,'map',{value:method});var blocked=false;try{define(p,'map',{value:0});}catch(e){blocked=e.name==='TypeError';}if(!blocked)throw 'nonconfigurable value';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), functions + 1);
    }

    #[test]
    fn replacement_and_deletion_before_observation_do_not_resurrect_methods() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var inspect=Object.getOwnPropertyDescriptor,define=Object.defineProperty;var p=Array.prototype;", &mut NoHost).unwrap();
        let functions = runtime.functions.len();
        runtime.execute("p.flat=17;if(p.flat!==17)throw 'replacement';delete p.flat;if(p.flat!==undefined||inspect(p,'flat')!==undefined)throw 'resurrection';define(p,'map',{get:undefined,configurable:true});if(p.map!==undefined)throw 'accessor replacement';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), functions);
    }

    #[test]
    fn aliases_preserve_identity_and_first_read_under_one_mib_is_bounded() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::for_capacity_test();
            runtime.execute("if(Array.prototype.values!==Array.prototype[Symbol.iterator]||Set.prototype.keys!==Set.prototype.values||Date.prototype.toGMTString!==Date.prototype.toUTCString)throw 'native alias';function nested(n){if(n>0)return nested(n-1);return String.prototype.trim;}var a=nested(8);if(a!==String.prototype.trim||a.call(' x ')!=='x')throw 'late method';", &mut NoHost).unwrap();
        }).unwrap().join().unwrap();
    }

    #[test]
    fn failed_method_realization_never_exposes_a_placeholder_or_retries_allocation() {
        let mut runtime = Runtime::for_capacity_test();
        while runtime.functions.len() < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_deadline);
        let error = runtime.execute("Array.prototype.map;", &mut NoHost).unwrap_err();
        assert!(error.contains("Function limit reached"), "{error}");
        assert_eq!(runtime.functions.len(), before.0);
        assert_eq!(runtime.objects.len(), before.1);
        assert_eq!(runtime.job_deadline, before.3);
        runtime.begin_job();
        let error = runtime.execute("Array.prototype.map;", &mut NoHost).unwrap_err();
        assert!(error.contains("earlier bounded failure"), "{error}");
        assert_eq!(runtime.functions.len(), before.0);
        assert_eq!(runtime.objects.len(), before.1);
        assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
        runtime.execute("Array.prototype.map=17;if(Array.prototype.map!==17)throw 'failed slot replacement';delete Array.prototype.map;if(Array.prototype.map!==undefined)throw 'failed slot resurrection';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), before.0);
    }

    #[test]
    fn frozen_method_rejects_an_unrelated_value_without_needing_a_callable_slot() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var freeze=Object.freeze,define=Object.defineProperty;freeze(Array.prototype);", &mut NoHost).unwrap();
        while runtime.functions.len() < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        let functions = runtime.functions.len();
        assert_eq!(runtime.execute("var caught=false;try{define(Array.prototype,'map',{value:7});}catch(e){caught=e.name==='TypeError';}caught;", &mut NoHost).unwrap().to_text(), "true");
        assert_eq!(runtime.functions.len(), functions);
    }

    #[test]
    fn observing_all_pending_methods_once_matches_the_eager_surface_without_repeated_allocations() {
        let mut runtime = Runtime::for_capacity_test();
        let before = runtime.functions.len();
        let pending: Vec<_> = runtime.objects.iter().enumerate().flat_map(|(owner, object)| {
            object.properties.iter().filter_map(move |(name, property)| matches!(property.value, StoredValue::DeferredNative(_))
                .then(|| (owner, name.as_scalar().expect("native method names are scalar").to_owned())))
        }).collect();
        let mut identities = Vec::new();
        for (owner, name) in &pending {
            let Value::Function(function) = runtime.own_property(*owner, name).unwrap() else { panic!("real native callable") };
            identities.push(function);
        }
        let after = runtime.functions.len();
        assert_eq!(after, before + pending.len());
        for ((owner, name), expected) in pending.iter().zip(identities) {
            assert!(matches!(runtime.own_property(*owner, name).unwrap(), Value::Function(function) if function == expected));
        }
        assert_eq!(runtime.functions.len(), after);
        println!("Native surface allocation: bootstrap={before}; pending-data-methods={}; after-observing-all={after}; repeated-read-additions=0.", pending.len());
    }

    #[test]
    fn original_sparse_method_workload_allocates_only_observed_identities() {
        let mut runtime = Runtime::for_capacity_test();
        let owner = runtime.new_object(None).unwrap();
        let before = runtime.functions.len();
        let retained = runtime.value_bytes;
        for index in 0..64 { runtime.prototype_method(owner, &format!("original{index}"), Builtin::IsNaN).unwrap(); }
        assert_eq!(runtime.functions.len(), before);
        assert!(runtime.value_bytes >= retained + 64 * std::mem::size_of::<NativeMethod>());
        let mut ids = Vec::new();
        for index in 0..3 {
            let Value::Function(function) = runtime.own_property(owner, &format!("original{index}")).unwrap() else { panic!("callable") };
            ids.push(function);
        }
        assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2]);
        assert_eq!(runtime.functions.len(), before + 3);
        runtime.set_property(owner, "original3".into(), Value::Number(17.0)).unwrap();
        runtime.remove_property(owner, "original4").unwrap();
        assert_eq!(runtime.own_property(owner, "original3").unwrap().to_number(), 17.0);
        assert!(matches!(runtime.own_property(owner, "original4").unwrap(), Value::Undefined));
        assert_eq!(runtime.functions.len(), before + 3);
        println!("Original sparse workload: registered=64; observed=3; callable-additions=3; unread replacement/deletion additions=0.");
    }
}
