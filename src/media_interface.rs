//! Genuine finite media interfaces over the embedding's existing native player.
//! Interface identity does not manufacture another controller or decoder.
use super::{Accessors, Fault, Function, Ordinary, Runtime, Value};

impl Runtime {
    fn media_native_function(&mut self,window:u32,operation:&'static str,name:&str,length:usize)
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

    pub fn define_media_interfaces(&mut self,window:u32)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let mut parent=*self.native_constructor_prototypes.get(&(window,"HTMLElement"))
                .ok_or_else(||Fault::new("Media interfaces require the canonical HTMLElement interface."))?;
            let mut definitions=Vec::new();
            definitions.try_reserve(2).map_err(|_|Fault::new("Could not retain media interface definitions."))?;
            for name in ["HTMLMediaElement","HTMLVideoElement"] {
                let parent_constructor=self.objects[parent].properties.get("constructor")
                    .ok_or_else(||Fault::new("Canonical media parent constructor is unavailable."))?.value.as_value()?.clone();
                let Value::Function(parent_constructor)=parent_constructor else {
                    return Err(Fault::new("Canonical media parent constructor is invalid."));
                };
                let allocation=self.allocate_host_function_with_length(window,name,0).map_err(Fault::new)?;
                if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
                let constructor=Value::Function(allocation.id);
                self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_constructor));
                let prototype=self.new_object(None)?;
                self.objects[prototype].prototype=Some(Ordinary::Object(parent));
                self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
                self.store_property(self.function_properties[allocation.id],"prototype".into(),
                    Value::Object(prototype),Some((false,false,false)))?;
                let fields:&[(&'static str,&'static str,Option<&'static str>)]=if name=="HTMLMediaElement" {&[
                    ("src","\0Media.get.src",Some("\0Media.set.src")),
                    ("currentTime","\0Media.get.currentTime",Some("\0Media.set.currentTime")),
                    ("currentSrc","\0Media.get.currentSrc",None),
                    ("controls","\0Media.get.controls",None),
                    ("duration","\0Media.get.duration",None),
                    ("paused","\0Media.get.paused",None),
                    ("ended","\0Media.get.ended",None),
                    ("seeking","\0Media.get.seeking",None),
                    ("readyState","\0Media.get.readyState",None),
                    ("networkState","\0Media.get.networkState",None),
                ]} else {&[
                    ("videoWidth","\0Video.get.videoWidth",None),
                    ("videoHeight","\0Video.get.videoHeight",None),
                ]};
                for &(field,operation,setter) in fields {
                    let Value::Function(get)=self.media_native_function(window,operation,&format!("get {field}"),0)?
                        else {unreachable!()};
                    let set=setter.map(|operation|self.media_native_function(window,operation,&format!("set {field}"),1))
                        .transpose()?.map(|value|{let Value::Function(id)=value else {unreachable!()};id});
                    self.store_accessor_property(prototype,field.into(),Accessors {get:Some(get),set},(true,true))?;
                }
                if name=="HTMLMediaElement" {
                    for (method,operation) in [("load","\0Media.load"),("pause","\0Media.pause"),("play","\0Media.play")] {
                        let function=self.media_native_function(window,operation,method,0)?;
                        self.store_property(prototype,method.into(),function,Some((true,true,true)))?;
                    }
                    for (constant,value) in [
                        ("NETWORK_EMPTY",0),("NETWORK_IDLE",1),("NETWORK_LOADING",2),("NETWORK_NO_SOURCE",3),
                        ("HAVE_NOTHING",0),("HAVE_METADATA",1),("HAVE_CURRENT_DATA",2),
                        ("HAVE_FUTURE_DATA",3),("HAVE_ENOUGH_DATA",4),
                    ] {
                        for object in [prototype,self.function_properties[allocation.id]] {
                            self.store_property(object,constant.into(),Value::Number(f64::from(value)),Some((true,false,false)))?;
                        }
                    }
                }
                self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),
                    Value::String(name.into()),Some((false,false,true)))?;
                // Root each complete private prototype before constructing its
                // subclass. Globals are published only after both are complete.
                self.native_constructor_prototypes.insert((window,name),prototype);
                definitions.push((name,constructor));
                parent=prototype;
            }
            for (name,constructor) in definitions {
                self.put_binding(self.global_environment,name.into(),Some(constructor),super::Declaration::Var,false)?;
                self.global_attributes.insert(name.into(),(false,true,true));
            }
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }
}
