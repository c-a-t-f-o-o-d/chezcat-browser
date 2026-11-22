//! Bounded ordinary expandos on explicitly selected native objects and live
//! named-property reflection. Host handles never acquire arbitrary capability.

use super::{Accessors, Builtin, Fault, Function, Host, Ordinary, Runtime, Value, collections,
    ELEMENT_INNER_HTML_GET, ELEMENT_INNER_HTML_SET, ELEMENT_STYLE_GET, ELEMENT_STYLE_SET, SVG_STYLE_GET,SVG_STYLE_SET,
    CREATE_CDATA_SECTION, CREATE_PROCESSING_INSTRUCTION, DOCUMENT_ACTIVE_ELEMENT_GET, DOCUMENT_CURRENT_SCRIPT_GET, DOCUMENT_IMPORT_NODE, DOCUMENT_ADOPT_NODE,
    NODE_MUTATION_METHODS, NODE_TEXT_CONTENT_GET, NODE_TEXT_CONTENT_SET, NODE_TRAVERSAL_GETTERS, PARENT_NODE_GETTERS,
    HTML_SLOT_NAME_GET, HTML_SLOT_NAME_SET, HTML_TEMPLATE_CONTENT_GET, PROCESSING_INSTRUCTION_TARGET_GET, html_reflection_interface, aria_reflection_interface};

#[cfg(test)]
#[path = "host_document_property_limits_tests.rs"]
mod document_property_limits_tests;

impl Runtime {
    pub(crate) fn define_inert_document_interfaces(
        &mut self, window: u32, implementation_get: &'static str,
        create: &'static str, has_feature: &'static str,
    ) -> Result<(), String> {
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let document = self.native_constructor_prototypes[&(window, "Document")];
            self.define_readonly_node_getter(document, window, "implementation", implementation_get)?;
            self.define_readonly_node_getter(document,window,"doctype",super::DOCUMENT_DOCTYPE_GET)?;
            for name in ["DOMImplementation", "DocumentType", "XMLDocument"] {
                let allocation = self.allocate_host_function_with_length(window, name, 0).map_err(Fault::new)?;
                if let Some(Function::Host { constructible, illegal_interface_constructor, .. }) =
                    self.functions.get_mut(allocation.id)
                {
                    *constructible = true;
                    *illegal_interface_constructor = true;
                }
                let prototype = self.new_object(None)?;
                if name == "DocumentType"||name=="XMLDocument" {
                    let parent = self.native_constructor_prototypes[&(window, if name=="XMLDocument"{"Document"}else{"Node"})];
                    self.objects[prototype].prototype = Some(Ordinary::Object(parent));
                    if let Value::Function(parent_constructor) = self.objects[parent].properties["constructor"].value.as_value()? {
                        self.objects[self.function_properties[allocation.id]].prototype = Some(Ordinary::Function(*parent_constructor));
                    }
                    if name=="DocumentType"{for &(name,method) in super::DOCUMENT_TYPE_GETTERS {self.define_readonly_node_getter(prototype,window,name,method)?;}}
                } else {
                    for (name, operation,length) in [("createHTMLDocument", create,0), ("hasFeature", has_feature,0),("createDocumentType","\0DOMImplementation.createDocumentType",3),("createDocument","\0DOMImplementation.createDocument",2)] {
                        let method = self.allocate_host_function_with_length(window, operation, length).map_err(Fault::new)?;
                        if let Some(Function::Host { receiver_required, .. }) = self.functions.get_mut(method.id) {
                            *receiver_required = true;
                        }
                        self.store_property(self.function_properties[method.id], "name".into(),
                            Value::String(name.into()), Some((false, false, true)))?;
                        self.store_property(prototype, name.into(), Value::Function(method.id), Some((true, true, true)))?;
                    }
                }
                self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(),
                    Value::String(name.into()), Some((false, false, true)))?;
                self.put_binding(self.global_environment, name.into(), Some(Value::Function(allocation.id)),
                    super::Declaration::Var, false)?;
                self.global_attributes.insert(name.into(), (false, true, true));
                self.constructor_prototype(name, Value::Object(prototype))?;
                self.native_constructor_prototypes.insert((window, name), prototype);
            }
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub(crate) fn define_dom_parser_interface(&mut self, window: u32, parse: &'static str) -> Result<(), String> {
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let allocation = self.allocate_host_function_with_length(window, "DOMParser", 0).map_err(Fault::new)?;
            if let Some(Function::Host { constructible, .. }) = self.functions.get_mut(allocation.id) {
                *constructible = true;
            }
            let constructor = Value::Function(allocation.id);
            let prototype = self.new_object(None)?;
            self.store_property(self.function_properties[allocation.id], "prototype".into(), Value::Object(prototype), Some((false,false,false)))?;
            self.store_property(prototype, "constructor".into(), constructor.clone(), Some((false,true,true)))?;
            self.define_canvas_operation(prototype, window, "parseFromString", parse, 2)?;
            self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(), Value::String("DOMParser".into()), Some((false,false,true)))?;
            self.native_constructor_prototypes.insert((window, "DOMParser"), prototype);
            self.put_binding(self.global_environment, "DOMParser".into(), Some(constructor), super::Declaration::Var, false)?;
            self.global_attributes.insert("DOMParser".into(), (false,true,true));
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    #[cfg(test)]
    pub(crate) fn native_overlay_state_for_test(&self, handle:u32)->(Option<usize>,Option<u32>) {
        (self.host_objects.get(&handle).copied(),self.native_expando_owners.get(&handle).copied())
    }

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
    pub(crate) fn define_abort_interfaces(&mut self, window: u32) -> Result<(), String> {
        use super::abort_interface::*;
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let target = self.native_constructor_prototypes[&(window, "EventTarget")];
            for name in [CONTROLLER, SIGNAL_INTERFACE] {
                let allocation = self.allocate_host_function_with_length(window, name, 0).map_err(Fault::new)?;
                let prototype = self.new_object(None)?;
                let constructor = Value::Function(allocation.id);
                let properties = self.function_properties[allocation.id];
                if name == CONTROLLER {
                    if let Some(Function::Host { constructible, .. }) = self.functions.get_mut(allocation.id) {
                        *constructible = true;
                    }
                    self.define_readonly_node_getter(prototype, window, "signal", SIGNAL)?;
                    self.define_canvas_operation(prototype, window, "abort", ABORT, 0)?;
                } else {
                    let parent = self.get_property(target, "constructor")?;
                    self.objects[properties].prototype = Ordinary::from_value(&parent);
                    self.objects[prototype].prototype = Some(Ordinary::Object(target));
                    self.define_readonly_node_getter(prototype, window, "aborted", ABORTED)?;
                    self.define_readonly_node_getter(prototype, window, "reason", REASON)?;
                    self.define_canvas_operation(prototype, window, "throwIfAborted", THROW, 0)?;
                    self.define_node_attribute(prototype, window, "onabort", ON_ABORT, Some(SET_ON_ABORT))?;
                    let method = self.allocate_host_function_with_length(window, STATIC_ABORT, 0).map_err(Fault::new)?;
                    self.store_property(self.function_properties[method.id], "name".into(), Value::String("abort".into()), Some((false, false, true)))?;
                    self.store_property(properties, "abort".into(), Value::Function(method.id), Some((true, true, true)))?;
                }
                self.store_property(properties, "prototype".into(), Value::Object(prototype), Some((false, false, false)))?;
                self.store_property(prototype, "constructor".into(), constructor.clone(), Some((false, true, true)))?;
                self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(), Value::String(name.into()), Some((false, false, true)))?;
                self.native_constructor_prototypes.insert((window, name), prototype);
                self.put_binding(self.global_environment, name.into(), Some(constructor), super::Declaration::Var, false)?;
                self.global_attributes.insert(name.into(), (false, true, true));
            }
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub(crate) fn define_media_query_interface(&mut self, window: u32) -> Result<(), String> {
        use super::{MEDIA_QUERY_LIST, MEDIA_QUERY_MATCHES, MEDIA_QUERY_MEDIA};
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let allocation = self.allocate_host_function_with_length(window, MEDIA_QUERY_LIST, 0).map_err(Fault::new)?;
            let prototype = self.new_object(None)?;
            let constructor = Value::Function(allocation.id);
            let properties = self.function_properties[allocation.id];
            if let Some(Function::Host { constructible, illegal_interface_constructor, .. }) = self.functions.get_mut(allocation.id) {
                *constructible = true;
                *illegal_interface_constructor = true;
            }
            let parent = self.native_constructor_prototypes[&(window, "EventTarget")];
            let parent_constructor = self.get_property(parent, "constructor")?;
            self.objects[properties].prototype = Ordinary::from_value(&parent_constructor);
            self.objects[prototype].prototype = Some(Ordinary::Object(parent));
            self.define_readonly_node_getter(prototype, window, "matches", MEDIA_QUERY_MATCHES)?;
            self.define_readonly_node_getter(prototype, window, "media", MEDIA_QUERY_MEDIA)?;
            self.store_property(properties, "prototype".into(), Value::Object(prototype), Some((false, false, false)))?;
            self.store_property(prototype, "constructor".into(), constructor.clone(), Some((false, true, true)))?;
            self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(), Value::String(MEDIA_QUERY_LIST.into()), Some((false, false, true)))?;
            self.native_constructor_prototypes.insert((window, MEDIA_QUERY_LIST), prototype);
            self.put_binding(self.global_environment, MEDIA_QUERY_LIST.into(), Some(constructor), super::Declaration::Var, false)?;
            self.global_attributes.insert(MEDIA_QUERY_LIST.into(), (false, true, true));
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub(crate) fn define_message_channel_interfaces(&mut self, window: u32) -> Result<(), String> {
        use super::message_channel_interface::*;
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            for name in [CHANNEL, PORT, EVENT] {
                let allocation = self.allocate_host_function_with_length(window, name, usize::from(name == EVENT)).map_err(Fault::new)?;
                let prototype = self.new_object(None)?;
                let constructor = Value::Function(allocation.id);
                let properties = self.function_properties[allocation.id];
                if let Some(Function::Host { constructible, illegal_interface_constructor, .. }) = self.functions.get_mut(allocation.id) {
                    *constructible = true;
                    *illegal_interface_constructor = name != CHANNEL;
                }
                if name == CHANNEL {
                    self.define_readonly_node_getter(prototype, window, "port1", PORT1)?;
                    self.define_readonly_node_getter(prototype, window, "port2", PORT2)?;
                } else {
                    let parent_name = if name == PORT { "EventTarget" } else { "Event" };
                    let parent = self.native_constructor_prototypes[&(window, parent_name)];
                    let parent_constructor = self.get_property(parent, "constructor")?;
                    self.objects[properties].prototype = Ordinary::from_value(&parent_constructor);
                    self.objects[prototype].prototype = Some(Ordinary::Object(parent));
                    if name == PORT {
                        self.define_canvas_operation(prototype, window, "postMessage", POST, 1)?;
                        self.define_canvas_operation(prototype, window, "start", START, 0)?;
                        self.define_canvas_operation(prototype, window, "close", CLOSE, 0)?;
                        self.define_node_attribute(prototype, window, "onmessage", MESSAGE, Some(SET_MESSAGE))?;
                    } else {
                        for (property, operation) in [("data", DATA), ("origin", ORIGIN), ("source", SOURCE), ("lastEventId", LAST_ID)] {
                            self.define_readonly_node_getter(prototype, window, property, operation)?;
                        }
                    }
                }
                self.store_property(properties, "prototype".into(), Value::Object(prototype), Some((false, false, false)))?;
                self.store_property(prototype, "constructor".into(), constructor.clone(), Some((false, true, true)))?;
                self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(), Value::String(name.into()), Some((false, false, true)))?;
                self.native_constructor_prototypes.insert((window, name), prototype);
                self.put_binding(self.global_environment, name.into(), Some(constructor), super::Declaration::Var, false)?;
                self.global_attributes.insert(name.into(), (false, true, true));
            }
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub(super) fn native_instance_prototype(&mut self,handle:u32,host:&mut dyn Host)->Result<Ordinary,Fault> {
        self.validate_native_access(handle,host)?;
        if !host.exposed_native_prototype(handle) {return Err(Fault::new("Native host prototype inspection is not implemented for this receiver."));}
        self.native_instance_visible_anchor(handle,host)
    }

