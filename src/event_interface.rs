//! Genuine Event/UIEvent/MouseEvent/KeyboardEvent interfaces over native records.
//! Constructed untrusted state stays separate from trusted native input.
use super::{Accessors, Fault, Function, Host, Ordinary, PropertyKey, Runtime, Value};

pub(super) const TRUSTED_GETTER_KEY: &str = "\0Event.isTrusted.getter";
pub(super) const PATH_METHOD: &str = "\0Event.composedPath";

const FIELDS: &[(&str, &str)] = &[
    ("type", "\0Event.get.type"),
    ("target", "\0Event.get.target"),
    ("srcElement", "\0Event.get.srcElement"),
    ("currentTarget", "\0Event.get.currentTarget"),
    ("eventPhase", "\0Event.get.eventPhase"),
    ("cancelBubble", "\0Event.get.cancelBubble"),
    ("bubbles", "\0Event.get.bubbles"),
    ("cancelable", "\0Event.get.cancelable"),
    ("returnValue", "\0Event.get.returnValue"),
    ("defaultPrevented", "\0Event.get.defaultPrevented"),
    ("composed", "\0Event.get.composed"),
    ("timeStamp", "\0Event.get.timeStamp"),
];

impl Runtime {
    #[cfg(test)]
    pub(crate) fn event_property_allowance_for_test(&mut self,limit:usize)->usize {
        std::mem::replace(&mut self.limits.heap.object_properties,limit)
    }
    fn event_native_function(&mut self, window:u32, operation:&'static str, name:&str, length:usize)
        -> Result<Value,Fault>
    {
        let allocation=self.allocate_host_function_with_length(window,operation,length).map_err(Fault::new)?;
        if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(allocation.id) {
            *receiver_required=true;
        }
        self.store_property(self.function_properties[allocation.id],"name".into(),
            Value::String(name.into()),Some((false,false,true)))?;
        Ok(Value::Function(allocation.id))
    }

    pub fn define_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let allocation=self.allocate_host_function_with_length(window,"Event",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {
                *constructible=true;
            }
            let constructor=Value::Function(allocation.id);
            let prototype=self.new_object(None)?;
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),
                Value::Object(prototype),Some((false,false,false)))?;
            for &(name,operation) in FIELDS {
                let Value::Function(get)=self.event_native_function(window,operation,&format!("get {name}"),0)?
                    else {unreachable!()};
                let set=match name {
                    "cancelBubble"=>Some("\0Event.set.cancelBubble"),
                    "returnValue"=>Some("\0Event.set.returnValue"),
                    _=>None,
                }.map(|operation|self.event_native_function(window,operation,&format!("set {name}"),1))
                    .transpose()?.map(|value|{let Value::Function(id)=value else {unreachable!()};id});
                self.store_accessor_property(prototype,name.into(),Accessors {get:Some(get),set},(true,true))?;
            }
            for (name,operation,length) in [
                ("composedPath",PATH_METHOD,0),
                ("stopPropagation","stopPropagation",0),
                ("stopImmediatePropagation","stopImmediatePropagation",0),
                ("preventDefault","preventDefault",0),
                ("initEvent","initEvent",1),
            ] {
                let function=self.event_native_function(window,operation,name,length)?;
                self.store_property(prototype,name.into(),function,Some((true,true,true)))?;
            }
            for (name,value) in [("NONE",0),("CAPTURING_PHASE",1),("AT_TARGET",2),("BUBBLING_PHASE",3)] {
                for object in [prototype,self.function_properties[allocation.id]] {
                    self.store_property(object,name.into(),Value::Number(f64::from(value)),Some((true,false,false)))?;
                }
            }
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),
                Value::String("Event".into()),Some((false,false,true)))?;
            // Prebuilt owner-realm getter, rooted in the existing native-function
            // cache but absent from both public constructor and prototype.
            let trusted=self.event_native_function(window,"\0Event.get.isTrusted","get isTrusted",0)?;
            self.host_method_values.insert((window,TRUSTED_GETTER_KEY),trusted);
            self.native_constructor_prototypes.insert((window,"Event"),prototype);
            self.put_binding(self.global_environment,"Event".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("Event".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub fn define_custom_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let parent=*self.native_constructor_prototypes.get(&(window,"Event"))
                .ok_or_else(||Fault::new("CustomEvent requires the canonical Event interface."))?;
            let parent_constructor=self.objects[parent].properties.get("constructor")
                .ok_or_else(||Fault::new("Canonical Event constructor is unavailable."))?.value.as_value()?.clone();
            let Value::Function(parent_constructor)=parent_constructor else {
                return Err(Fault::new("Canonical Event constructor is invalid."));
            };
            let allocation=self.allocate_host_function_with_length(window,"CustomEvent",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
            let prototype=self.new_object(None)?;
            self.objects[prototype].prototype=Some(Ordinary::Object(parent));
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),
                Value::Object(prototype),Some((false,false,false)))?;
            let Value::Function(get)=self.event_native_function(window,"\0CustomEvent.get.detail","get detail",0)?
                else {unreachable!()};
            self.store_accessor_property(prototype,"detail".into(),Accessors {get:Some(get),set:None},(true,true))?;
            let initializer=self.event_native_function(window,"initCustomEvent","initCustomEvent",1)?;
            self.store_property(prototype,"initCustomEvent".into(),initializer,Some((true,true,true)))?;
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),
                Value::String("CustomEvent".into()),Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window,"CustomEvent"),prototype);
            self.put_binding(self.global_environment,"CustomEvent".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("CustomEvent".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub fn define_ui_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let parent=*self.native_constructor_prototypes.get(&(window,"Event"))
                .ok_or_else(||Fault::new("UIEvent requires the canonical Event interface."))?;
            let parent_constructor=self.objects[parent].properties.get("constructor")
                .ok_or_else(||Fault::new("Canonical Event constructor is unavailable."))?.value.as_value()?.clone();
            let Value::Function(parent_constructor)=parent_constructor else {
                return Err(Fault::new("Canonical Event constructor is invalid."));
            };
            let allocation=self.allocate_host_function_with_length(window,"UIEvent",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
            let prototype=self.new_object(None)?;
            self.objects[prototype].prototype=Some(Ordinary::Object(parent));
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),
                Value::Object(prototype),Some((false,false,false)))?;
            for (name,operation) in [("view","\0UIEvent.get.view"),("detail","\0UIEvent.get.detail"),("which","\0UIEvent.get.which")] {
                let Value::Function(get)=self.event_native_function(window,operation,&format!("get {name}"),0)?
                    else {unreachable!()};
                self.store_accessor_property(prototype,name.into(),Accessors {get:Some(get),set:None},(true,true))?;
            }
            let initializer=self.event_native_function(window,"initUIEvent","initUIEvent",1)?;
            self.store_property(prototype,"initUIEvent".into(),initializer,Some((true,true,true)))?;
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),
                Value::String("UIEvent".into()),Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window,"UIEvent"),prototype);
            self.put_binding(self.global_environment,"UIEvent".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("UIEvent".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub fn define_mouse_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let parent=*self.native_constructor_prototypes.get(&(window,"UIEvent"))
                .ok_or_else(||Fault::new("MouseEvent requires the canonical UIEvent interface."))?;
            let parent_constructor=self.objects[parent].properties.get("constructor")
                .ok_or_else(||Fault::new("Canonical UIEvent constructor is unavailable."))?.value.as_value()?.clone();
            let Value::Function(parent_constructor)=parent_constructor else {
                return Err(Fault::new("Canonical UIEvent constructor is invalid."));
            };
            let allocation=self.allocate_host_function_with_length(window,"MouseEvent",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
            let prototype=self.new_object(None)?;
            self.objects[prototype].prototype=Some(Ordinary::Object(parent));
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),
                Value::Object(prototype),Some((false,false,false)))?;
            for (name,operation) in [
                ("screenX","\0MouseEvent.get.screenX"),("screenY","\0MouseEvent.get.screenY"),
                ("clientX","\0MouseEvent.get.clientX"),("clientY","\0MouseEvent.get.clientY"),
                ("x","\0MouseEvent.get.x"),("y","\0MouseEvent.get.y"),
                ("pageX","\0MouseEvent.get.pageX"),("pageY","\0MouseEvent.get.pageY"),
                ("offsetX","\0MouseEvent.get.offsetX"),("offsetY","\0MouseEvent.get.offsetY"),
                ("layerX","\0MouseEvent.get.layerX"),("layerY","\0MouseEvent.get.layerY"),
                ("ctrlKey","\0MouseEvent.get.ctrlKey"),("altKey","\0MouseEvent.get.altKey"),
                ("shiftKey","\0MouseEvent.get.shiftKey"),("metaKey","\0MouseEvent.get.metaKey"),
                ("button","\0MouseEvent.get.button"),("buttons","\0MouseEvent.get.buttons"),
                ("relatedTarget","\0MouseEvent.get.relatedTarget"),
            ] {
                let Value::Function(get)=self.event_native_function(window,operation,&format!("get {name}"),0)?
                    else {unreachable!()};
                self.store_accessor_property(prototype,name.into(),Accessors {get:Some(get),set:None},(true,true))?;
            }
            for method in ["initMouseEvent","getModifierState"] {
                let function=self.event_native_function(window,method,method,1)?;
                self.store_property(prototype,method.into(),function,Some((true,true,true)))?;
            }
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),
                Value::String("MouseEvent".into()),Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window,"MouseEvent"),prototype);
            self.put_binding(self.global_environment,"MouseEvent".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("MouseEvent".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub fn define_keyboard_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let parent=*self.native_constructor_prototypes.get(&(window,"UIEvent"))
                .ok_or_else(||Fault::new("KeyboardEvent requires the canonical UIEvent interface."))?;
            let parent_constructor=self.objects[parent].properties.get("constructor")
                .ok_or_else(||Fault::new("Canonical UIEvent constructor is unavailable."))?.value.as_value()?.clone();
            let Value::Function(parent_constructor)=parent_constructor else {return Err(Fault::new("Canonical UIEvent constructor is invalid."));};
            let allocation=self.allocate_host_function_with_length(window,"KeyboardEvent",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
            let prototype=self.new_object(None)?;
            self.objects[prototype].prototype=Some(Ordinary::Object(parent));
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
            for (name,operation) in [
                ("key","\0KeyboardEvent.get.key"),("code","\0KeyboardEvent.get.code"),
                ("location","\0KeyboardEvent.get.location"),("ctrlKey","\0KeyboardEvent.get.ctrlKey"),
                ("shiftKey","\0KeyboardEvent.get.shiftKey"),("altKey","\0KeyboardEvent.get.altKey"),
                ("metaKey","\0KeyboardEvent.get.metaKey"),("repeat","\0KeyboardEvent.get.repeat"),
                ("isComposing","\0KeyboardEvent.get.isComposing"),("keyCode","\0KeyboardEvent.get.keyCode"),
                ("charCode","\0KeyboardEvent.get.charCode"),
            ] {
                let Value::Function(get)=self.event_native_function(window,operation,&format!("get {name}"),0)? else {unreachable!()};
                self.store_accessor_property(prototype,name.into(),Accessors {get:Some(get),set:None},(true,true))?;
            }
            let modifier=self.event_native_function(window,"\0KeyboardEvent.getModifierState","getModifierState",1)?;
            self.store_property(prototype,"getModifierState".into(),modifier,Some((true,true,true)))?;
            for (name,value) in [("DOM_KEY_LOCATION_STANDARD",0),("DOM_KEY_LOCATION_LEFT",1),("DOM_KEY_LOCATION_RIGHT",2),("DOM_KEY_LOCATION_NUMPAD",3)] {
                for object in [prototype,self.function_properties[allocation.id]] {
                    self.store_property(object,name.into(),Value::Number(f64::from(value)),Some((true,false,false)))?;
                }
            }
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("KeyboardEvent".into()),Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window,"KeyboardEvent"),prototype);
            self.put_binding(self.global_environment,"KeyboardEvent".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("KeyboardEvent".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);result.map_err(Fault::display)
    }

    pub fn define_focus_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let parent=*self.native_constructor_prototypes.get(&(window,"UIEvent"))
                .ok_or_else(||Fault::new("FocusEvent requires the canonical UIEvent interface."))?;
            let parent_constructor=self.objects[parent].properties.get("constructor")
                .ok_or_else(||Fault::new("Canonical UIEvent constructor is unavailable."))?.value.as_value()?.clone();
            let Value::Function(parent_constructor)=parent_constructor else {return Err(Fault::new("Canonical UIEvent constructor is invalid."));};
            let allocation=self.allocate_host_function_with_length(window,"FocusEvent",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
            let prototype=self.new_object(None)?;
            self.objects[prototype].prototype=Some(Ordinary::Object(parent));
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
            let Value::Function(get)=self.event_native_function(window,"\0FocusEvent.get.relatedTarget","get relatedTarget",0)? else {unreachable!()};
            self.store_accessor_property(prototype,"relatedTarget".into(),Accessors {get:Some(get),set:None},(true,true))?;
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("FocusEvent".into()),Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window,"FocusEvent"),prototype);
            self.put_binding(self.global_environment,"FocusEvent".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("FocusEvent".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);result.map_err(Fault::display)
    }

    pub fn define_progress_event_interface(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let parent=*self.native_constructor_prototypes.get(&(window,"Event"))
                .ok_or_else(||Fault::new("ProgressEvent requires the canonical Event interface."))?;
            let parent_constructor=self.objects[parent].properties.get("constructor")
                .ok_or_else(||Fault::new("Canonical Event constructor is unavailable."))?.value.as_value()?.clone();
            let Value::Function(parent_constructor)=parent_constructor else {return Err(Fault::new("Canonical Event constructor is invalid."));};
            let allocation=self.allocate_host_function_with_length(window,"ProgressEvent",1).map_err(Fault::new)?;
            if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
            let constructor=Value::Function(allocation.id);
            self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
            let prototype=self.new_object(None)?;
            self.objects[prototype].prototype=Some(Ordinary::Object(parent));
            self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
            self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
            for (name,operation) in [("lengthComputable","\0ProgressEvent.get.lengthComputable"),("loaded","\0ProgressEvent.get.loaded"),("total","\0ProgressEvent.get.total")] {
                let Value::Function(get)=self.event_native_function(window,operation,&format!("get {name}"),0)? else {unreachable!()};
                self.store_accessor_property(prototype,name.into(),Accessors {get:Some(get),set:None},(true,true))?;
            }
            self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("ProgressEvent".into()),Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window,"ProgressEvent"),prototype);
            self.put_binding(self.global_environment,"ProgressEvent".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("ProgressEvent".into(),(false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);result.map_err(Fault::display)
    }

    pub(super) fn event_unforgeable(&mut self,object:usize,owner:u32)->Result<(),Fault> {
        let Some(Value::Function(get))=self.host_method_values.get(&(owner,TRUSTED_GETTER_KEY)) else {
            return Err(Fault::new("Canonical Event isTrusted getter is unavailable."));
        };
        let get=*get;
        self.store_accessor_property(object,"isTrusted".into(),
            Accessors {get:Some(get),set:None},(true,false))
    }

    pub(super) fn finish_new_event_result(&mut self,object:u32,method:&str,value:&Value,host:&mut dyn Host)
        ->Result<(),Fault>
    {
        let Value::Host(handle)=value else {return Ok(())};
        let Some(owner)=host.unpublished_base_event_owner(object,method,*handle) else {return Ok(())};
        let result=(|| {
            let realm=self.ensure_window_realm(owner,host)?;
            self.with_intrinsic_realm(realm,|runtime|runtime.ensure_native_expando(*handle,host,true))
        })();
        if let Err(fault)=result {
            host.rollback_unpublished_base_event(*handle).map_err(Fault::new)?;
            return Err(fault);
        }
        Ok(())
    }

    pub(super) fn event_path_array(&mut self,handle:u32,host:&mut dyn Host)->Result<Value,Fault> {
        let path=host.event_path_snapshot(handle).map_err(Fault::new)?;
        if path.len()>self.limits.heap.object_properties {
            return Err(Fault::new("Event path exceeds the array/property allowance."));
        }
        let output=Value::Object(self.new_object(Some(path.len()))?);
        for (index,handle) in path.into_iter().enumerate() {
            self.step()?;
            self.create_data_property(&output,PropertyKey::String(index.to_string()),Value::Host(handle),host)?;
        }
        Ok(output)
    }
}