    pub(super) fn native_instance_visible_anchor(&mut self, handle: u32, host: &mut dyn Host) -> Result<Ordinary, Fault> {
        let native = self.native_instance_anchor(handle, host)?;
        if host.mutable_native_wrapper_prototype(handle) {
            self.ensure_native_expando(handle, host, false)?;
            if let Some(&backing) = self.host_objects.get(&handle) {
                return self.ordinary_prototype(Ordinary::Object(backing))?
                    .ok_or_else(|| Fault::new("Native prototype backing is unavailable."));
            }
        }
        Ok(native)
    }

    pub(super) fn set_native_wrapper_prototype(
        &mut self, handle: u32, prototype: Option<Ordinary>, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.validate_native_access(handle, host)?;
        if !host.mutable_native_wrapper_prototype(handle) {
            return Err(Fault::new("Native host object prototypes cannot be changed."));
        }
        let native = self.native_instance_anchor(handle, host)?;
        let mut current = prototype;
        while let Some(parent) = current {
            self.step()?;
            if parent == native {
                // The VM's object owns author properties and the script-visible
                // chain. The host keeps its immutable native brand and owner.
                if prototype == Some(native) && !self.host_objects.contains_key(&handle) {
                    return Ok(());
                }
                self.ensure_native_expando(handle, host, true)?;
                let backing = self.host_objects[&handle];
                return self.set_prototype(Ordinary::Object(backing), prototype);
            }
            if self.objects[self.ordinary_id(parent)?].proxy.is_some() {
                break;
            }
            current = self.ordinary_prototype(parent)?;
        }
        Err(Fault::exception("TypeError", "A native wrapper prototype must retain its real interface in the ordinary chain."))
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
    fn define_attribute_node_interface(&mut self,window:u32,node:usize)->Result<(),Fault>{
        let Value::Function(parent)=self.objects[node].properties["constructor"].value.as_value()?.clone()else{return Err(Fault::new("Invalid Attr parent constructor."));};
        let method=self.allocate_host_function_with_length(window,"Attr",0).map_err(Fault::new)?;if let Some(Function::Host{constructible,illegal_interface_constructor,..})=self.functions.get_mut(method.id){*constructible=true;*illegal_interface_constructor=true;}
        let prototype=self.new_object(None)?;self.objects[prototype].prototype=Some(Ordinary::Object(node));self.objects[self.function_properties[method.id]].prototype=Some(Ordinary::Function(parent));let constructor=Value::Function(method.id);
        self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;self.store_property(self.function_properties[method.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("Attr".into()),Some((false,false,true)))?;
        for(key,getter)in[("namespaceURI","\0Attr.get.namespaceURI"),("prefix","\0Attr.get.prefix"),("localName","\0Attr.get.localName"),("name","\0Attr.get.name"),("value","\0Attr.get.value"),("ownerElement","\0Attr.get.ownerElement"),("specified","\0Attr.get.specified")]{self.define_node_attribute(prototype,window,key,getter,if key=="value"{Some("\0Attr.set.value")}else{None})?;}
        self.put_binding(self.global_environment,"Attr".into(),Some(constructor),super::Declaration::Var,false)?;self.global_attributes.insert("Attr".into(),(false,true,true));self.native_constructor_prototypes.insert((window,"Attr"),prototype);Ok(())
    }
    pub fn define_node_interfaces(&mut self,window:u32,constants:&[(&str,u16)],image:Option<(u32,&'static str)>)->Result<(),String> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            let mut parent=*self.native_constructor_prototypes.get(&(window,"EventTarget")).ok_or_else(||Fault::new("EventTarget must precede Node interfaces."))?;
            for name in ["Node","Element","HTMLElement"] {
                let parent_constructor=self.objects[parent].properties.get("constructor").ok_or_else(||Fault::new("Missing native parent constructor."))?.value.as_value()?.clone();
                let Value::Function(parent_function)=parent_constructor else {return Err(Fault::new("Invalid native parent constructor."));};
                let allocation=self.allocate_host_function_with_length(window,name,0).map_err(Fault::new)?;
                if let Some(Function::Host {constructible,illegal_interface_constructor,..})=self.functions.get_mut(allocation.id) {
                    *constructible=true;
                    *illegal_interface_constructor=true;
                }
                let prototype=self.new_object(None)?;
                self.objects[prototype].prototype=Some(Ordinary::Object(parent));
                self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_function));
                let constructor=Value::Function(allocation.id);
                self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
                self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String(name.into()),Some((false,false,true)))?;
                if name=="Node" {
                    let readers = [("getRootNode", "\0Node.getRootNode", 0),
                        ("contains", "\0Node.contains", 1), ("normalize","\0Node.normalize",0), ("cloneNode", "\0Node.cloneNode", 0),("isDefaultNamespace","\0Node.isDefaultNamespace",1),("lookupNamespaceURI","\0Node.lookupNamespaceURI",1),("lookupPrefix","\0Node.lookupPrefix",1),("isSameNode","\0Node.isSameNode",1),("isEqualNode","\0Node.isEqualNode",1),("compareDocumentPosition","\0Node.compareDocumentPosition",1)];
                    for (name, operation, length) in readers.into_iter().chain(
                        NODE_MUTATION_METHODS.map(|(name, length)| (name, name, length)),
                    ) {
                        let method = self.allocate_host_function_with_length(window, operation, length).map_err(Fault::new)?;
                        if let Some(Function::Host { receiver_required, .. }) = self.functions.get_mut(method.id) {
                            *receiver_required = true;
                        }
                        self.store_property(self.function_properties[method.id], "name".into(), Value::String(name.into()), Some((false,false,true)))?;
                        self.store_property(prototype, name.into(), Value::Function(method.id), Some((true,true,true)))?;
                    }
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
                    for (property, operation) in NODE_TRAVERSAL_GETTERS {
                        self.define_readonly_node_getter(prototype, window, property, operation)?;
                    }
                    self.define_node_attribute(prototype, window, "textContent",
                        NODE_TEXT_CONTENT_GET, Some(NODE_TEXT_CONTENT_SET))?;
                }
                if name=="Element" {
                    for(key,operation,length)in[("getElementsByTagName",super::ELEMENT_GET_ELEMENTS_TAG,1),("getElementsByClassName",super::ELEMENT_GET_ELEMENTS_CLASS,1),("getAttributeNode","getAttributeNode",1),("getAttributeNodeNS","getAttributeNodeNS",2),("setAttributeNode","setAttributeNode",1),("setAttributeNodeNS","setAttributeNodeNS",1),("removeAttributeNode","removeAttributeNode",1)]{
                        let method=self.allocate_host_function_with_length(window,operation,length).map_err(Fault::new)?;
                        if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(method.id){*receiver_required=true;}
                        self.store_property(self.function_properties[method.id],"name".into(),Value::String(key.into()),Some((false,false,true)))?;
                        self.store_property(prototype,key.into(),Value::Function(method.id),Some((true,true,true)))?;
                    }
                    for &(key,_,get,set) in aria_reflection_interface::ATTRIBUTES {self.define_node_attribute(prototype,window,key,get,Some(set))?;}
                    for (property,operation) in PARENT_NODE_GETTERS {
                        self.define_readonly_node_getter(prototype,window,property,operation)?;
                    }
                }
                if name=="Element" {
                    self.define_node_attribute(prototype, window, "innerHTML",
                        ELEMENT_INNER_HTML_GET, Some(ELEMENT_INNER_HTML_SET))?;
                    for (key, getter, setter) in [
                        ("scrollLeft", "\0Element.get.scrollLeft", "\0Element.set.scrollLeft"),
                        ("scrollTop", "\0Element.get.scrollTop", "\0Element.set.scrollTop"),
                    ] {
                        self.define_node_attribute(prototype, window, key, getter, Some(setter))?;
                    }
                    self.define_element_sibling_getters(prototype,window,"Element")?;
                    self.define_readonly_node_getter(prototype, window, "assignedSlot", "\0Element.get.assignedSlot")?;
                    for (key, operation) in [
                        ("querySelector", "\0Element.querySelector"),
                        ("querySelectorAll", "\0Element.querySelectorAll"),
                        ("matches", "\0Element.matches"),
                        ("closest", "\0Element.closest"),
                        ("getAttributeNS", "getAttributeNS"),
                        ("getAttributeNodeNS", "getAttributeNodeNS"),
                        ("setAttributeNS", "setAttributeNS"),
                        ("removeAttributeNS", "removeAttributeNS"),
                        ("getAttribute", "getAttribute"),
                        ("setAttribute", "setAttribute"),
                        ("getAttributeNode", "getAttributeNode"),
                        ("hasAttribute", "hasAttribute"),
                        ("removeAttribute", "removeAttribute"),
                        ("getAttributeNames", "\0Element.getAttributeNames"),
                    ] {
                        let method=self.allocate_host_function_with_length(window,operation,if key=="getAttributeNames"{0}else if key=="setAttributeNS"{3}else if matches!(key,"setAttribute"|"getAttributeNS"|"getAttributeNodeNS"|"removeAttributeNS"){2}else{1}).map_err(Fault::new)?;
                        if let Some(Function::Host {receiver_required,..})=self.functions.get_mut(method.id) {*receiver_required=true;}
                        self.store_property(self.function_properties[method.id],"name".into(),Value::String(key.into()),Some((false,false,true)))?;
                        self.store_property(prototype,key.into(),Value::Function(method.id),Some((true,true,true)))?;
                    }
                }
                if name == "HTMLElement" {
                    for &(key,get,set) in html_reflection_interface::ATTRIBUTES{self.define_node_attribute(prototype,window,key,get,Some(set))?;}
                    self.define_node_attribute(prototype, window, "contentEditable",
                        "\0HTMLElement.get.contentEditable", Some("\0HTMLElement.set.contentEditable"))?;
                    self.define_node_attribute(prototype, window, "style",
                        ELEMENT_STYLE_GET, Some(ELEMENT_STYLE_SET))?;
                    for (key, operation) in [
                        ("offsetParent", "\0HTMLElement.get.offsetParent"),
                        ("offsetLeft", "\0HTMLElement.get.offsetLeft"),
                        ("offsetTop", "\0HTMLElement.get.offsetTop"),
                    ] {
                        self.define_readonly_node_getter(prototype, window, key, operation)?;
                    }
                }
                self.put_binding(self.global_environment,name.into(),Some(constructor),super::Declaration::Var,false)?;
                self.global_attributes.insert(name.into(),(false,true,true));
                self.native_constructor_prototypes.insert((window,name),prototype);
                parent=prototype;
            }
            let element=self.native_constructor_prototypes[&(window,"Element")];
            self.define_svg_element_interface(window, element)?;
            self.define_concrete_html_node_interfaces(window, parent, image)?;
            self.define_canvas_2d_interface(window)?;
            // Text and Comment inherit CharacterData. Only Text includes Slottable.
            let node=self.native_constructor_prototypes[&(window,"Node")];
            self.define_attribute_node_interface(window,node)?;
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
            let substring=self.allocate_host_function_with_length(window,"\0CharacterData.substringData",2).map_err(Fault::new)?;
            if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(substring.id){*receiver_required=true;}
            self.store_property(self.function_properties[substring.id],"name".into(),Value::String("substringData".into()),Some((false,false,true)))?;
            self.store_property(character,"substringData".into(),Value::Function(substring.id),Some((true,true,true)))?;
            for (key,operation,length) in super::character_data_interface::METHODS{let method=self.allocate_host_function_with_length(window,operation,length).map_err(Fault::new)?;if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(method.id){*receiver_required=true;}self.store_property(self.function_properties[method.id],"name".into(),Value::String(key.into()),Some((false,false,true)))?;self.store_property(character,key.into(),Value::Function(method.id),Some((true,true,true)))?;}

            self.put_binding(self.global_environment,"CharacterData".into(),Some(constructor),super::Declaration::Var,false)?;
            self.global_attributes.insert("CharacterData".into(),(false,true,true));
            self.native_constructor_prototypes.insert((window,"CharacterData"),character);
            self.define_character_data_interface(window, character, "Text")?;
            self.define_character_data_interface(window, character, "Comment")?;
            self.define_character_data_interface(window, character, "ProcessingInstruction")?;
            let text = self.native_constructor_prototypes[&(window, "Text")];
            self.define_readonly_node_getter(text,window,"wholeText","\0Text.get.wholeText")?;
            let split=self.allocate_host_function_with_length(window,"\0Text.splitText",1).map_err(Fault::new)?;if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(split.id){*receiver_required=true;}self.store_property(self.function_properties[split.id],"name".into(),Value::String("splitText".into()),Some((false,false,true)))?;self.store_property(text,"splitText".into(),Value::Function(split.id),Some((true,true,true)))?;
            self.define_character_data_interface(window, text, "CDATASection")?;
            for name in ["DocumentFragment","Document"] {
                let allocation=self.allocate_host_function_with_length(window,name,0).map_err(Fault::new)?;
                if let Some(Function::Host {constructible,..})=self.functions.get_mut(allocation.id) {*constructible=true;}
                let prototype=self.new_object(None)?;
                self.objects[prototype].prototype=Some(Ordinary::Object(node));
                self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(node_constructor));
                let constructor=Value::Function(allocation.id);
                self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
                self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
                self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String(name.into()),Some((false,false,true)))?;
                for (property,operation) in PARENT_NODE_GETTERS {
                    self.define_readonly_node_getter(prototype,window,property,operation)?;
                }
                let queries=if name=="Document" {[("querySelector","\0Document.querySelector"),("querySelectorAll","\0Document.querySelectorAll"),("getElementById","\0Document.getElementById")]}else{[("querySelector","\0DocumentFragment.querySelector"),("querySelectorAll","\0DocumentFragment.querySelectorAll"),("getElementById","\0DocumentFragment.getElementById")]};
                for (key,operation) in queries {
                    let method=self.allocate_host_function_with_length(window,operation,1).map_err(Fault::new)?;
                    if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(method.id){*receiver_required=true;}
                    self.store_property(self.function_properties[method.id],"name".into(),Value::String(key.into()),Some((false,false,true)))?;
                    self.store_property(prototype,key.into(),Value::Function(method.id),Some((true,true,true)))?;
                }
                if name=="Document" {
                    self.define_node_attribute(prototype,window,"documentElement","\0Document.get.documentElement",None)?;
                    for (property, getter) in super::DOCUMENT_ENCODING_GETTERS {
                        self.define_readonly_node_getter(prototype, window, property, getter)?;
                    }
                    self.define_readonly_node_getter(prototype, window, "currentScript", DOCUMENT_CURRENT_SCRIPT_GET)?;
                    self.define_readonly_node_getter(prototype, window, "activeElement", DOCUMENT_ACTIVE_ELEMENT_GET)?;
                    for (name, operation, length) in [
                        ("createCDATASection", CREATE_CDATA_SECTION, 1),
                        ("createProcessingInstruction", CREATE_PROCESSING_INSTRUCTION, 2),
                        ("importNode", DOCUMENT_IMPORT_NODE, 1),
                        ("adoptNode",DOCUMENT_ADOPT_NODE,1),
                        ("createAttribute",super::DOCUMENT_CREATE_ATTRIBUTE,1),
                        ("createAttributeNS",super::DOCUMENT_CREATE_ATTRIBUTE_NS,2),
                        ("getElementsByTagName",super::DOCUMENT_GET_ELEMENTS_TAG,1),
                        ("getElementsByClassName",super::DOCUMENT_GET_ELEMENTS_CLASS,1),
                    ] {
                        let method = self.allocate_host_function_with_length(window, operation, length)
                            .map_err(Fault::new)?;
                        if let Some(Function::Host { receiver_required, .. }) = self.functions.get_mut(method.id) {
                            *receiver_required = true;
                        }
                        self.store_property(self.function_properties[method.id], "name".into(),
                            Value::String(name.into()), Some((false, false, true)))?;
                        self.store_property(prototype, name.into(),
                            Value::Function(method.id), Some((true, true, true)))?;
                    }
                    // HTML defines this Window property as an alias of the very same interface object.
                    self.put_binding(self.global_environment,"HTMLDocument".into(),Some(constructor.clone()),super::Declaration::Var,false)?;
                    self.global_attributes.insert("HTMLDocument".into(),(false,true,true));
                }
                self.put_binding(self.global_environment,name.into(),Some(constructor),super::Declaration::Var,false)?;
                self.global_attributes.insert(name.into(),(false,true,true));
                self.native_constructor_prototypes.insert((window,name),prototype);
            }
            if let Some(key)=image {
                let image=*self.native_constructor_prototypes.get(&key).ok_or_else(||Fault::new("Missing canonical Image prototype."))?;
                self.objects[image].prototype=Some(Ordinary::Object(parent));
                self.store_symbol_property(image,self.symbol_to_string_tag.unwrap(),Value::String("HTMLImageElement".into()),Some((false,false,true)))?;
            }
            self.define_node_list_iteration_interface(window)?;
            self.define_html_collection_interface(window)?;
            self.define_named_node_map_interface(window)?;
            self.define_window_interface(window)?;
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    /// SVG nodes expose only their proven Element/SVGElement ancestry suffix.
    /// Their more-specific native prototypes and SVG mixins remain opaque.
    fn define_svg_element_interface(&mut self, window:u32, element:usize)->Result<(),Fault> {
        let parent_constructor=self.objects[element].properties["constructor"].value.as_value()?.clone();
        let Value::Function(parent_function)=parent_constructor else {
            return Err(Fault::new("Invalid SVGElement parent constructor."));
        };
        let allocation=self.allocate_host_function_with_length(window,"SVGElement",0)
            .map_err(Fault::new)?;
        if let Some(Function::Host {constructible,illegal_interface_constructor,..})=self.functions.get_mut(allocation.id) {
            *constructible=true;
            *illegal_interface_constructor=true;
        }
        let prototype=self.new_object(None)?;
        self.objects[prototype].prototype=Some(Ordinary::Object(element));
        self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(parent_function));
        let constructor=Value::Function(allocation.id);
        self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
        self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
        let tag=self.symbol_to_string_tag.ok_or_else(||Fault::new("Missing SVGElement class-tag symbol."))?;
        self.store_symbol_property(prototype,tag,Value::String("SVGElement".into()),Some((false,false,true)))?;
        self.define_node_attribute(prototype,window,"style",SVG_STYLE_GET,Some(SVG_STYLE_SET))?;
        self.native_constructor_prototypes.insert((window,"SVGElement"),prototype);
        self.put_binding(self.global_environment,"SVGElement".into(),Some(constructor),super::Declaration::Var,false)?;
        self.global_attributes.insert("SVGElement".into(),(false,true,true));
        Ok(())
    }

    /// The measured concrete HTML interfaces share the real HTMLElement
    /// suffix, while keeping their own canonical constructor and class tag.
    /// Registration runs in each owner's realm, including blank child realms.
    fn define_concrete_html_node_interfaces(
        &mut self,
        window: u32,
        html_element: usize,
        image: Option<(u32, &'static str)>,
    ) -> Result<(), Fault> {
        let parent_constructor = self.objects[html_element].properties
            .get("constructor")
            .ok_or_else(|| Fault::new("Missing HTMLElement constructor."))?
            .value.as_value()?.clone();
        let Value::Function(parent_function) = parent_constructor else {
            return Err(Fault::new("Invalid HTMLElement constructor."));
        };
        let tag_key = self.symbol_to_string_tag
            .ok_or_else(|| Fault::new("Missing interface class-tag symbol."))?;

        for &(_, name) in super::ORDINARY_HTML_SUBCLASSES {
            let allocation = self.allocate_host_function_with_length(window, name, 0)
                .map_err(Fault::new)?;
            if let Some(Function::Host { constructible, illegal_interface_constructor, .. }) = self.functions.get_mut(allocation.id) {
                *constructible = true;
                *illegal_interface_constructor = true;
            }
            // The legacy Image factory and HTMLImageElement have distinct
            // constructors but share the actual image-element prototype.
            let prototype = if name == "HTMLImageElement" {
                if let Some(key) = image {
                    *self.native_constructor_prototypes.get(&key)
                        .ok_or_else(|| Fault::new("Missing canonical Image prototype."))?
                } else {
                    self.new_object(None)?
                }
            } else {
                self.new_object(None)?
            };
            self.objects[prototype].prototype = Some(Ordinary::Object(html_element));
            self.objects[self.function_properties[allocation.id]].prototype =
                Some(Ordinary::Function(parent_function));
            let constructor = Value::Function(allocation.id);
            self.store_property(
                prototype, "constructor".into(), constructor.clone(),
                Some((false, true, true)),
            )?;
            self.store_property(
                self.function_properties[allocation.id], "prototype".into(),
                Value::Object(prototype), Some((false, false, false)),
            )?;
            self.store_symbol_property(
                prototype, tag_key, Value::String(name.into()),
                Some((false, false, true)),
            )?;
            if name == "HTMLFormElement" {
                self.define_canvas_operation(prototype,window,"submit",super::HTML_FORM_SUBMIT,0)?;
            }
            if name == "HTMLTemplateElement" {
                self.define_readonly_node_getter(prototype, window, "content", HTML_TEMPLATE_CONTENT_GET)?;
            }
            if name == "HTMLSlotElement" {
                self.define_node_attribute(prototype, window, "name", HTML_SLOT_NAME_GET, Some(HTML_SLOT_NAME_SET))?;
            }
            if name == "HTMLIFrameElement" {
                self.define_node_attribute(prototype,window,"sandbox",super::IFRAME_SANDBOX_GET,Some(super::IFRAME_SANDBOX_SET))?;
            }
            if matches!(name,"HTMLTextAreaElement"|"HTMLInputElement"){let (get,set)=if name=="HTMLTextAreaElement"{("\0HTMLTextAreaElement.get.placeholder","\0HTMLTextAreaElement.set.placeholder")}else{("\0HTMLInputElement.get.placeholder","\0HTMLInputElement.set.placeholder")};self.define_node_attribute(prototype,window,"placeholder",get,Some(set))?;}
            if name == "HTMLTextAreaElement" {
                self.define_node_attribute(prototype,window,"value",super::TEXTAREA_VALUE_GET,Some(super::TEXTAREA_VALUE_SET))?;
            }
            if name == "HTMLInputElement" {
                for (key, getter, setter) in [
                    ("value", "\0HTMLInputElement.get.value", "\0HTMLInputElement.set.value"),
                    ("checked", "\0HTMLInputElement.get.checked", "\0HTMLInputElement.set.checked"),
                    ("defaultChecked", "\0HTMLInputElement.get.defaultChecked", "\0HTMLInputElement.set.defaultChecked"),
                ] {
                    self.define_node_attribute(prototype, window, key, getter, Some(setter))?;
                }
            }
            if name == "HTMLCanvasElement" {
                for (key, getter, setter) in [
                    ("width", "\0HTMLCanvasElement.get.width", "\0HTMLCanvasElement.set.width"),
                    ("height", "\0HTMLCanvasElement.get.height", "\0HTMLCanvasElement.set.height"),
                ] {
                    self.define_node_attribute(prototype, window, key, getter, Some(setter))?;
                }
                self.define_canvas_operation(
                    prototype,
                    window,
                    "getContext",
                    "\0HTMLCanvasElement.getContext",
                    1,
                )?;
            }

            // Existing native constructor roots keep both prototype chains
            // alive across realm parking, deferred calls and collection.
            self.native_constructor_prototypes.insert((window, name), prototype);
            self.put_binding(
                self.global_environment, name.into(), Some(constructor),
                super::Declaration::Var, false,
            )?;
            self.global_attributes.insert(name.into(), (false, true, true));
        }
        Ok(())
    }

    /// A rendering context is an independent Web IDL object, not a Node or
    /// EventTarget. Each owner realm supplies its actual ordinary prototype;
    /// only a genuine host context can satisfy the methods' receiver checks.
    fn define_canvas_2d_interface(&mut self, window: u32) -> Result<(), Fault> {
        let allocation = self
            .allocate_host_function_with_length(window, "CanvasRenderingContext2D", 0)
            .map_err(Fault::new)?;
        if let Some(Function::Host { constructible, .. }) = self.functions.get_mut(allocation.id) {
            // The bridge rejects construction as an illegal constructor. This
            // does not opt another native family into class-base support.
            *constructible = true;
        }
        let prototype = self.new_object(None)?;
        let constructor = Value::Function(allocation.id);
        self.store_property(
            prototype,
            "constructor".into(),
            constructor.clone(),
            Some((false, true, true)),
        )?;
        self.store_property(
            self.function_properties[allocation.id],
            "prototype".into(),
            Value::Object(prototype),
            Some((false, false, false)),
        )?;
        let tag = self
            .symbol_to_string_tag
            .ok_or_else(|| Fault::new("Missing canvas context class-tag symbol."))?;
        self.store_symbol_property(
            prototype,
            tag,
            Value::String("CanvasRenderingContext2D".into()),
            Some((false, false, true)),
        )?;
        self.define_readonly_node_getter(
            prototype,
            window,
            "canvas",
            "\0CanvasRenderingContext2D.get.canvas",
        )?;
        self.define_node_attribute(
            prototype,
            window,
            "fillStyle",
            "\0CanvasRenderingContext2D.get.fillStyle",
            Some("\0CanvasRenderingContext2D.set.fillStyle"),
        )?;
        for (name, operation) in [
            ("clearRect", "\0CanvasRenderingContext2D.clearRect"),
            ("fillRect", "\0CanvasRenderingContext2D.fillRect"),
            ("getImageData", super::canvas_readback::METHOD),
        ] {
            self.define_canvas_operation(prototype, window, name, operation, 4)?;
        }
        self.native_constructor_prototypes
            .insert((window, "CanvasRenderingContext2D"), prototype);
        self.put_binding(
            self.global_environment,
            "CanvasRenderingContext2D".into(),
            Some(constructor),
            super::Declaration::Var,
            false,
        )?;
        self.global_attributes
            .insert("CanvasRenderingContext2D".into(), (false, true, true));
        Ok(())
    }

    pub(super) fn define_canvas_operation(
        &mut self,
        prototype: usize,
        window: u32,
        name: &'static str,
        operation: &'static str,
        length: usize,
    ) -> Result<(), Fault> {
        let method = self
            .allocate_host_function_with_length(window, operation, length)
            .map_err(Fault::new)?;
        if let Some(Function::Host { receiver_required, .. }) = self.functions.get_mut(method.id) {
            *receiver_required = true;
        }
        self.store_property(
            self.function_properties[method.id],
            "name".into(),
            Value::String(name.into()),
            Some((false, false, true)),
        )?;
        self.store_property(
            prototype,
            name.into(),
            Value::Function(method.id),
            Some((true, true, true)),
        )
    }

    /// Supported class bases have constructor-specific newTarget handling.
    /// Nonallocating interfaces inherit normally but cannot create unregistered
    /// instances. Admission uses immutable function metadata.
    /// Keep native qualification off ordinary recursive Script call frames.
    #[inline(never)]
    pub(super) fn supported_class_base(&self, function:usize, host:&dyn Host)->Result<bool,Fault> {
        match self.functions.get(function) {
            Some(Function::Script { .. }
                | Function::Native(Builtin::Collection(collections::Op::Constructor(_)) | Builtin::Error(_))) => Ok(true),
            Some(Function::Host {
                object, constructible:true, illegal_interface_constructor:true, ..
            }) => {
                // Saved same-origin constructors do not confer permanent
                // access after the actual document/owner boundary changes.
                self.validate_native_access(*object,host)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn define_window_interface(&mut self, window: u32) -> Result<(), Fault> {
        let Some((handle, global_properties)) = self.browser_global else { return Ok(()); };
        if handle != window || self.native_constructor_prototypes.contains_key(&(window, "Window")) {
            return Ok(());
        }
        let event_target = self.native_constructor_prototypes[&(window, "EventTarget")];
        let Value::Function(parent) = self.objects[event_target].properties["constructor"].value.as_value()?.clone() else {
            return Err(Fault::new("Invalid Window parent interface."));
        };
        let allocation = self.allocate_host_function_with_length(window, "Window", 0).map_err(Fault::new)?;
        if let Some(Function::Host { constructible, illegal_interface_constructor, .. }) =
            self.functions.get_mut(allocation.id)
        {
            *constructible = true;
            *illegal_interface_constructor = true;
        }
        let constructor = Value::Function(allocation.id);
        let prototype = self.new_object(None)?;
        let named_properties = self.new_object(None)?;
        self.objects[prototype].prototype = Some(Ordinary::Object(named_properties));
        self.objects[named_properties].prototype = Some(Ordinary::Object(event_target));
        self.objects[self.function_properties[allocation.id]].prototype = Some(Ordinary::Function(parent));
        self.store_property(prototype, "constructor".into(), constructor.clone(), Some((false, true, true)))?;
        self.store_property(self.function_properties[allocation.id], "prototype".into(),
            Value::Object(prototype), Some((false, false, false)))?;
        let tag = self.symbol_to_string_tag.ok_or_else(|| Fault::new("Missing Window class-tag symbol."))?;
        for (object, value) in [(prototype, "Window"), (named_properties, "WindowProperties")] {
            self.store_symbol_property(object, tag, Value::String(value.into()), Some((false, false, true)))?;
            self.objects[object].immutable_prototype = true;
        }
        self.objects[named_properties].window_named_properties = true;
        self.objects[event_target].immutable_prototype = true;
        self.put_binding(self.global_environment, "Window".into(), Some(constructor),
            super::Declaration::Var, false)?;
        self.global_attributes.insert("Window".into(), (false, true, true));
        self.native_constructor_prototypes.insert((window, "Window"), prototype);
        self.objects[global_properties].prototype = Some(Ordinary::Object(prototype));
        Ok(())
    }

    /// Tag lookup keeps the native caller check on the already bound Window.
    pub(super) fn bound_window_tag_backing(
        &mut self,
        window: u32,
        host: &dyn Host,
    ) -> Result<Option<Ordinary>, Fault> {
        let Some(&realm) = self.window_realms.get(&window) else { return Ok(None); };
        self.validate_native_access(window, host)?;
        let caller = self.native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(handle, _)| handle));
        if caller != Some(window) {
            host.validate_window_access(caller, window).map_err(Fault::new)?;
        }
        self.with_intrinsic_realm(realm, |runtime| {
            if !runtime.is_browser_global(window) { return Ok(None); }
            Ok(runtime.native_constructor_prototypes.get(&(window, "Window"))
                .copied().map(Ordinary::Object))
        })
    }

    fn define_named_node_map_interface(&mut self,window:u32)->Result<(),Fault>{
        let allocation=self.allocate_host_function_with_length(window,"NamedNodeMap",0).map_err(Fault::new)?;
        if let Some(Function::Host{constructible,..})=self.functions.get_mut(allocation.id){*constructible=true;}
        let constructor=Value::Function(allocation.id);let prototype=self.new_object(None)?;
        self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
        self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
        self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("NamedNodeMap".into()),Some((false,false,true)))?;
        let values=self.array_iterator_intrinsic(super::collections::Mode::Values);self.store_symbol_property(prototype,self.symbol_iterator.unwrap(),values,Some((false,true,true)))?;
        for(name,operation,length)in[("length","\0NamedNodeMap.get.length",0),("item","\0NamedNodeMap.item",1),("getNamedItem","\0NamedNodeMap.getNamedItem",1),("getNamedItemNS","\0NamedNodeMap.getNamedItemNS",2)]{
            let method=self.allocate_host_function_with_length(window,operation,length).map_err(Fault::new)?;
            if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(method.id){*receiver_required=true;}
            self.store_property(self.function_properties[method.id],"name".into(),Value::String(if name=="length"{"get length"}else{name}.into()),Some((false,false,true)))?;
            if name=="length"{self.store_accessor_property(prototype,name.into(),Accessors{get:Some(method.id),set:None},(true,true))?;}
            else{self.store_property(prototype,name.into(),Value::Function(method.id),Some((true,true,true)))?;}
        }
        self.put_binding(self.global_environment,"NamedNodeMap".into(),Some(constructor),super::Declaration::Var,false)?;self.global_attributes.insert("NamedNodeMap".into(),(false,true,true));
        self.native_constructor_prototypes.insert((window,"NamedNodeMap"),prototype);Ok(())
    }

    fn define_html_collection_interface(&mut self,window:u32)->Result<(),Fault> {
        let allocation=self.allocate_host_function_with_length(window,"HTMLCollection",0).map_err(Fault::new)?;
        if let Some(Function::Host{constructible,..})=self.functions.get_mut(allocation.id){*constructible=true;}
        let constructor=Value::Function(allocation.id);
        let prototype=self.new_object(None)?;
        self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
        self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
        self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("HTMLCollection".into()),Some((false,false,true)))?;
        let values=self.array_iterator_intrinsic(super::collections::Mode::Values);
        self.store_symbol_property(prototype,self.symbol_iterator.unwrap(),values,Some((false,true,true)))?;
        for (name,operation,length) in [("length","\0HTMLCollection.get.length",0),("item","\0HTMLCollection.item",1),("namedItem","\0HTMLCollection.namedItem",1)] {
            let method=self.allocate_host_function_with_length(window,operation,length).map_err(Fault::new)?;
            if let Some(Function::Host{receiver_required,..})=self.functions.get_mut(method.id){*receiver_required=true;}
            self.store_property(self.function_properties[method.id],"name".into(),Value::String(if name=="length"{"get length"}else{name}.into()),Some((false,false,true)))?;
            if name=="length"{self.store_accessor_property(prototype,name.into(),Accessors{get:Some(method.id),set:None},(true,true))?;}
            else {self.store_property(prototype,name.into(),Value::Function(method.id),Some((true,true,true)))?;}
        }
        self.put_binding(self.global_environment,"HTMLCollection".into(),Some(constructor),super::Declaration::Var,false)?;
        self.global_attributes.insert("HTMLCollection".into(),(false,true,true));
        self.native_constructor_prototypes.insert((window,"HTMLCollection"),prototype);
        Ok(())
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

    fn define_character_data_interface(
        &mut self, window: u32, parent: usize, name: &'static str,
    ) -> Result<(), Fault> {
        let parent_constructor = self.objects[parent].properties
            .get("constructor")
            .ok_or_else(|| Fault::new("Missing CharacterData constructor."))?
            .value.as_value()?.clone();
        let Value::Function(parent_function) = parent_constructor else {
            return Err(Fault::new("Invalid CharacterData constructor."));
        };
        let length = usize::from(name == "ProcessingInstruction");
        let allocation = self.allocate_host_function_with_length(window, name, length)
            .map_err(Fault::new)?;
        if let Some(Function::Host { constructible, illegal_interface_constructor, .. }) =
            self.functions.get_mut(allocation.id)
        {
            *constructible = true;
            *illegal_interface_constructor = name == "CDATASection";
        }
        let prototype = self.new_object(None)?;
        self.objects[prototype].prototype = Some(Ordinary::Object(parent));
        self.objects[self.function_properties[allocation.id]].prototype =
            Some(Ordinary::Function(parent_function));
        let constructor = Value::Function(allocation.id);
        self.store_property(
            prototype, "constructor".into(), constructor.clone(),
            Some((false, true, true)),
        )?;
        self.store_property(
            self.function_properties[allocation.id], "prototype".into(),
            Value::Object(prototype), Some((false, false, false)),
        )?;
        let tag = self.symbol_to_string_tag
            .ok_or_else(|| Fault::new("Missing CharacterData class-tag symbol."))?;
        self.store_symbol_property(
            prototype, tag, Value::String(name.into()),
            Some((false, false, true)),
        )?;
        if name == "Text" {
            self.define_readonly_node_getter(prototype, window, "assignedSlot", "\0Text.get.assignedSlot")?;
        }
        if name == "ProcessingInstruction" {
            self.define_readonly_node_getter(prototype, window, "target", PROCESSING_INSTRUCTION_TARGET_GET)?;
        }
        self.native_constructor_prototypes.insert((window, name), prototype);
        self.put_binding(
            self.global_environment, name.into(), Some(constructor),
            super::Declaration::Var, false,
        )?;
        self.global_attributes.insert(name.into(), (false, true, true));
        Ok(())
    }

    pub(super) fn define_readonly_node_getter(
        &mut self,
        prototype: usize,
        window: u32,
        name: &'static str,
        operation: &'static str,
    ) -> Result<(), Fault> {
        self.define_node_attribute(prototype, window, name, operation, None)
    }

    fn define_node_attribute(&mut self, prototype: usize, window: u32, name: &'static str,
        getter: &'static str, setter: Option<&'static str>) -> Result<(), Fault> {
        let mut accessor = |operation, prefix, length| -> Result<usize, Fault> {
            let f = self.allocate_host_function_with_length(window, operation, length).map_err(Fault::new)?;
            if let Some(Function::Host { receiver_required, .. }) = self.functions.get_mut(f.id) {
                *receiver_required = true;
            }
            self.store_property(self.function_properties[f.id], "name".into(),
                Value::String(format!("{prefix} {name}")), Some((false, false, true)))?;
            Ok(f.id)
        };
        let get = Some(accessor(getter, "get", 0)?);
        let set = setter.map(|operation| accessor(operation, "set", 1)).transpose()?;
        self.store_accessor_property(
            prototype, name.into(),
            Accessors { get, set }, (true, true),
        )
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
        let base_event=matches!(ancestry.constructor,Some((window,"Event"|"UIEvent"|"MouseEvent"|"WheelEvent"|"KeyboardEvent"|"FocusEvent"|"ProgressEvent"|"AnimationEvent"|"ErrorEvent"|"CustomEvent"|"MessageEvent")) if window==owner);
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
        // Ordinary inherited accessors can supply a setter even when this
        // native receiver cannot retain a new own data property.
        if host.inherited_assignment(handle, &name) {
            // A form's named getter can mask this key during reads. Web IDL
            // ignores named properties when looking up the assignment target.
            self.ensure_native_expando(handle, host, true)?;
            if let Some(&id) = self.host_objects.get(&handle) {
                return self.assign_ordinary_property_with_receiver(Ordinary::Object(id), name, value, Value::Host(handle), host);
            }
            let base = self.native_instance_anchor(handle, host)?;
            return self.assign_inherited_property(base, name, value, Value::Host(handle), host);
        }
        if host.readonly_property(handle, &name) {
            if let Some(base) = self.host_lookup_tail(handle, &name, host)? {
                return self.assign_inherited_property(base, name, value, Value::Host(handle), host);
            }
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
    pub(super) fn host_set_native(&mut self,handle:u32,name:&str,value:Value,host:&mut dyn Host)->Result<(),Fault>{self.host_set_native_edit(handle,name,value,None,host)}
    pub(super) fn host_set_character_data_splice(&mut self,handle:u32,edit:super::NativeCharacterDataEdit,host:&mut dyn Host)->Result<(),Fault>{let splice=Some((edit.offset,edit.removed,edit.inserted));self.host_set_native_edit(handle,"data",edit.value,splice,host)}
    fn host_set_native_edit(&mut self,handle:u32,name:&str,value:Value,splice:Option<(usize,usize,usize)>,host:&mut dyn Host)->Result<(),Fault>
    {
        self.validate_native_access(handle,host)?;
        self.reject_foreign_window(&Value::Host(handle),host)?;
        let value = if host.nullable_string_setter(handle,name) && matches!(value,Value::Null|Value::Undefined) {
            Value::Null
        } else if host.nullable_scalar_setter(handle, name) {
            match value {
                Value::Null | Value::Undefined => Value::Null,
                Value::Bool(_) | Value::Number(_) | Value::String(_) => self.checked_value(value)?,
                _ => {
                    let primitive = self.primitive_with_hint(value, true, host)?;
                    self.work(primitive.bytes())?;
                    self.checked_value(Value::String(primitive.script_text()?))?
                }
            }
        } else if host.string_setter(handle, name) {
            let value = if host.nullish_string_setter(handle, name) && matches!(value, Value::Null | Value::Undefined)
                || host.null_string_setter(handle, name) && matches!(value, Value::Null) {
                Value::String(String::new())
            } else { value };
            let primitive = self.primitive_with_hint(value, true, host)?;
            self.work(primitive.bytes())?;
            self.checked_value(if host.preserves_dom_string_setter(handle,name)&&primitive.is_string(){primitive}
                else{Value::String(primitive.script_text()?)})?
        } else if host.number_setter(handle, name) {
            let primitive = self.primitive_with_hint(value, false, host)?;
            self.work(primitive.bytes())?;
            Value::Number(primitive.script_number()?)
        } else {
            self.checked_value(value)?
        };
        self.pin_exported_value(&value);
        self.validate_native_access(handle,host)?;
        if let Err((kind,message)) = host.validate_set(handle, name, &value) {
            if host.setter_dom_exception(handle,name,kind) {
                return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));
            }
            return Err(Fault::exception(kind,message));
        }
        let ticket=self.prepare_host_microtask(host,handle,name,std::slice::from_ref(&value),true)?;
        let reflected = host.observed_attribute_setter(handle, name);
        let observation = self.prepare_setter_observation(handle,name,&value,reflected,host);
        let (observation,tree_observation) = match observation {
            Ok(observation) => observation,
            Err(error) => {
                self.finish_host_microtask(host, ticket)?;
                return Err(error);
            }
        };
        host.native_operation_caller(self.native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(window, _)| window)));
        let result=if let Some((offset,removed,inserted))=splice{host.set_character_data_splice(handle,value,offset,removed,inserted)}else{host.set(handle,name,value)}.map_err(Fault::new);
        if tree_observation {
            self.finish_tree_observation(observation, host)?;
        } else {
            self.finish_character_data_write(observation, name, host)?;
        }
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
        if inherited && host.expando_owner(handle).is_some() && !host.ordinary_inherited_enumeration(handle) {
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
    fn element_inline_style_property_ignored_arguments_do_not_become_permanent_host_roots() {
        let mut runtime = Runtime::for_test();
        let mut host = Native { enabled: true, reads: 0 };
        let id = runtime.new_object(None).unwrap();
        for (operation, arguments) in [
            (ELEMENT_STYLE_GET, vec![Value::Object(id)]),
            (ELEMENT_STYLE_SET, vec![Value::String(String::new()),Value::Object(id)]),
        ] {
            let error = runtime.call_host(42, operation, &arguments, &mut host).unwrap_err();
            assert!(error.message.contains("No native call"), "{}", error.message);
            assert!(!runtime.exported_object_roots.contains(&id));
        }
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
