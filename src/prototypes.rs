//! Original ordinary-object prototype support. Links and callable properties
//! share the interpreter's arenas and budgets; no host object can be inserted
//! into a script-controlled prototype chain. Traversal is iterative.
//!
//! ECMAScript references: OrdinaryGet/SetPrototypeOf, OrdinaryHasInstance,
//! Object.create/getPrototypeOf/setPrototypeOf, and ordinary property descriptors.
//! Accessors use the original receiver and share the normal execution budgets.
//! Proxy prototype queries use the live internal method in proxy_operations.

use super::{
    strict_equal, Accessors, Builtin, Fault, Function, Host, Ordinary, PropertyKey, Runtime,
    StringKey, Value,
};
use std::collections::HashSet;

enum DescriptorTarget {
    Ordinary(Ordinary),
    GlobalBinding,
}

enum OwnPropertyQuery {
    Descriptor,
    Present,
    Enumerable,
}

struct PropertyDescriptor {
    value: Option<Value>,
    enumerable: Option<bool>,
    writable: Option<bool>,
    configurable: Option<bool>,
    get: Option<Option<usize>>,
    set: Option<Option<usize>>,
}

impl PropertyDescriptor {
    fn is_accessor(&self) -> bool {
        self.get.is_some() || self.set.is_some()
    }

    fn is_data(&self) -> bool {
        self.value.is_some() || self.writable.is_some()
    }
}

impl Runtime {
    pub(super) fn initialize_prototype_roots(&mut self) -> Result<(), Fault> {
        self.object_prototype = Some(self.new_object(None)?);
        let Value::Function(function) = self.native_function(Builtin::EmptyFunction, "")? else {
            unreachable!()
        };
        self.function_prototype = Some(function);
        Ok(())
    }

    pub(super) fn native_function(&mut self, builtin: Builtin, name: &str) -> Result<Value, Fault> {
        if self.function_count_refused(1) {
            let requested = match builtin {
                Builtin::ArrayMethod(_) => "native-array-method",
                Builtin::StringMethod(_) => "native-string-method",
                Builtin::ObjectStatic(_) | Builtin::ObjectMethod(_) => "native-object-method",
                Builtin::Binary(_) => "native-binary",
                Builtin::Collection(_) => "native-collection",
                Builtin::Promise(_) => "native-promise",
                Builtin::Date(_) => "native-date",
                Builtin::Url(_) => "native-url",
                Builtin::Rect(_) => "native-rect",
                Builtin::ImageData(_) => "native-image-data",
                Builtin::DomException(_) => "native-dom-exception",
                Builtin::TextEncoding(_) => "native-text-encoding",
                _ => "native-core",
            };
            return Err(self.function_limit_fault(requested));
        }
        let length = match builtin {
            Builtin::Binary(operation) => operation.arity(),
            Builtin::Collection(operation) => operation.arity(),
            Builtin::Iterable(operation) => operation.arity(),
            Builtin::Iterator(_) => 0,
            Builtin::TextEncoding(operation) => operation.arity(),
            Builtin::Symbol(operation) => operation.arity(),
            Builtin::Promise(operation) => operation.arity(),
            Builtin::Date(operation) => operation.arity(),
            Builtin::Url(operation) => operation.arity(),
            Builtin::Headers(operation) => operation.arity(),
            Builtin::Response(operation) => operation.arity(),
            Builtin::Rect(operation) => operation.arity(),
            Builtin::ImageData(super::image_data_api::Op::Constructor) => 2,
            Builtin::ImageData(_) => 0,
            Builtin::DomException(operation) => operation.arity(),
            Builtin::MutationObserver(operation) => operation.arity(),
            Builtin::TreeWalker(operation) => operation.arity(),
            Builtin::EmptyFunction | Builtin::ThrowTypeError => 0,
            Builtin::Proxy | Builtin::RegExp | Builtin::ParseInt | Builtin::JsonParse | Builtin::ReflectGet => 2,
            Builtin::JsonStringify | Builtin::ReflectSet => 3,
            Builtin::BigIntWidth(_) => 2,
            Builtin::Math("random") => 0,
            Builtin::Math("atan2" | "hypot" | "imul" | "max" | "min" | "pow") => 2,
            Builtin::ObjectStatic(
                "create"
                | "hasOwn"
                | "is"
                | "setPrototypeOf"
                | "getOwnPropertyDescriptor"
                | "defineProperties"
                | "assign",
            ) => 2,
            Builtin::ObjectStatic("defineProperty") => 3,
            Builtin::FunctionMethod("apply") => 2,
            Builtin::FunctionMethod("toString")
            | Builtin::ObjectMethod("toString" | "valueOf" | "\0get __proto__")
            | Builtin::ErrorToString => 0,
            Builtin::ArrayMethod(
                "pop" | "toString" | "shift" | "reverse" | "toReversed" | "flat",
            ) => 0,
            Builtin::ArrayMethod("slice" | "splice" | "copyWithin" | "with" | "toSpliced") => 2,
            Builtin::RegExpMethod("toString") => 0,
            Builtin::StringMethod(
                "toString" | "valueOf" | "trim" | "toLowerCase" | "toUpperCase",
            ) => 0,
            Builtin::StringMethod("split" | "slice" | "substring" | "substr" | "replace" | "replaceAll") => 2,
            Builtin::PrimitiveMethod("toString" | "toFixed", true) => 1,
            Builtin::PrimitiveMethod(_, _) | Builtin::BigIntMethod(false) => 0,
            _ => 1,
        };
        let allocation = self.insert_function_slot(Function::Native(builtin))?;
        self.initialize_function_properties(allocation, name, length, false)?;
        Ok(Value::Function(allocation.id))
    }

    pub(super) fn initialize_function_properties(
        &mut self,
        allocation: super::function_arena::Allocation,
        name: &str,
        length: usize,
        constructible: bool,
    ) -> Result<(), Fault> {
        // Every factory enters here, including deferred native materialization.
        // Parallel function metadata is incomplete until initialization finishes.
        let marker = self.inhibit_transient_collection();
        let result = self.initialize_function_properties_unpublished(allocation, name, length, constructible);
        self.restore_transient_marker(marker);
        result
    }

    fn initialize_function_properties_unpublished(
        &mut self,
        allocation: super::function_arena::Allocation,
        name: &str,
        length: usize,
        constructible: bool,
    ) -> Result<(), Fault> {
        let index = allocation.id;
        let properties = match self.new_object(None) {
            Ok(properties) => properties,
            Err(error) => {
                self.rollback_unpublished_function(allocation)?;
                return Err(error);
            }
        };
        self.function_properties[index] = properties;
        if matches!(self.functions[index], Function::Script { .. }) {
            self.function_arena.script_created = self.function_arena.script_created.saturating_add(1);
        }
        if !matches!(self.functions[index], Function::Script { .. } | Function::Bound(_)) {
            self.work(name.len())?;
            if self.value_bytes.saturating_add(name.len()) > self.limits.heap.retained_values {
                return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
            }
            self.function_initial_names[index] = Some(name.into());
            self.value_bytes += name.len();
        }
        if let Some(prototype) = self.function_prototype {
            self.objects[properties].prototype = Some(Ordinary::Function(prototype));
        }
        self.store_property(
            properties,
            "length".into(),
            Value::Number(length as f64),
            Some((false, false, true)),
        )?;
        self.store_property(
            properties,
            "name".into(),
            Value::String(name.into()),
            Some((false, false, true)),
        )?;
        if constructible {
            let prototype = self.new_object(None)?;
            self.store_property(
                prototype,
                "constructor".into(),
                Value::Function(index),
                Some((false, true, true)),
            )?;
            self.store_property(
                properties,
                "prototype".into(),
                Value::Object(prototype),
                Some((false, true, false)),
            )?;
        }
        Ok(())
    }

    pub(super) fn prototype_method(
        &mut self,
        owner: usize,
        name: &str,
        builtin: Builtin,
    ) -> Result<(), Fault> {
        // Keep the scan, dispatch and sparse-array primitives ready for their
        // existing allocation-free operation paths and exact work accounting.
        if matches!(builtin, Builtin::StringMethod("charCodeAt")
            | Builtin::FunctionMethod("call") | Builtin::ArrayMethod("join")) {
            let value = self.native_function(builtin, name)?;
            return self.store_property(owner, name.into(), value, Some((false, true, true)));
        }
        self.deferred_native_method(owner, name, builtin)
    }

    pub(super) fn native_accessor(
        &mut self,
        owner: usize,
        name: &str,
        builtin: Builtin,
        field: super::NativeGetter,
    ) -> Result<(), Fault> {
        let getter = self.native_function(builtin, &format!("get {name}"))?;
        self.store_property(owner, name.into(), getter, Some((false, false, true)))?;
        self.objects[owner].properties.get_mut(name).unwrap().getter = Some(field);
        Ok(())
    }

    pub(super) fn native_getter(
        &mut self,
        field: super::NativeGetter,
        receiver: Value,
    ) -> Result<Value, Fault> {
        match field {
            super::NativeGetter::ArgumentsCallee => Err(Fault::exception(
                "TypeError",
                "Strict arguments callee access is forbidden.",
            )),
            super::NativeGetter::Binary(field) => self.binary_getter(field, receiver),
            super::NativeGetter::CollectionSize(kind) => self.collection_size(receiver, kind),
            super::NativeGetter::TextEncoding(field) => self.text_getter(field, receiver),
            super::NativeGetter::SymbolDescription => self.symbol_description(receiver),
        }
    }

    pub(super) fn constructor_prototype(
        &mut self,
        name: &str,
        prototype: Value,
    ) -> Result<(), Fault> {
        let constructor = self.binding_value(self.global_environment, name)?;
        let id = self.ordinary_id(Ordinary::from_value(&constructor).unwrap())?;
        self.store_property(
            id,
            "prototype".into(),
            prototype.clone(),
            Some((false, false, false)),
        )?;
        let id = self.ordinary_id(Ordinary::from_value(&prototype).unwrap())?;
        self.store_property(
            id,
            "constructor".into(),
            constructor,
            Some((false, true, true)),
        )
    }

    pub(super) fn initialize_intrinsic_prototypes(&mut self) -> Result<(), Fault> {
        let object = self.object_prototype.unwrap();
        let function = self.function_prototype.unwrap();
        self.constructor_prototype("Object", Value::Object(object))?;
        self.constructor_prototype("Function", Value::Function(function))?;
        for name in [
            "hasOwnProperty",
            "propertyIsEnumerable",
            "isPrototypeOf",
            "toString",
            "valueOf",
        ] {
            self.prototype_method(object, name, Builtin::ObjectMethod(name))?;
        }
        let Value::Function(get) = self.native_function(Builtin::ObjectMethod("\0get __proto__"), "get __proto__")? else { unreachable!() };
        let Value::Function(set) = self.native_function(Builtin::ObjectMethod("\0set __proto__"), "set __proto__")? else { unreachable!() };
        self.store_accessor_property(object, "__proto__".into(), Accessors { get: Some(get), set: Some(set) }, (false, true))?;
        let function_properties = self.ordinary_id(Ordinary::Function(function))?;
        for name in ["call", "apply", "bind", "toString"] {
            self.prototype_method(function_properties, name, Builtin::FunctionMethod(name))?;
        }
        let Value::Function(object_constructor) = self.binding_value(self.global_environment, "Object")? else {
            unreachable!()
        };
        let object_constructor = self.ordinary_id(Ordinary::Function(object_constructor))?;
        for name in [
            "keys",
            "getOwnPropertyNames",
            "getOwnPropertyDescriptors",
            "values",
            "entries",
            "hasOwn",
            "is",
            "create",
            "getPrototypeOf",
            "setPrototypeOf",
            "getOwnPropertyDescriptor",
            "getOwnPropertySymbols",
            "defineProperty",
            "defineProperties",
            "assign",
            "preventExtensions",
            "seal",
            "freeze",
            "isExtensible",
            "isSealed",
            "isFrozen",
        ] {
            self.prototype_method(object_constructor, name, Builtin::ObjectStatic(name))?;
        }
        let array = self.new_object(Some(0))?;
        self.array_prototype = Some(array);
        self.constructor_prototype("Array", Value::Object(array))?;
        for name in [
            "push",
            "pop",
            "join",
            "indexOf",
            "includes",
            "slice",
            "toString",
            "shift",
            "unshift",
            "splice",
            "map",
            "filter",
            "forEach",
            "some",
            "every",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "reduce",
            "reduceRight",
            "at",
            "lastIndexOf",
            "reverse",
            "fill",
            "copyWithin",
            "concat",
            "sort",
            "toSorted",
            "toReversed",
            "with",
            "toSpliced",
            "flat",
            "flatMap",
        ] {
            self.prototype_method(array, name, Builtin::ArrayMethod(name))?;
        }
        let Value::Function(array_constructor) = self.binding_value(self.global_environment, "Array")? else {
            unreachable!()
        };
        let array_constructor = self.ordinary_id(Ordinary::Function(array_constructor))?;
        self.prototype_method(
            array_constructor,
            "isArray",
            Builtin::ObjectStatic("isArray"),
        )?;
        let regexp = self.new_object(None)?;
        self.regexp_prototype = Some(regexp);
        self.constructor_prototype("RegExp", Value::Object(regexp))?;
        for name in ["test", "exec", "toString"] {
            self.prototype_method(regexp, name, Builtin::RegExpMethod(name))?;
        }
        for name in [
            "Error",
            "EvalError",
            "RangeError",
            "ReferenceError",
            "SyntaxError",
            "TypeError",
            "URIError",
        ] {
            let prototype = self.new_object(None)?;
            self.objects[prototype].error_name = Some(name);
            if let Some(error) = self.error_prototypes.get("Error") {
                self.objects[prototype].prototype = Some(Ordinary::Object(*error));
            }
            self.error_prototypes.insert(name, prototype);
            self.constructor_prototype(name, Value::Object(prototype))?;
            self.store_property(
                prototype,
                "name".into(),
                Value::String(name.into()),
                Some((false, true, true)),
            )?;
            self.store_property(
                prototype,
                "message".into(),
                Value::String(String::new()),
                Some((false, true, true)),
            )?;
            if name == "Error" {
                self.prototype_method(prototype, "toString", Builtin::ErrorToString)?;
            }
        }
        let string = self.new_object(None)?;
        self.string_prototype = Some(string);
        self.initialize_string_object(string, String::new())?;
        self.constructor_prototype("String", Value::Object(string))?;
        let Value::Function(string_constructor) = self.binding_value(self.global_environment, "String")? else {
            unreachable!()
        };
        let string_constructor = self.ordinary_id(Ordinary::Function(string_constructor))?;
        self.prototype_method(
            string_constructor,
            "fromCharCode",
            Builtin::StringFromCharCode,
        )?;
        for name in [
            "toString",
            "valueOf",
            "split",
            "slice",
            "substring",
            "substr",
            "concat",
            "charAt",
            "charCodeAt",
            "codePointAt",
            "trim",
            "toLowerCase",
            "toUpperCase",
            "includes",
            "indexOf",
            "lastIndexOf",
            "startsWith",
            "endsWith",
            "search",
            "match",
            "replace",
            "replaceAll",
        ] {
            self.prototype_method(string, name, Builtin::StringMethod(name))?;
        }
        for (name, number) in [("Number", true), ("Boolean", false)] {
            let prototype = self.new_object(None)?;
            if number {
                self.number_prototype = Some(prototype);
                self.objects[prototype].primitive =
                    Some(super::primitive_wrappers::Data::Number(0.0));
            } else {
                self.boolean_prototype = Some(prototype);
                self.objects[prototype].primitive =
                    Some(super::primitive_wrappers::Data::Boolean(false));
            }
            self.constructor_prototype(name, Value::Object(prototype))?;
            for method in ["toString", "valueOf"] {
                self.prototype_method(prototype, method, Builtin::PrimitiveMethod(method, number))?;
            }
            if number {
                self.prototype_method(prototype, "toFixed", Builtin::PrimitiveMethod("toFixed", true))?;
            }
        }
        Ok(())
    }

    pub(super) fn ordinary_id(&self, value: Ordinary) -> Result<usize, Fault> {
        match value {
            Ordinary::Object(id) => {
                self.object(id)?;
                Ok(id)
            }
            Ordinary::Function(id) => self
                .function_properties
                .get(id)
                .copied()
                .filter(|properties| self.function_is_occupied(id) && *properties != usize::MAX)
                .ok_or_else(|| Fault::new("Invalid function handle.")),
        }
    }

    pub(super) fn ordinary_prototype(&self, value: Ordinary) -> Result<Option<Ordinary>, Fault> {
        let id = self.ordinary_id(value)?;
        self.reject_unsupported_proxy_operation(id, "getPrototypeOf without an execution context")?;
        Ok(self.objects[id].prototype)
    }

    pub(super) fn owns_property(&self, id: usize, name: &str) -> Result<bool, Fault> {
        let object = self.object(id)?;
        if let Some(index) = self.typed_numeric_key(id, name)? {
            return Ok(self.typed_index_valid(id, index));
        }
        Ok(object.properties.contains_key(name)
            || self.string_object_index(id, name).is_some()
            || name == "length" && object.length.is_some()
            || object.regexp.is_some()
                && matches!(
                    name,
                    "source"
                        | "flags"
                        | "global"
                        | "ignoreCase"
                        | "multiline"
                        | "dotAll"
                        | "sticky"
                        | "unicode"
                        | "unicodeSets"
                        | "hasIndices"
                ))
    }

    pub(super) fn property_owner(
        &mut self,
        value: Ordinary,
        name: &str,
    ) -> Result<Option<usize>, Fault> {
        let owner = self.property_dispatch_owner(value, name)?;
        self.ordinary_descriptor_owner(owner)
    }

    // A proxy intercepts a lookup even if its target lacks an own key. Only
    // callers with a live receiver and execution context may dispatch the trap.
    pub(super) fn property_dispatch_owner(
        &mut self,
        mut value: Ordinary,
        name: &str,
    ) -> Result<Option<usize>, Fault> {
        loop {
            self.step()?;
            let id = self.ordinary_id(value)?;
            if self.objects[id].proxy.is_some() || self.owns_property(id, name)? {
                return Ok(Some(id));
            }
            // Integer-indexed exotic objects do not consult their prototype
            // for a canonical numeric key that is outside the view.
            if self.typed_numeric_key(id, name)?.is_some() {
                return Ok(None);
            }
            let Some(prototype) = self.objects[id].prototype else {
                return Ok(None);
            };
            value = prototype;
        }
    }

    pub(super) fn get_ordinary_property(
        &mut self,
        value: Ordinary,
        name: &str,
    ) -> Result<Value, Fault> {
        self.property_name(name)?;
        match self.property_owner(value, name)? {
            Some(owner) => {
                if self.objects[owner]
                    .properties
                    .get(name)
                    .is_some_and(|property| property.accessor.is_some())
                {
                    return Err(Fault::new(
                        "Script accessor read requires an execution context.",
                    ));
                }
                if let Some(field) = self.objects[owner]
                    .properties
                    .get(name)
                    .and_then(|property| property.getter)
                {
                    let function = self.objects[owner].properties[name].value.as_value()?.clone();
                    self.native_getter_in_realm(&function, field, value.value())
                } else {
                    self.own_property(owner, name)
                }
            }
            None => Ok(Value::Undefined),
        }
    }

    pub(super) fn get_ordinary_property_with_host(
        &mut self,
        object: Ordinary,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.get_ordinary_property_with_receiver(object, name, &object.value(), host)
    }

    pub(super) fn get_ordinary_property_with_receiver(
        &mut self,
        object: Ordinary,
        name: &str,
        receiver: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.property_name(name)?;
        let Some(owner) = self.property_dispatch_owner(object, name)? else {
            return Ok(Value::Undefined);
        };
        if self.objects[owner].proxy.is_some() {
            self.work(name.len())?;
            return self.proxy_get(owner, PropertyKey::String(name.to_owned()), receiver, host);
        }
        self.materialize_host_accessor(owner, name, false)?;
        if let Some(accessor) = self.objects[owner]
            .properties
            .get(name)
            .and_then(|property| property.accessor)
        {
            // Resolve once before materializing: data properties and absent
            // getters never need to copy the original primitive receiver.
            let receiver = if accessor.get.is_some() {
                self.copy_value(receiver)?
            } else {
                Value::Undefined
            };
            return self.call_accessor(accessor.get, receiver, &[], host);
        }
        if let Some(field) = self.objects[owner]
            .properties
            .get(name)
            .and_then(|property| property.getter)
        {
            let receiver = self.copy_value(receiver)?;
            let function = self.objects[owner].properties[name].value.as_value()?.clone();
            return self.native_getter_in_realm(&function, field, receiver);
        }
        self.own_property(owner, name)
    }

    pub(super) fn assign_ordinary_property(
        &mut self,
        object: Ordinary,
        name: String,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.assign_ordinary_property_with_receiver(object, name, value, object.value(), host)
    }

    pub(super) fn assign_ordinary_property_with_receiver(
        &mut self,
        object: Ordinary,
        name: String,
        value: Value,
        receiver: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.property_name(&name)?;
        let id = self.ordinary_id(object)?;
        if let Some(index) = self.typed_numeric_key(id, &name)? {
            let number = self.binary_number(&value, host)?;
            if let Some(view) = self
                .typed_view(id)
                .filter(|_| self.typed_index_valid(id, index))
            {
                self.typed_write(view, index.unwrap(), number)?;
            }
            return Ok(());
        }
        if let Some(owner) = self.property_dispatch_owner(object, &name)? {
            if self.objects[owner].proxy.is_some() {
                return self.assign_via_proxy(owner, PropertyKey::String(name), &value, &receiver, host);
            }
            self.materialize_host_accessor(owner, &name, true)?;
            if let Some(accessor) = self.objects[owner]
                .properties
                .get(&name)
                .and_then(|property| property.accessor)
            {
                if accessor.set.is_none() {
                    return if self.strict {
                        Err(Fault::exception("TypeError", "The property has no setter."))
                    } else {
                        Ok(())
                    };
                }
                self.call_accessor(accessor.set, receiver, &[value], host)?;
                return Ok(());
            }
            if self.objects[owner]
                .properties
                .get(&name)
                .is_some_and(|property| {
                    matches!(property.getter, Some(super::NativeGetter::ArgumentsCallee))
                })
            {
                return Err(Fault::exception(
                    "TypeError",
                    "Strict arguments callee access is forbidden.",
                ));
            }
            if self.objects[owner]
                .properties
                .get(&name)
                .is_some_and(|property| !property.writable)
                || self.string_object_index(owner, &name).is_some()
                || name == "length"
                    && self.objects[owner].length.is_some()
                    && !self.objects[owner].length_writable
            {
                return if self.strict {
                    Err(Fault::exception("TypeError", "The property is read-only."))
                } else {
                    Ok(())
                };
            }
        }
        if name == "length" && self.objects[id].length.is_some() {
            // Ordinary Set has already checked the current property above.
            // Its value can now run two conversions and change this array, so
            // the descriptor application must read the live length again.
            let length = self.array_length_value(&value, host)?;
            if !self.apply_array_length(id, Some(length), None, true)? && self.strict {
                return Err(Fault::exception("TypeError", "Cannot change array length."));
            }
            return Ok(());
        }
        self.set_property(id, name, value)
    }

    pub(super) fn assign_inherited_property(
        &mut self, base: Ordinary, name: String, value: Value, receiver: Value, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.property_name(&name)?;
        if let Some(owner) = self.property_dispatch_owner(base, &name)? {
            if self.objects[owner].proxy.is_some() {
                return self.assign_via_proxy(owner, PropertyKey::String(name), &value, &receiver, host);
            }
            self.materialize_host_accessor(owner, &name, true)?;
            if let Some(accessor) = self.objects[owner].properties.get(&name).and_then(|property| property.accessor) {
                if accessor.set.is_none() {
                    return if self.strict { Err(Fault::exception("TypeError", "The inherited property has no setter.")) } else { Ok(()) };
                }
                self.call_accessor(accessor.set, receiver, &[value], host)?;
                return Ok(());
            }
            let readonly = self.objects[owner].properties.get(&name)
                .is_some_and(|property| property.getter.is_some() || !property.writable)
                || self.string_object_index(owner, &name).is_some()
                || name == "length" && self.objects[owner].length.is_some() && !self.objects[owner].length_writable
                || self.objects[owner].regexp.is_some() && !self.objects[owner].properties.contains_key(&name);
            if readonly {
                return if self.strict { Err(Fault::exception("TypeError", "The inherited property is read-only.")) } else { Ok(()) };
            }
        }
        if matches!(receiver, Value::Host(_)) {
            return Err(Fault::new("Creating an own data property on this native receiver is not implemented."));
        }
        let Some(receiver) = Ordinary::from_value(&receiver) else {
            return if self.strict { Err(Fault::exception("TypeError", "A super data receiver must be an object.")) } else { Ok(()) };
        };
        let id = self.ordinary_id(receiver)?;
        if self.objects[id].proxy.is_some() {
            let set = self.proxy_receiver_set_data(id, PropertyKey::String(name), &value, host)?;
            return self.proxy_assignment_result(set);
        }
        if self.uses_stored_property_descriptors(id) {
            let set = self.set_stored_receiver_data(id, PropertyKey::String(name), |_| Ok(value))?;
            return if !set && self.strict {
                Err(Fault::exception("TypeError", "The super receiver property cannot be written."))
            } else { Ok(()) };
        }
        if self.objects[id].properties.get(&name).is_some_and(|property|
            property.accessor.is_some() || property.getter.is_some() || !property.writable)
            || self.string_object_index(id, &name).is_some()
            || name == "length" && self.objects[id].length.is_some() && !self.objects[id].length_writable
            || self.objects[id].regexp.is_some() && self.owns_property(id, &name)? && !self.objects[id].properties.contains_key(&name)
        {
            return if self.strict { Err(Fault::exception("TypeError", "The super receiver property is read-only or an accessor.")) } else { Ok(()) };
        }
        let existing = self.owns_property(id, &name)?;
        if existing {
            // Only a checked writable own data descriptor reaches this path.
            // Preserve existing exotic/length conversion and strictness rules.
            return self.assign_ordinary_property_with_receiver(receiver, name, value, receiver.value(), host);
        }
        if !self.objects[id].extensible
            || self.objects[id].length.is_some_and(|length|
                super::property_index(&name).is_some_and(|index| index >= length) && !self.objects[id].length_writable)
        {
            return if self.strict { Err(Fault::exception("TypeError", "A super receiver property cannot be created.")) } else { Ok(()) };
        }
        if let Some(index) = self.typed_numeric_key(id, &name)? {
            if !self.typed_index_valid(id, index) {
                return if self.strict { Err(Fault::exception("TypeError", "The super receiver index is invalid.")) } else { Ok(()) };
            }
        }
        // Create directly on the receiver, never invoke its inherited setter.
        self.store_property(id, name, value, Some((true, true, true)))
    }

    pub(super) fn set_prototype(
        &mut self,
        object: Ordinary,
        prototype: Option<Ordinary>,
    ) -> Result<(), Fault> {
        self.step()?;
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "setPrototypeOf")?;
        if self.objects[id].prototype == prototype {
            return Ok(());
        }
        if !self.objects[id].extensible {
            return Err(Fault::exception(
                "TypeError",
                "The object is not extensible.",
            ));
        }
        if self.objects[id].immutable_prototype || self.immutable_object_prototype(id) {
            return Err(Fault::exception(
                "TypeError",
                "This object has an immutable prototype.",
            ));
        }
        let mut current = prototype;
        while let Some(parent) = current {
            self.step()?;
            if parent == object {
                return Err(Fault::exception(
                    "TypeError",
                    "A prototype chain cannot contain a cycle.",
                ));
            }
            // OrdinarySetPrototypeOf stops its cycle check at an exotic
            // [[GetPrototypeOf]]. Installing a proxy never calls its trap.
            let parent_id = self.ordinary_id(parent)?;
            if self.objects[parent_id].proxy.is_some() {
                break;
            }
            current = self.objects[parent_id].prototype;
        }
        self.objects[id].prototype = prototype;
        Ok(())
    }

    pub(super) fn inherits_from(
        &mut self,
        object: Ordinary,
        prototype: Ordinary,
        host: &mut dyn Host,
    ) -> Result<bool, Fault> {
        let sought = prototype.value();
        let mut current = self.prototype_with_host(object, &sought, host)?;
        while let Some(parent) = current {
            self.step()?;
            if parent == prototype {
                return Ok(true);
            }
            current = self.prototype_with_host(parent, &sought, host)?;
        }
        Ok(false)
    }

    pub(super) fn inherited_keys(&mut self, object: Ordinary) -> Result<Vec<StringKey>, Fault> {
        let mut current = Some(object);
        let mut seen = HashSet::new();
        let mut keys = Vec::new();
        while let Some(object) = current {
            self.step()?;
            let id = self.ordinary_id(object)?;
            for key in self.enumerable_keys(id)? {
                if seen.insert(key.clone()) {
                    if keys.len() >= self.limits.heap.object_properties {
                        return Err(Fault::new(format!("for-in enumeration limit reached ({} keys).", self.limits.heap.object_properties)));
                    }
                    keys.push(key);
                }
            }
            // A nonenumerable own property also shadows an inherited name.
            self.work_as(
                self.objects[id]
                    .properties
                    .len()
                    .saturating_mul(self.limits.capacity.property_bytes),
                super::WorkKind::PropertyEnumeration,
            )?;
            seen.extend(self.objects[id].properties.keys().map(|key| key.to_owned()));
            if self.objects[id].length.is_some() {
                seen.insert("length".into());
            }
            current = self.objects[id].prototype;
        }
        Ok(keys)
    }

    pub(super) fn ordinary_required(&self, value: &Value) -> Result<Ordinary, Fault> {
        if let Some(object) = Ordinary::from_value(value) {
            self.ordinary_id(object)?;
            Ok(object)
        } else if matches!(value, Value::Host(_)) {
            Err(Fault::new(
                "Native host object prototypes and descriptors are not implemented.",
            ))
        } else {
            Err(Fault::exception(
                "TypeError",
                "An ordinary object or function is required.",
            ))
        }
    }

    pub(super) fn prototype_argument(&self, value: &Value) -> Result<Option<Ordinary>, Fault> {
        if matches!(value, Value::Null) {
            return Ok(None);
        }
        self.ordinary_required(value).map(Some)
    }

    // The supported Proxy endpoints have ordinary stored own descriptors.
    // Read the requested fields only after the live trap lookup, which can
    // change or delete the property even if it returns null or undefined.
    fn proxy_own_property(
        &mut self,
        proxy: usize,
        key: &PropertyKey,
        query: OwnPropertyQuery,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.with_proxy_operation(proxy, &Value::Undefined, None, |runtime, target, handler| {
            runtime.proxy_descriptor_forward(handler, "getOwnPropertyDescriptor", host)?;
            match query {
                OwnPropertyQuery::Descriptor => match key {
                    PropertyKey::String(name) => {
                        runtime.own_descriptor(Ordinary::Object(target), name)
                    }
                    PropertyKey::StringUnits(units) => {
                        runtime.own_units_descriptor(Ordinary::Object(target), units)
                    }
                    PropertyKey::Symbol(symbol) => {
                        runtime.own_symbol_descriptor(Ordinary::Object(target), *symbol)
                    }
                },
                OwnPropertyQuery::Present => {
                    Ok(Value::Bool(runtime.stored_descriptor_property(target, key).is_some()))
                }
                OwnPropertyQuery::Enumerable => Ok(Value::Bool(
                    runtime.stored_descriptor_property(target, key).is_some_and(|p| p.enumerable),
                )),
            }
        })
    }

    pub(super) fn own_descriptor(&mut self, object: Ordinary, name: &str) -> Result<Value, Fault> {
        self.property_name(name)?;
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "getOwnPropertyDescriptor")?;
        if !self.owns_property(id, name)? {
            return Ok(Value::Undefined);
        }
        self.materialize_host_accessor(id, name, false)?;
        self.materialize_host_accessor(id, name, true)?;
        if let Some(property) = self.objects[id].properties.get(name) {
            if let Some(accessor) = property.accessor {
                return self.accessor_descriptor(
                    accessor,
                    property.enumerable,
                    property.configurable,
                );
            }
        }
        if let Some(field) = self.objects[id]
            .properties
            .get(name)
            .and_then(|property| property.getter)
        {
            let getter = self.copy_value(&self.objects[id].properties[name].value.as_value()?.clone())?;
            let enumerable = self.objects[id].properties[name].enumerable;
            let configurable = self.objects[id].properties[name].configurable;
            let descriptor = self.new_object(None)?;
            let setter = if matches!(field, super::NativeGetter::ArgumentsCallee) {
                self.copy_value(&getter)?
            } else {
                Value::Undefined
            };
            for (name, value) in [
                ("get", getter),
                ("set", setter),
                ("enumerable", Value::Bool(enumerable)),
                ("configurable", Value::Bool(configurable)),
            ] {
                self.set_property(descriptor, name.into(), value)?;
            }
            return Ok(Value::Object(descriptor));
        }
        if self.objects[id].regexp.is_some() && !self.objects[id].properties.contains_key(name) {
            return Err(Fault::new(
                "RegExp accessor descriptor inspection is not implemented.",
            ));
        }
        let value = self.own_property(id, name)?;
        let (enumerable, writable, configurable) = self.objects[id]
            .properties
            .get(name)
            .map(|property| {
                (
                    property.enumerable,
                    property.writable,
                    property.configurable,
                )
            })
            .unwrap_or_else(|| {
                if self.typed_numeric_key(id, name).ok().flatten().is_some() {
                    (true, true, true)
                } else if self.string_object_index(id, name).is_some() {
                    (true, false, false)
                } else {
                    (
                        false,
                        name == "length" && self.objects[id].length_writable,
                        false,
                    )
                }
            });
        let descriptor = self.new_object(None)?;
        for (name, value) in [
            ("value", value),
            ("writable", Value::Bool(writable)),
            ("enumerable", Value::Bool(enumerable)),
            ("configurable", Value::Bool(configurable)),
        ] {
            self.set_property(descriptor, name.into(), value)?;
        }
        Ok(Value::Object(descriptor))
    }

    pub(super) fn own_units_descriptor(&mut self, object: Ordinary, units: &[u16]) -> Result<Value, Fault> {
        self.property_name_units(units)?;
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "getOwnPropertyDescriptor")?;
        let Some(property) = self.objects[id].properties.get_units(units) else {
            return Ok(Value::Undefined);
        };
        if let Some(accessor) = property.accessor {
            return self.accessor_descriptor(accessor, property.enumerable, property.configurable);
        }
        let (enumerable, writable, configurable) =
            (property.enumerable, property.writable, property.configurable);
        let value = self.own_units_property(id, units)?;
        let descriptor = self.new_object(None)?;
        for (name, value) in [
            ("value", value),
            ("writable", Value::Bool(writable)),
            ("enumerable", Value::Bool(enumerable)),
            ("configurable", Value::Bool(configurable)),
        ] {
            self.set_property(descriptor, name.into(), value)?;
        }
        Ok(Value::Object(descriptor))
    }

    pub(super) fn own_units_enumerable(&mut self, id: usize, units: &[u16]) -> Result<bool, Fault> {
        self.property_name_units(units)?;
        self.owns_units_property(id, units)?;
        Ok(self.objects[id].properties.get_units(units).is_some_and(|property| property.enumerable))
    }

    fn own_symbol_descriptor(&mut self, object: Ordinary, key: usize) -> Result<Value, Fault> {
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "getOwnPropertyDescriptor")?;
        let Some(property) = self.objects[id].symbol_properties.get(&key) else {
            return Ok(Value::Undefined);
        };
        if let Some(accessor) = property.accessor {
            return self.accessor_descriptor(accessor, property.enumerable, property.configurable);
        }
        let (enumerable, writable, configurable, getter) = (
            property.enumerable,
            property.writable,
            property.configurable,
            property.getter,
        );
        self.work(property.value.bytes())?;
        let value = self.objects[id].symbol_properties[&key].value.as_value()?.clone();
        let descriptor = self.new_object(None)?;
        if getter.is_some() {
            self.set_property(descriptor, "get".into(), value)?;
            self.set_property(descriptor, "set".into(), Value::Undefined)?;
        } else {
            self.set_property(descriptor, "value".into(), value)?;
            self.set_property(descriptor, "writable".into(), Value::Bool(writable))?;
        }
        self.set_property(descriptor, "enumerable".into(), Value::Bool(enumerable))?;
        self.set_property(descriptor, "configurable".into(), Value::Bool(configurable))?;
        Ok(Value::Object(descriptor))
    }

    fn accessor_descriptor(
        &mut self,
        accessor: Accessors,
        enumerable: bool,
        configurable: bool,
    ) -> Result<Value, Fault> {
        let descriptor = self.new_object(None)?;
        for (name, value) in [
            (
                "get",
                accessor.get.map_or(Value::Undefined, Value::Function),
            ),
            (
                "set",
                accessor.set.map_or(Value::Undefined, Value::Function),
            ),
            ("enumerable", Value::Bool(enumerable)),
            ("configurable", Value::Bool(configurable)),
        ] {
            self.set_property(descriptor, name.into(), value)?;
        }
        Ok(Value::Object(descriptor))
    }

    fn define_symbol_data_property(
        &mut self,
        target: &Value,
        key: usize,
        descriptor: PropertyDescriptor,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let object = self.symbol_backing(target, host)?.ok_or_else(|| {
            Fault::new("Symbol properties of this native object are not implemented.")
        })?;
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "defineProperty")?;
        self.define_stored_property(id, PropertyKey::Symbol(key), descriptor)
    }

    fn define_units_data_property(
        &mut self,
        target: &Value,
        units: Vec<u16>,
        descriptor: PropertyDescriptor,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.property_name_units(&units)?;
        let object = self.symbol_backing(target, host)?.ok_or_else(|| {
            Fault::new("UTF-16 properties of this native object are not implemented.")
        })?;
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "defineProperty")?;
        self.define_stored_property(id, PropertyKey::StringUnits(units), descriptor)
    }

    fn property_descriptor(
        &mut self,
        descriptor: Ordinary,
        host: &mut dyn Host,
    ) -> Result<PropertyDescriptor, Fault> {
        let mut data = PropertyDescriptor {
            value: None,
            enumerable: None,
            writable: None,
            configurable: None,
            get: None,
            set: None,
        };
        // HasProperty and Get are separate, in the specified field order. A
        // field getter can mutate a later field or throw before it is read.
        for key in [
            "enumerable",
            "configurable",
            "value",
            "writable",
            "get",
            "set",
        ] {
            if self.property_owner(descriptor, key)?.is_none() {
                continue;
            }
            let value = self.get_ordinary_property_with_host(descriptor, key, host)?;
            match key {
                "enumerable" => data.enumerable = Some(value.truthy()),
                "configurable" => data.configurable = Some(value.truthy()),
                "value" => data.value = Some(value),
                "writable" => data.writable = Some(value.truthy()),
                "get" | "set" => {
                    let callback = match value {
                        Value::Undefined => None,
                        Value::Function(id) => Some(id),
                        _ => {
                            return Err(Fault::exception(
                                "TypeError",
                                "A property getter or setter must be callable or undefined.",
                            ))
                        }
                    };
                    if key == "get" {
                        data.get = Some(callback);
                    } else {
                        data.set = Some(callback);
                    }
                }
                _ => unreachable!(),
            }
        }
        if data.is_accessor() && data.is_data() {
            return Err(Fault::exception(
                "TypeError",
                "A property descriptor cannot combine accessors with value or writable.",
            ));
        }
        Ok(data)
    }

    pub(super) fn stored_descriptor_property(&self, id: usize, key: &PropertyKey) -> Option<&super::Property> {
        match key {
            PropertyKey::String(_) | PropertyKey::StringUnits(_) => self.objects[id].properties.get_key(key),
            PropertyKey::Symbol(key) => self.objects[id].symbol_properties.get(key),
        }
    }

    fn define_stored_property(
        &mut self,
        id: usize,
        key: PropertyKey,
        descriptor: PropertyDescriptor,
    ) -> Result<(), Fault> {
        if let PropertyKey::String(name) = &key {
            if let Some(property) = self.objects[id].properties.get(name) {
                if matches!(property.value, super::StoredValue::DeferredHostAttribute(_)) {
                    if !descriptor.is_accessor() && !descriptor.is_data() {
                        if !property.configurable && (descriptor.configurable == Some(true)
                            || descriptor.enumerable.is_some_and(|flag| flag != property.enumerable)) {
                            return Err(Fault::exception("TypeError", "Cannot redefine a nonconfigurable property."));
                        }
                        self.step()?;
                        let property = self.objects[id].properties.get_mut(name).unwrap();
                        property.enumerable = descriptor.enumerable.unwrap_or(property.enumerable);
                        property.configurable = descriptor.configurable.unwrap_or(property.configurable);
                        return Ok(());
                    }
                    if descriptor.is_accessor() {
                        let configurable = property.configurable;
                        if !configurable || descriptor.get.is_none() {
                            self.materialize_host_accessor(id, name, false)?;
                        }
                        if !configurable || descriptor.set.is_none() {
                            self.materialize_host_accessor(id, name, true)?;
                        }
                    }
                }
            }
        }
        let previous = self.stored_descriptor_property(id, &key);
        if previous.is_some_and(|property| property.getter.is_some()) {
            return Err(Fault::new(
                "Changing built-in accessor descriptors is not implemented.",
            ));
        }
        let old_attrs = previous.map(|property| {
            (
                property.enumerable,
                property.writable,
                property.configurable,
            )
        });
        let old_accessor = previous.and_then(|property| property.accessor);
        let old = old_attrs.unwrap_or((false, false, false));
        let accessor = descriptor.is_accessor() || old_accessor.is_some() && !descriptor.is_data();
        let enumerable = descriptor.enumerable.unwrap_or(old.0);
        let configurable = descriptor.configurable.unwrap_or(old.2);
        if old_attrs.is_some() && !old.2 {
            if configurable || enumerable != old.0 || accessor != old_accessor.is_some() {
                return Err(Fault::exception(
                    "TypeError",
                    "Cannot redefine a nonconfigurable property.",
                ));
            }
            if let Some(old_accessor) = old_accessor {
                if descriptor.get.is_some_and(|get| get != old_accessor.get)
                    || descriptor.set.is_some_and(|set| set != old_accessor.set)
                {
                    return Err(Fault::exception(
                        "TypeError",
                        "Cannot change a nonconfigurable accessor.",
                    ));
                }
            } else if !old.1 {
                let bytes = previous
                    .map_or(0, |property| property.value.bytes())
                    .saturating_add(descriptor.value.as_ref().map_or(0, Value::bytes));
                self.work(bytes)?;
                if descriptor.writable == Some(true) {
                    return Err(Fault::exception(
                        "TypeError",
                        "Cannot redefine a nonconfigurable property.",
                    ));
                }
                if let Some(value) = descriptor.value.as_ref() {
                    if !matches!(value, Value::Function(_))
                        && matches!(self.stored_descriptor_property(id, &key).unwrap().value, super::StoredValue::DeferredNative(_)) {
                        return Err(Fault::exception("TypeError", "Cannot redefine a nonconfigurable property."));
                    }
                    if let PropertyKey::String(name) = &key { self.materialize_native_property(id, name)?; }
                    let previous = self.stored_descriptor_property(id, &key).unwrap().value.as_value()?;
                    if !same_value(previous, value) {
                        return Err(Fault::exception("TypeError", "Cannot redefine a nonconfigurable property."));
                    }
                }
            }
        }
        if accessor {
            let old = old_accessor.unwrap_or(Accessors {
                get: None,
                set: None,
            });
            let accessor = Accessors {
                get: descriptor.get.unwrap_or(old.get),
                set: descriptor.set.unwrap_or(old.set),
            };
            match key {
                PropertyKey::String(name) => {
                    self.store_accessor_property(id, name, accessor, (enumerable, configurable))
                }
                PropertyKey::StringUnits(units) => {
                    self.store_units_accessor_property(id, units, accessor, (enumerable, configurable))
                }
                PropertyKey::Symbol(key) => self.store_symbol_accessor_property(
                    id,
                    key,
                    accessor,
                    (enumerable, configurable),
                ),
            }
        } else {
            let writable = descriptor
                .writable
                .unwrap_or(old_accessor.is_none() && old.1);
            if descriptor.value.is_none() && old_accessor.is_none()
                && self.stored_descriptor_property(id, &key).is_some_and(|property| matches!(property.value, super::StoredValue::DeferredNative(_))) {
                self.step()?;
                let property = match &key {
                    PropertyKey::String(_) | PropertyKey::StringUnits(_) => self.objects[id].properties.get_key_mut(&key).unwrap(),
                    PropertyKey::Symbol(key) => self.objects[id].symbol_properties.get_mut(key).unwrap(),
                };
                property.enumerable = enumerable;
                property.writable = writable;
                property.configurable = configurable;
                return Ok(());
            }
            let value = if let Some(value) = descriptor.value {
                value
            } else if old_attrs.is_some() && old_accessor.is_none() {
                // Descriptor inspection retains a data value without calling
                // user code; a conversion from accessor starts with undefined.
                match &key {
                    PropertyKey::String(name) => self.own_property(id, name)?,
                    PropertyKey::StringUnits(units) => self.own_units_property(id, units)?,
                    PropertyKey::Symbol(symbol) => {
                        let bytes = self.objects[id].symbol_properties[symbol].value.bytes();
                        self.work(bytes)?;
                        self.objects[id].symbol_properties[symbol].value.as_value()?.clone()
                    }
                }
            } else {
                Value::Undefined
            };
            match key {
                PropertyKey::String(name) => {
                    self.store_property(id, name, value, Some((enumerable, writable, configurable)))
                }
                PropertyKey::StringUnits(units) => {
                    self.store_units_property(id, units, value, Some((enumerable, writable, configurable)))
                }
                PropertyKey::Symbol(key) => self.store_symbol_property(
                    id,
                    key,
                    value,
                    Some((enumerable, writable, configurable)),
                ),
            }
        }
    }

    pub(super) fn create_data_property(
        &mut self,
        target: &Value,
        key: PropertyKey,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        // Creation defines an own data property; it must not call an inherited
        // setter. Reuse descriptor checks for arrays, typed views and functions.
        let descriptor = PropertyDescriptor {
            value: Some(value),
            enumerable: Some(true),
            writable: Some(true),
            configurable: Some(true),
            get: None,
            set: None,
        };
        match key {
            PropertyKey::String(name) => {
                let object = self.ordinary_required(target)?;
                self.define_data_property(object, name, descriptor, host)
            }
            PropertyKey::StringUnits(units) => {
                self.ordinary_required(target)?;
                self.define_units_data_property(target, units, descriptor, host)
            }
            PropertyKey::Symbol(key) => {
                self.ordinary_required(target)?;
                self.define_symbol_data_property(target, key, descriptor, host)
            }
        }
    }

    fn define_data_property(
        &mut self,
        object: Ordinary,
        name: String,
        descriptor: PropertyDescriptor,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.property_name(&name)?;
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, "defineProperty")?;
        if let Some(index) = self.string_object_index(id, &name) {
            let mut compatible = !descriptor.is_accessor()
                && descriptor.enumerable != Some(false)
                && descriptor.writable != Some(true)
                && descriptor.configurable != Some(true);
            if let Some(value) = descriptor.value.as_ref() {
                compatible &= self.string_object_index_matches(id, index, value)?;
            }
            return if compatible {
                Ok(())
            } else {
                Err(Fault::exception(
                    "TypeError",
                    "Cannot redefine a String index property.",
                ))
            };
        }
        if name == "length" && self.objects[id].length.is_some() {
            // A value is converted before fixed-attribute checks, and each
            // conversion may change length or make it read-only.
            let length = descriptor
                .value
                .as_ref()
                .map(|value| self.array_length_value(value, host))
                .transpose()?;
            if !self.apply_array_length(
                id,
                length,
                descriptor.writable,
                !descriptor.is_accessor()
                    && descriptor.enumerable != Some(true)
                    && descriptor.configurable != Some(true),
            )? {
                return Err(Fault::exception(
                    "TypeError",
                    "Cannot redefine array length.",
                ));
            }
            return Ok(());
        }
        if let Some(index) = self.typed_numeric_key(id, &name)? {
            if !self.typed_index_valid(id, index)
                || descriptor.is_accessor()
                || descriptor.enumerable == Some(false)
                || descriptor.writable == Some(false)
                || descriptor.configurable == Some(false)
            {
                return Err(Fault::exception(
                    "TypeError",
                    "Invalid typed-array index descriptor.",
                ));
            }
            if let Some(value) = descriptor.value {
                let number = self.binary_number(&value, host)?;
                self.typed_write(self.typed_view(id).unwrap(), index.unwrap(), number)?;
            }
            return Ok(());
        }
        if let Some(property) = self.objects[id].properties.get(&name) {
            if matches!(property.getter, Some(super::NativeGetter::ArgumentsCallee)) {
                let callback = match property.value {
                    super::StoredValue::Value(Value::Function(callback)) => Some(callback),
                    _ => None,
                };
                return if descriptor.is_data()
                    || descriptor.get.is_some_and(|get| get != callback)
                    || descriptor.set.is_some_and(|set| set != callback)
                    || descriptor.enumerable == Some(true)
                    || descriptor.configurable == Some(true)
                {
                    Err(Fault::exception(
                        "TypeError",
                        "Cannot redefine the strict arguments callee accessor.",
                    ))
                } else {
                    Ok(())
                };
            }
        }
        if self.objects[id].regexp.is_some()
            && self.owns_property(id, &name)?
            && !self.objects[id].properties.contains_key(&name)
        {
            return Err(Fault::new(
                "RegExp accessor descriptor changes are not implemented.",
            ));
        }
        self.define_stored_property(id, PropertyKey::String(name), descriptor)
    }

    fn target_descriptor(&mut self, target: DescriptorTarget, name: &str) -> Result<Value, Fault> {
        if let DescriptorTarget::Ordinary(object) = target {
            return self.own_descriptor(object, name);
        }
        let value = self.binding_value(self.global_environment, name)?;
        let attrs = self
            .global_attributes
            .get(name)
            .copied()
            .unwrap_or((true, true, false));
        let result = self.new_object(None)?;
        for (key, value) in [
            ("value", value),
            ("enumerable", Value::Bool(attrs.0)),
            ("writable", Value::Bool(attrs.1)),
            ("configurable", Value::Bool(attrs.2)),
        ] {
            self.set_property(result, key.into(), value)?;
        }
        Ok(Value::Object(result))
    }

    fn define_target_property(
        &mut self,
        target: DescriptorTarget,
        name: String,
        descriptor: PropertyDescriptor,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if let DescriptorTarget::Ordinary(object) = target {
            return self.define_data_property(object, name, descriptor, host);
        }
        if descriptor.is_accessor() {
            return Err(Fault::exception(
                "TypeError",
                "Declared global bindings cannot be replaced by accessor properties.",
            ));
        }
        let old = self
            .global_attributes
            .get(&name)
            .copied()
            .unwrap_or((true, true, false));
        let attrs = (
            descriptor.enumerable.unwrap_or(old.0),
            descriptor.writable.unwrap_or(old.1),
            descriptor.configurable.unwrap_or(old.2),
        );
        let previous = self.binding_value(self.global_environment, &name)?;
        let value = descriptor.value.unwrap_or_else(|| previous.clone());
        if !old.2
            && (attrs.2
                || attrs.0 != old.0
                || !old.1 && (attrs.1 || !same_value(&previous, &value)))
        {
            return Err(Fault::exception(
                "TypeError",
                "Cannot redefine a nonconfigurable global property.",
            ));
        }
        let kind = self.environment(self.global_environment).bindings[&name].kind;
        self.put_binding(self.global_environment, name.clone(), Some(value), kind, true)?;
        self.global_attributes.insert(name, attrs);
        Ok(())
    }

    pub(super) fn object_integrity(&mut self, method: &str, value: &Value) -> Result<Value, Fault> {
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "Native host object integrity cannot be inspected or changed.",
            ));
        }
        let Some(object) = Ordinary::from_value(value) else {
            return Ok(match method {
                "isExtensible" => Value::Bool(false),
                "isSealed" | "isFrozen" => Value::Bool(true),
                _ => self.copy_value(value)?,
            });
        };
        let id = self.ordinary_id(object)?;
        self.reject_unsupported_proxy_operation(id, method)?;
        self.work(
            self.objects[id]
                .properties
                .len()
                .saturating_add(self.objects[id].symbol_properties.len())
                .saturating_mul(std::mem::size_of::<super::Property>()),
        )?;
        if method == "isExtensible" {
            return Ok(Value::Bool(self.objects[id].extensible));
        }
        if matches!(method, "isSealed" | "isFrozen") {
            let object = &self.objects[id];
            let frozen = method == "isFrozen";
            let has_elements = self.typed_view(id).is_some_and(|view| view.length != 0);
            let result = !object.extensible
                && object
                    .properties
                    .values()
                    .chain(object.symbol_properties.values())
                    .all(|property| {
                        !property.configurable
                            && (!frozen
                                || property.getter.is_some()
                                || property.accessor.is_some()
                                || !property.writable)
                    })
                && (!frozen
                    || (!has_elements && (object.length.is_none() || !object.length_writable)));
            return Ok(Value::Bool(result));
        }
        if self.objects[id].window_named_properties {
            return Err(Fault::exception("TypeError", "WindowProperties must remain extensible."));
        }
        self.objects[id].extensible = false;
        if method == "preventExtensions" {
            return self.copy_value(value);
        }
        if self.typed_view(id).is_some_and(|view| view.length != 0) {
            return Err(Fault::exception(
                "TypeError",
                "Cannot seal or freeze a typed array with elements.",
            ));
        }
        let frozen = method == "freeze";
        if frozen {
            self.disconnect_arguments(id)?;
        }
        let object = &mut self.objects[id];
        for property in object
            .properties
            .values_mut()
            .chain(object.symbol_properties.values_mut())
        {
            property.configurable = false;
            if frozen && property.getter.is_none() && property.accessor.is_none() {
                property.writable = false;
            }
        }
        if frozen && object.length.is_some() {
            object.length_writable = false;
        }
        self.copy_value(value)
    }

    fn descriptor_target(
        &mut self,
        value: &Value,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<DescriptorTarget, Fault> {
        self.reject_foreign_window(value, host)?;
        if let Value::Host(handle) = value {
            if self.is_browser_global(*handle) { self.ensure_global_intrinsics(name)?; }
            if let Some(object) = self.host_ordinary(*handle, name, host)? {
                return Ok(DescriptorTarget::Ordinary(object));
            }
            if self.is_browser_global(*handle) {
                if self.host_owns_global(name, host) || self.host_globals.contains(name) {
                    return Err(Fault::new("Descriptors of native or declared global properties cannot be changed by this subset."));
                }
                if self.global_binding(name) {
                    return Ok(DescriptorTarget::GlobalBinding);
                }
                let (_, properties) = self.browser_global.unwrap();
                return Ok(DescriptorTarget::Ordinary(Ordinary::Object(properties)));
            }
        }
        if let Value::Host(handle)=value{
            let interface=host.instance_ancestry(*handle).and_then(|entry|entry.constructor.map(|(_,name)|name)).unwrap_or("unclassified");
            let excerpt:String=name.chars().take(80).collect();
            return Err(Fault::new(format!("Native host object prototypes and descriptors are not implemented. Receiver {handle:#010x}, interface {interface}, property {excerpt:?}.")));
        }
        self.ordinary_required(value)
            .map(DescriptorTarget::Ordinary)
    }

    pub(super) fn object_static(
        &mut self,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let first = args.first().unwrap_or(&Value::Undefined);
        let boxed;
        let first = if first.is_string()
            && matches!(method, "keys" | "getOwnPropertyDescriptor" | "assign")
        {
            let value = self.copy_value(first)?;
            boxed = self.box_primitive(value)?;
            &boxed
        } else {
            first
        };
        match method {
            "is" => {
                let second = args.get(1).unwrap_or(&Value::Undefined);
                self.work(first.bytes().saturating_add(second.bytes()))?;
                Ok(Value::Bool(same_value(first, second)))
            }
            "preventExtensions" | "seal" | "freeze" | "isExtensible" | "isSealed" | "isFrozen" => {
                self.object_integrity(method, first)
            }
            "keys" => self.object_keys(first, host),
            "getOwnPropertyNames" | "getOwnPropertyDescriptors" | "values" | "entries" => {
                self.object_inspection(method, first, host)
            }
            "hasOwn" => {
                // This static method checks ToObject before converting its key.
                Self::require_property_receiver(first)?;
                let receiver = self.copy_value(first)?;
                self.object_method(
                    "hasOwnProperty",
                    receiver,
                    args.get(1..).unwrap_or(&[]),
                    host,
                )
            }
            "getOwnPropertySymbols" => {
                Self::require_property_receiver(first)?;
                let keys = if let Some(object) = self.symbol_backing(first, host)? {
                    let id = self.ordinary_id(object)?;
                    self.own_symbol_keys(id, false)?
                } else {
                    Vec::new()
                };
                let array = self.new_object(Some(keys.len()))?;
                for (index, key) in keys.into_iter().enumerate() {
                    self.set_property(array, index.to_string(), Value::Symbol(key))?;
                }
                Ok(Value::Object(array))
            }
            "isArray" => Ok(Value::Bool(
                matches!(first, Value::Object(id) if self.object(*id)?.length.is_some()),
            )),
            "getPrototypeOf" => {
                let prototype = match first {
                    Value::Host(handle)=>Some(self.native_instance_prototype(*handle,host)?),
                    Value::String(_) | Value::StringUnits(_) => self.string_prototype.map(Ordinary::Object),
                    Value::Number(_) => self.number_prototype.map(Ordinary::Object),
                    Value::BigInt(_) => self.big_int_prototype.map(Ordinary::Object),
                    Value::Bool(_) => self.boolean_prototype.map(Ordinary::Object),
                    Value::Symbol(_) => self.symbol_prototype.map(Ordinary::Object),
                    _ => {
                        let object = self.ordinary_required(first)?;
                        self.prototype_with_host(object, &Value::Undefined, host)?
                    }
                };
                Ok(prototype.map_or(Value::Null, Ordinary::value))
            }
            "setPrototypeOf" => {
                if matches!(first, Value::Null | Value::Undefined) {
                    return Err(Fault::exception(
                        "TypeError",
                        "Cannot set a prototype of null or undefined.",
                    ));
                }
                let prototype =
                    self.prototype_argument(args.get(1).unwrap_or(&Value::Undefined))?;
                if let Some(object) = Ordinary::from_value(first) {
                    self.set_prototype(object, prototype)?;
                } else if let Value::Host(handle) = first {
                    self.set_native_wrapper_prototype(*handle, prototype, host)?;
                }
                self.copy_value(first)
            }
            "create" => {
                let prototype = self.prototype_argument(first)?;
                let id = self.new_object(None)?;
                self.objects[id].prototype = prototype;
                if let Some(descriptors) = args
                    .get(1)
                    .filter(|value| !matches!(value, Value::Undefined))
                {
                    self.define_data_properties(&Value::Object(id), descriptors, host)?;
                }
                Ok(Value::Object(id))
            }
            "getOwnPropertyDescriptor" | "defineProperty" => {
                if method == "getOwnPropertyDescriptor"
                    && matches!(first, Value::Number(_) | Value::BigInt(_) | Value::Bool(_) | Value::Symbol(_))
                {
                    // These temporary wrappers have no own properties. Still
                    // observe key conversion, including its possible exception.
                    let key = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
                    self.property_key(key, host)?;
                    return Ok(Value::Undefined);
                }
                if !matches!(
                    first,
                    Value::Object(_) | Value::Function(_) | Value::Host(_)
                ) {
                    return Err(Fault::exception(
                        "TypeError",
                        "An ordinary object or function is required.",
                    ));
                }
                let key = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
                let key = self.property_key(key, host)?;
                if let (Value::Object(id), "getOwnPropertyDescriptor") = (first, method) {
                    if self.object(*id)?.proxy.is_some() {
                        return self.proxy_own_property(*id, &key, OwnPropertyQuery::Descriptor, host);
                    }
                }
                let key = match key {
                    PropertyKey::StringUnits(units) => {
                        let object = self.symbol_backing(first, host)?;
                        if method == "getOwnPropertyDescriptor" {
                            return if let Some(object) = object {
                                self.own_units_descriptor(object, &units)
                            } else {
                                Ok(Value::Undefined)
                            };
                        }
                        if object.is_none() {
                            return Err(Fault::new("UTF-16 properties of this native object are not implemented."));
                        }
                        // Validate the native target before author descriptor
                        // getters, then revalidate its backing at application.
                        let descriptor =
                            self.ordinary_required(args.get(2).unwrap_or(&Value::Undefined))?;
                        let descriptor = self.property_descriptor(descriptor, host)?;
                        self.define_units_data_property(first, units, descriptor, host)?;
                        return self.copy_value(first);
                    }
                    PropertyKey::Symbol(key) => {
                        if method == "getOwnPropertyDescriptor" {
                            return if let Some(object) = self.symbol_backing(first, host)? {
                                self.own_symbol_descriptor(object, key)
                            } else {
                                Ok(Value::Undefined)
                            };
                        }
                        let descriptor =
                            self.ordinary_required(args.get(2).unwrap_or(&Value::Undefined))?;
                        let descriptor = self.property_descriptor(descriptor, host)?;
                        self.define_symbol_data_property(first, key, descriptor, host)?;
                        return self.copy_value(first);
                    }
                    PropertyKey::String(key) => key,
                };
                if method == "getOwnPropertyDescriptor" {
                    if let Value::Host(handle) = first {
                        if !self.is_browser_global(*handle) {
                            return self.host_descriptor(*handle, &key, host);
                        }
                    }
                }
                if method == "defineProperty" {
                    if let Value::Host(handle) = first {
                        if host.ordinary_prototype(*handle) {
                            let descriptor =
                                self.ordinary_required(args.get(2).unwrap_or(&Value::Undefined))?;
                            let descriptor = self.property_descriptor(descriptor, host)?;
                            if descriptor.is_accessor()
                                || descriptor.enumerable == Some(false)
                                || descriptor.writable == Some(false)
                                || descriptor.configurable == Some(false)
                            {
                                return Err(Fault::exception("TypeError", "Named dataset properties must remain writable, enumerable and configurable data properties."));
                            }
                            self.host_write(
                                *handle,
                                key,
                                descriptor.value.unwrap_or(Value::Undefined),
                                host,
                            )?;
                            return self.copy_value(first);
                        }
                    }
                }
                // A supported own-Window definition converts its descriptor
                // before selecting the current binding. Conversion may throw,
                // delete that binding or create it. Keep native/foreign receiver
                // rejection ahead of callbacks, without bootstrapping an unused
                // intrinsic family before the descriptor has been converted.
                let defer_target = method == "defineProperty"
                    && matches!(first, Value::Host(handle) if self.is_browser_global(*handle)
                        && !self.host_owns_global(&key, host) && !self.host_globals.contains(&key));
                let object = if defer_target {
                    self.reject_foreign_window(first, host)?;
                    None
                } else {
                    Some(self.descriptor_target(first, &key, host)?)
                };
                if method == "getOwnPropertyDescriptor" {
                    return self.target_descriptor(object.unwrap(), &key);
                }
                let descriptor =
                    self.ordinary_required(args.get(2).unwrap_or(&Value::Undefined))?;
                let descriptor = self.property_descriptor(descriptor, host)?;
                let object = match object { Some(object) => object, None => self.descriptor_target(first, &key, host)? };
                self.define_target_property(object, key, descriptor, host)?;
                self.copy_value(first)
            }
            "defineProperties" => {
                self.define_data_properties(first, args.get(1).unwrap_or(&Value::Undefined), host)?;
                self.copy_value(first)
            }
            "assign" => {
                let boxed;
                let first = if matches!(first, Value::Number(_) | Value::BigInt(_) | Value::Bool(_) | Value::Symbol(_))
                {
                    boxed = self.box_primitive(first.clone())?;
                    &boxed
                } else {
                    first
                };
                if !matches!(first, Value::Host(_)) {
                    self.ordinary_required(first)?;
                }
                for source in args.get(1..).unwrap_or(&[]) {
                    let boxed_source;
                    let source = if source.is_string() {
                        let value = self.copy_value(source)?;
                        boxed_source = self.box_primitive(value)?;
                        &boxed_source
                    } else {
                        source
                    };
                    if matches!(
                        source,
                        Value::Null
                            | Value::Undefined
                            | Value::Number(_)
                            | Value::BigInt(_)
                            | Value::Bool(_)
                            | Value::Symbol(_)
                    ) {
                        continue;
                    }
                    let keys = if let Value::Host(handle) = source {
                        self.ensure_native_expando(*handle,host,false)?;
                        if self.is_browser_global(*handle) {
                            self.window_keys(false, false, host)?
                        } else if host.document_location_owner(*handle).is_some() {
                            self.document_own_string_keys(*handle,false,host)?
                        } else if host.indexed_iteration_owner(*handle).is_some() {
                            self.indexed_host_keys(*handle,false,false,host)?
                        } else if let Some(&id) = self.host_objects.get(handle) {
                            self.own_string_keys(id, false)?
                        } else {
                            self.host_keys(*handle, false, host)?
                        }
                    } else {
                        let ordinary = self.ordinary_required(source)?;
                        let id = self.ordinary_id(ordinary)?;
                        self.own_string_keys(id, false)?
                    };
                    // Snapshot symbol keys before the first write too. A host
                    // setter can mutate a source; newly added keys are not part
                    // of this source's copy operation.
                    let symbols = if let Some(object) = self.symbol_backing(source, host)? {
                        let id = self.ordinary_id(object)?;
                        self.own_symbol_keys(id, false)?
                    } else {
                        Vec::new()
                    };
                    if keys.len().saturating_add(symbols.len()) > self.limits.heap.object_properties {
                        return Err(Fault::new(
                            format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
                        ));
                    }
                    for key in keys {
                        let enumerable = match &key {
                            StringKey::Scalar(name) => {
                                if let Value::Host(handle) = source {
                                    self.host_owned(*handle, name, true, host)?
                                } else {
                                    let object = self.ordinary_required(source)?;
                                    let id = self.ordinary_id(object)?;
                                    self.own_enumerable(id, name)?
                                }
                            }
                            StringKey::Units(units) => {
                                if let Some(object) = self.symbol_backing(source, host)? {
                                    let id = self.ordinary_id(object)?;
                                    self.own_units_enumerable(id, units)?
                                } else {
                                    false
                                }
                            }
                        };
                        if !enumerable {
                            continue;
                        }
                        let value = self.read_reference(
                            &key.clone().into_property_key().reference(source.clone()),
                            host,
                        )?;
                        // Object.assign uses Set with Throw=true in every
                        // caller, including a non-strict script.
                        let previous_strict = self.strict;
                        self.strict = true;
                        let result = self.write_reference(
                            key.into_property_key().reference(first.clone()),
                            value,
                            host,
                        );
                        self.strict = previous_strict;
                        result?;
                    }
                    for key in symbols {
                        let Some(object) = self.symbol_backing(source, host)? else {
                            continue;
                        };
                        let id = self.ordinary_id(object)?;
                        if !self.objects[id]
                            .symbol_properties
                            .get(&key)
                            .is_some_and(|property| property.enumerable)
                        {
                            continue;
                        }
                        let value = self.get_symbol_property(source.clone(), key, host)?;
                        let previous_strict = self.strict;
                        self.strict = true;
                        let result = self.assign_symbol_property(first.clone(), key, value, host);
                        self.strict = previous_strict;
                        result?;
                    }
                }
                self.copy_value(first)
            }
            _ => Err(Fault::new("Unsupported Object method.")),
        }
    }

    pub(super) fn define_data_properties(
        &mut self,
        target: &Value,
        descriptors: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.reject_foreign_window(target, host)?;
        if let Value::Host(handle)=target {self.ensure_native_expando(*handle,host,true)?;}
        // Reject an invalid target even when the descriptor list is empty.
        if !matches!(target, Value::Host(handle) if self.is_browser_global(*handle) || self.host_objects.contains_key(handle))
        {
            self.ordinary_required(target)?;
        }
        // Number/Boolean/Symbol wrappers have no own descriptor keys. ToObject
        // does not inspect their prototypes or invoke conversion callbacks.
        if matches!(
            descriptors,
            Value::Number(_) | Value::BigInt(_) | Value::Bool(_) | Value::Symbol(_)
        ) {
            return Ok(());
        }
        let boxed_descriptors;
        let descriptors = if descriptors.is_string() {
            let value = self.copy_value(descriptors)?;
            boxed_descriptors = self.box_primitive(value)?;
            &boxed_descriptors
        } else {
            descriptors
        };
        let descriptors = self.ordinary_required(descriptors)?;
        let id = self.ordinary_id(descriptors)?;
        let mut entries = Vec::new();
        let mut snapshot_bytes = 0usize;
        let mut keys: Vec<_> = self
            .own_string_keys(id, false)?
            .into_iter()
            .map(StringKey::into_property_key)
            .collect();
        let symbols = self.own_symbol_keys(id, false)?;
        if keys.len().saturating_add(symbols.len()) > self.limits.heap.object_properties {
            return Err(Fault::new(
                format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
            ));
        }
        keys.extend(symbols.into_iter().map(PropertyKey::Symbol));
        for key in keys {
            let enumerable = match &key {
                PropertyKey::String(key) => self.own_enumerable(id, key)?,
                PropertyKey::StringUnits(units) => self.own_units_enumerable(id, units)?,
                PropertyKey::Symbol(key) => self.objects[id]
                    .symbol_properties
                    .get(key)
                    .is_some_and(|property| property.enumerable),
            };
            if !enumerable {
                continue;
            }
            let value = match &key {
                PropertyKey::String(key) => {
                    self.get_ordinary_property_with_host(descriptors, key, host)?
                }
                PropertyKey::StringUnits(units) => {
                    self.get_units_property_with_receiver(descriptors, units, &descriptors.value(), host)?
                }
                PropertyKey::Symbol(key) => {
                    self.get_symbol_property(descriptors.value(), *key, host)?
                }
            };
            let descriptor = self.ordinary_required(&value)?;
            // Snapshot every field before applying the first property. A
            // descriptor object may itself be the destination of an earlier
            // definition; retaining its handle would read the mutated value.
            let descriptor = self.property_descriptor(descriptor, host)?;
            snapshot_bytes =
                snapshot_bytes.saturating_add(descriptor.value.as_ref().map_or(0, Value::bytes));
            if snapshot_bytes > self.limits.heap.retained_values {
                return Err(Fault::new(format!("Descriptor snapshot limit reached ({} bytes).", self.limits.heap.retained_values)));
            }
            entries.push((key, descriptor));
        }
        for (key, descriptor) in entries {
            match key {
                PropertyKey::String(key) => {
                    let target = self.descriptor_target(target, &key, host)?;
                    self.define_target_property(target, key, descriptor, host)?;
                }
                PropertyKey::StringUnits(units) => {
                    self.define_units_data_property(target, units, descriptor, host)?;
                }
                PropertyKey::Symbol(key) => {
                    self.define_symbol_data_property(target, key, descriptor, host)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn object_method(
        &mut self,
        method: &str,
        this: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // ECMA-262 Object.prototype.__proto__: real per-realm accessors, not a
        // special assignment/key rule. Native inspection/mutation continues to
        // use its existing provenance and origin gates before any author hook.
        if method == "\0get __proto__" {
            Self::require_property_receiver(&this)?;
            self.reject_foreign_window(&this, host)?;
            let prototype = if let Value::Host(handle) = this {
                Some(self.native_instance_prototype(handle, host)?)
            } else {
                let object = if let Some(object) = Ordinary::from_value(&this) { object }
                    else { Ordinary::from_value(&self.box_primitive(this)?).unwrap() };
                self.prototype_with_host(object, &Value::Undefined, host)?
            };
            return Ok(prototype.map_or(Value::Null, Ordinary::value));
        }
        if method == "\0set __proto__" {
            Self::require_property_receiver(&this)?;
            self.reject_foreign_window(&this, host)?;
            let prototype = args.first().unwrap_or(&Value::Undefined);
            if !matches!(prototype, Value::Null | Value::Object(_) | Value::Function(_) | Value::Host(_)) {
                return Ok(Value::Undefined);
            }
            if let Value::Host(handle) = this {
                self.reject_foreign_window(prototype, host)?;
                let prototype = self.prototype_argument(prototype)?;
                self.set_native_wrapper_prototype(handle, prototype, host)?;
            } else if let Some(object) = Ordinary::from_value(&this) {
                self.reject_foreign_window(prototype, host)?;
                let prototype = self.prototype_argument(prototype)?;
                self.set_prototype(object, prototype)?;
            }
            return Ok(Value::Undefined);
        }
        if method == "toString" {
            let tagged_host = match &this {
                Value::Host(handle) if host.exposed_native_prototype(*handle) => true,
                Value::Host(handle) if self.is_browser_global(*handle) => {
                    self.bound_window_tag_backing(*handle, host)?.is_some()
                }
                _ => false,
            };
            if Ordinary::from_value(&this).is_some() || tagged_host {
                if let Value::Host(handle)=this {self.validate_native_access(handle,host)?;}
                if let Some(key) = self.symbol_to_string_tag {
                    let receiver = self.copy_value(&this)?;
                    if let Value::String(tag) = self.get_symbol_property(receiver, key, host)? {
                        self.work(tag.len())?;
                        return self.checked_value(Value::String(format!("[object {tag}]")));
                    }
                }
            }
            let name = match &this {
                Value::Undefined => "Undefined",
                Value::Null => "Null",
                Value::Bool(_) => "Boolean",
                Value::Number(_) => "Number",
                Value::BigInt(_) => "BigInt",
                Value::String(_) | Value::StringUnits(_) => "String",
                Value::Symbol(_) => "Symbol",
                Value::Function(_) => "Function",
                Value::Object(id) => {
                    let object = self.object(*id)?;
                    if let Some(primitive) = &object.primitive {
                        primitive.tag()
                    } else if let Some(binary) = &object.binary {
                        binary.tag()
                    } else if let Some(collection) = &object.collection {
                        collection.tag()
                    } else if let Some(iterator) = &object.iterator {
                        iterator.tag()
                    } else if let Some(text) = &object.text {
                        text.tag()
                    } else if let Some(url) = &object.url {
                        url.tag()
                    } else if object.date.is_some() {
                        "Date"
                    } else if object.promise.is_some() {
                        "Promise"
                    } else if object.arguments.is_some() {
                        "Arguments"
                    } else if object.length.is_some() {
                        "Array"
                    } else if object.regexp.is_some() {
                        "RegExp"
                    } else if object.error_name.is_some() {
                        "Error"
                    } else if Some(*id) == self.string_prototype {
                        "String"
                    } else if Some(*id) == self.number_prototype {
                        "Number"
                    } else if Some(*id) == self.boolean_prototype {
                        "Boolean"
                    } else if Some(*id) == self.symbol_prototype {
                        "Symbol"
                    } else {
                        "Object"
                    }
                }
                Value::Host(_) if tagged_host => "Object",
                Value::Host(_) => {
                    return Err(Fault::new("Native host object tags are not implemented."))
                }
            };
            return self.checked_value(Value::String(format!("[object {name}]")));
        }
        if method == "isPrototypeOf" {
            if let Some(Value::Host(handle))=args.first() {
                let anchor=self.native_instance_visible_anchor(*handle,host)?;
                if matches!(this,Value::Null|Value::Undefined) {return Err(Fault::exception("TypeError","Cannot convert null or undefined to an object."));}
                let Some(prototype)=Ordinary::from_value(&this) else {return Ok(Value::Bool(false));};
                return Ok(Value::Bool(anchor==prototype || self.inherits_from(anchor,prototype,host)?));
            }
            let Some(value) = args.first().and_then(Ordinary::from_value) else {
                if matches!(args.first(), Some(Value::Host(_))) {
                    return Err(Fault::new(
                        "Native host object prototypes are not implemented.",
                    ));
                }
                return Ok(Value::Bool(false));
            };
            if matches!(this, Value::Null | Value::Undefined) {
                return Err(Fault::exception(
                    "TypeError",
                    "Cannot convert null or undefined to an object.",
                ));
            }
            let Some(prototype) = Ordinary::from_value(&this) else {
                return Ok(Value::Bool(false));
            };
            return self.inherits_from(value, prototype, host).map(Value::Bool);
        }
        if method == "valueOf" {
            Self::require_property_receiver(&this)?;
            if matches!(this, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
                return Ok(this);
            }
            return self.box_primitive(this);
        }
        // Both own-property methods convert their argument before ToObject on
        // the receiver, including when that receiver is null or undefined.
        let key = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        let key = self.property_key(key, host)?;
        Self::require_property_receiver(&this)?;
        if let Value::Object(id) = &this {
            if self.object(*id)?.proxy.is_some() {
                let query = if method == "propertyIsEnumerable" {
                    OwnPropertyQuery::Enumerable
                } else {
                    OwnPropertyQuery::Present
                };
                return self.proxy_own_property(*id, &key, query, host);
            }
        }
        let key = match key {
            PropertyKey::StringUnits(units) => {
                let owned = if let Some(object) = self.symbol_backing(&this, host)? {
                    let id = self.ordinary_id(object)?;
                    if method == "propertyIsEnumerable" {
                        self.own_units_enumerable(id, &units)?
                    } else {
                        self.owns_units_property(id, &units)?
                    }
                } else {
                    false
                };
                return Ok(Value::Bool(owned));
            }
            PropertyKey::Symbol(key) => {
                let owned = if let Some(object) = self.symbol_backing(&this, host)? {
                    let id = self.ordinary_id(object)?;
                    self.objects[id]
                        .symbol_properties
                        .get(&key)
                        .is_some_and(|property| {
                            method != "propertyIsEnumerable" || property.enumerable
                        })
                } else {
                    false
                };
                return Ok(Value::Bool(owned));
            }
            PropertyKey::String(key) => key,
        };
        let owned = if let Some(value) = Ordinary::from_value(&this) {
            let id = self.ordinary_id(value)?;
            if method == "propertyIsEnumerable" {
                self.own_enumerable(id, &key)?
            } else {
                self.owns_property(id, &key)?
            }
        } else if this.is_string() {
            self.work(this.bytes())?;
            let index = super::property_index(&key)
                .is_some_and(|index| this.string_length().is_some_and(|length| index < length));
            index || method != "propertyIsEnumerable" && key == "length"
        } else if let Value::Host(handle) = this {
            self.host_owned(handle, &key, method == "propertyIsEnumerable", host)?
        } else {
            false
        };
        Ok(Value::Bool(owned))
    }

    pub(super) fn prototype_builtin(
        &mut self,
        builtin: Builtin,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        self.call_depth += 1;
        let audited = matches!(builtin, Builtin::FunctionMethod("call" | "apply"));
        if audited { self.transient_roots.owned_call_depth += 1; }
        let result = self.prototype_builtin_inner(builtin, args, this, host);
        self.call_depth -= 1;
        if audited { self.transient_roots.owned_call_depth -= 1; }
        result
    }

    pub(super) fn prototype_builtin_inner(
        &mut self,
        builtin: Builtin,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match builtin {
            Builtin::ObjectStatic(method) => self.object_static(method, args, host),
            Builtin::ObjectMethod(method) => self.object_method(method, this, args, host),
            Builtin::FunctionMethod(method) => {
                let Value::Function(index) = this else {
                    return Err(Fault::exception(
                        "TypeError",
                        "Function method requires a callable receiver.",
                    ));
                };
                if method == "bind" {
                    return self.bind_function(index, args, host);
                }
                if method == "toString" {
                    return self.function_source(index);
                }
                let receiver = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                if method == "apply" {
                    let arguments =
                        self.apply_arguments(args.get(1).unwrap_or(&Value::Undefined), host)?;
                    self.call_value(Value::Function(index), &arguments, receiver, host)
                } else {
                    self.call_value(
                        Value::Function(index),
                        args.get(1..).unwrap_or(&[]),
                        receiver,
                        host,
                    )
                }
            }
            Builtin::ArrayMethod(method) => {
                if method=="slice" {
                    if matches!(this, Value::Null | Value::Undefined) {
                        return Err(Fault::exception("TypeError", "Array.prototype.slice requires a non-null receiver."));
                    }
                    if let Value::Host(handle) = this { return self.array_slice_host(handle,args,host); }
                }
                if matches!(method, "flat" | "flatMap") {
                    return self.array_flat(this, method == "flatMap", args, host);
                }
                let Value::Object(id) = this else {
                    return Err(Fault::new(
                        "Borrowed array methods require an ordinary object in this subset.",
                    ));
                };
                if method == "toString" {
                    let join = self.get_property_with_host(id, "join", host)?;
                    if matches!(join, Value::Function(_)) {
                        self.call_value(join, &[], Value::Object(id), host)
                    } else {
                        self.object_method("toString", Value::Object(id), &[], host)
                    }
                } else {
                    self.array_method(id, method, args, host)
                }
            }
            Builtin::RegExpMethod(method) => {
                let Value::Object(id) = this else {
                    return Err(Fault::exception(
                        "TypeError",
                        "RegExp method requires a RegExp receiver.",
                    ));
                };
                if self.object(id)?.regexp.is_none() {
                    return Err(Fault::exception(
                        "TypeError",
                        "RegExp method requires a RegExp receiver.",
                    ));
                }
                self.regexp_method(id, method, args, host)
            }
            Builtin::ErrorToString => {
                let object = self.ordinary_required(&this)?;
                let id = self.ordinary_id(object)?;
                self.error_text(id, host)
            }
            Builtin::StringFromCharCode => {
                // call_value bounds the argument count. Decode only after all
                // conversions so adjacent UTF-16 units can form a surrogate pair.
                let mut units = Vec::with_capacity(args.len());
                for value in args {
                    let number = self.binary_number(value, host)?;
                    let unit = if number.is_finite() {
                        number.trunc().rem_euclid(65536.0) as u16
                    } else {
                        0
                    };
                    units.push(unit);
                }
                self.units_string(&units).map(Value::String)
            }
            Builtin::StringMethod(method) => {
                if matches!(method,"replace"|"replaceAll") {
                    return self.string_replace_value(this,args,method=="replaceAll",host);
                }
                let this = if matches!(method, "toString" | "valueOf") {
                    match self.wrapped_data(&this) {
                        Some(super::primitive_wrappers::Data::String { .. }) => {
                            self.wrapped_primitive(&this)?.unwrap()
                        }
                        _ => this,
                    }
                } else {
                    this
                };
                let receiver = match this {
                    Value::String(_) | Value::StringUnits(_) => this,
                    Value::Null | Value::Undefined => {
                        return Err(Fault::exception(
                            "TypeError",
                            "String method receiver cannot be null or undefined.",
                        ))
                    }
                    value if matches!(method, "toString" | "valueOf") => {
                        let _ = value;
                        return Err(Fault::exception(
                            "TypeError",
                            "String value method requires a string receiver.",
                        ));
                    }
                    value => {
                        let value = self.primitive_with_hint(value, true, host)?;
                        self.work(value.bytes())?;
                        if value.is_string() { value } else { Value::String(value.script_text()?) }
                    }
                };
                self.call_member(receiver, method, args, host)
            }
            Builtin::BigIntMethod(to_string) => self.big_int_method(to_string, this, args, host),
            Builtin::PrimitiveMethod(method, number) => {
                self.primitive_value_method(method, number, this, args, host)
            }
            _ => Err(Fault::new("Invalid prototype builtin.")),
        }
    }
}

pub(super) fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Symbol(a), Value::Symbol(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => {
            a.is_nan() && b.is_nan() || a.to_bits() == b.to_bits()
        }
        _ => strict_equal(left, right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_descriptor_flag_commit_checks_the_remaining_step_budget() {
        let mut runtime = Runtime::new();
        let owner = runtime.array_prototype.unwrap();
        let functions = runtime.functions.len();
        runtime.begin_job();
        runtime.steps = 0;
        let descriptor = PropertyDescriptor { value: None, enumerable: None, writable: Some(false), configurable: None, get: None, set: None };
        let error = runtime.define_stored_property(owner, PropertyKey::String("map".into()), descriptor).unwrap_err();
        assert!(error.message.contains("limit"), "{}", error.message);
        assert!(runtime.objects[owner].properties["map"].writable);
        assert!(matches!(runtime.objects[owner].properties["map"].value, super::super::StoredValue::DeferredNative(_)));
        assert_eq!(runtime.functions.len(), functions);
        runtime.begin_job();
        runtime.steps = 1;
        let descriptor = PropertyDescriptor { value: None, enumerable: None, writable: Some(false), configurable: None, get: None, set: None };
        runtime.define_stored_property(owner, PropertyKey::String("map".into()), descriptor).unwrap();
        assert!(!runtime.objects[owner].properties["map"].writable);
        assert_eq!(runtime.steps, 0);
        assert_eq!(runtime.functions.len(), functions);
    }

    struct NoHost;

    impl Host for NoHost {
        fn owns_property(&self, object: u32, property: &str) -> bool {
            object != 11 || property == "reserved"
        }

        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("Unexpected native property read.".into())
        }

        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native property write.".into())
        }

        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native method call.".into())
        }
    }

    fn succeeds(source: &str) {
        let mut runtime = Runtime::for_test();
        assert_eq!(
            runtime.execute(source, &mut NoHost).unwrap().to_text(),
            "ok"
        );
    }

    #[test]
    fn primitive_receiver_lookup_preserves_reference_and_accessor_semantics() {
        succeeds(include_str!(
            "../tools/fixtures/primitive-receiver-lookup.js"
        ));
    }

    #[test]
    fn primitive_receiver_data_reads_skip_copies_but_charge_returned_values() {
        let mut runtime = Runtime::for_test();
        runtime
            .execute(
                "Object.defineProperty(String.prototype,'setterOnly',{set:function(){throw 'setter';}});",
                &mut NoHost,
            )
            .unwrap();
        let receiver = Value::String("A".repeat(8192));
        for name in ["charCodeAt", "missing", "setterOnly"] {
            runtime.begin_job();
            runtime.copy_bytes = 128;
            let value = runtime.read_property(&receiver, name, &mut NoHost).unwrap();
            assert_eq!(matches!(value, Value::Function(_)), name == "charCodeAt");
            assert!(runtime.copy_bytes > 0);
        }

        runtime.begin_job();
        runtime
            .set_property(
                runtime.string_prototype.unwrap(),
                "data".into(),
                Value::String("B".repeat(8192)),
            )
            .unwrap();
        runtime.copy_bytes = 128;
        let error = runtime
            .read_property(&receiver, "data", &mut NoHost)
            .unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.display().contains("String/copy work limit"));
        assert_eq!(runtime.copy_bytes, 0);
    }

    #[test]
    fn primitive_receiver_getter_copy_is_charged_before_effects_and_recovers() {
        let mut runtime = Runtime::for_test();
        runtime
            .execute(
                "var getterRuns=0;Object.defineProperty(String.prototype,'read',{get:function(){'use strict';getterRuns++;return 7;}});",
                &mut NoHost,
            )
            .unwrap();
        let receiver = Value::String("A".repeat(8192));
        runtime.begin_job();
        runtime.copy_bytes = 128;
        let error = runtime
            .read_property(&receiver, "read", &mut NoHost)
            .unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.display().contains("String/copy work limit"));
        assert_eq!(runtime.copy_bytes, 0);
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("getterRuns;", &mut NoHost)
                .unwrap()
                .to_text(),
            "0"
        );
        assert_eq!(
            runtime
                .read_property(&receiver, "read", &mut NoHost)
                .unwrap()
                .to_text(),
            "7"
        );
        assert_eq!(
            runtime
                .execute("getterRuns;", &mut NoHost)
                .unwrap()
                .to_text(),
            "1"
        );
    }

    #[test]
    fn primitive_receiver_reads_remove_only_the_redundant_payload_work() {
        for (source, copies) in [
            ("payload;", 1),
            ("payload.length;", 1),
            ("typeof payload.charCodeAt;", 1),
            // Character reads now scan only the requested prefix; at zero,
            // only the owned binding read scales with the payload length.
            ("payload.charCodeAt(0);", 1),
            ("payload?.charCodeAt?.(0);", 1),
            ("(payload?.charCodeAt)(0);", 1),
            ("payload[0];", 1),
            ("payload?.[0];", 1),
        ] {
            let mut measurements = Vec::new();
            for length in [1024, 8192] {
                let mut runtime = Runtime::for_test();
                runtime
                    .define_global("payload", Value::String("A".repeat(length)))
                    .unwrap();
                runtime.begin_job();
                runtime.execute(source, &mut NoHost).unwrap();
                measurements.push(runtime.job_usage());
            }
            assert_eq!(
                measurements[1].1 - measurements[0].1,
                copies * (8192 - 1024),
                "{source}"
            );
            assert_eq!(measurements[1].0, measurements[0].0, "{source}");
        }
    }

    #[test]
    fn primitive_receiver_scans_keep_shared_job_limit_and_one_mib_recovery() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for microtask in [false, true] {
                    // Retain the old 1536-read input as an improved success;
                    // a larger witness still exercises the same fatal guard.
                    for iterations in [768, 1536, 4096] {
                        let mut runtime = Runtime::for_test();
                        runtime.define_global("payload", Value::String("A".repeat(2048))).unwrap();
                        let scan = format!("for(;completed<{iterations};completed++){{total+=payload.charCodeAt(completed%2048);}}");
                        let source = if microtask {
                            format!("var completed=0,total=0,tail=0;Promise.resolve().then(function(){{{scan}}}).then(function(){{tail=1;}});")
                        } else {
                            format!("var completed=0,total=0,tail=0;{scan}tail=1;")
                        };
                        runtime.begin_job();
                        let mut result = runtime.execute(&source, &mut NoHost).map(|_| ());
                        if microtask {
                            result.unwrap();
                            result = runtime.microtask_checkpoint(&mut NoHost).map(|messages| assert!(messages.is_empty()));
                        }
                        if iterations <= 1536 {
                            result.unwrap();
                            assert!(runtime.job_usage().1 < super::super::MAX_COPY_BYTES);
                        } else {
                            let error = result.unwrap_err();
                            assert!(error.contains("String/copy work limit"), "{error}");
                            assert_eq!(runtime.job_usage().1, super::super::MAX_COPY_BYTES);
                            assert!(!runtime.has_microtasks());
                        }
                        assert_eq!(runtime.eval_depth, 0);
                        assert_eq!(runtime.call_depth, 0);
                        runtime.begin_job();
                        let completed = runtime.execute("completed;", &mut NoHost).unwrap().to_number();
                        if iterations <= 1536 {
                            assert_eq!(completed, f64::from(iterations));
                            assert_eq!(runtime.execute("total;", &mut NoHost).unwrap().to_number(), f64::from(iterations * 65));
                            assert_eq!(runtime.execute("tail;", &mut NoHost).unwrap().to_text(), "1");
                        } else {
                            assert!(completed > 0.0 && completed < 4096.0);
                            assert_eq!(runtime.execute("tail;", &mut NoHost).unwrap().to_text(), "0");
                        }
                        assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_text(), "42");
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn object_has_own_checks_own_keys_without_reading_values() {
        let mut runtime = Runtime::for_test();
        runtime.bind_host_object(11, &[]).unwrap();
        runtime.define_global("native", Value::Host(11)).unwrap();
        assert_eq!(
            runtime
                .execute(
                    r#"
                    let reads=0, key=Symbol('own'), inherited=Symbol('inherited'), parent={inherited:1};
                    parent[inherited]=2;
                    let object=Object.create(parent);object.hasOwnProperty=0;object.empty=undefined;object[key]=3;
                    Object.defineProperty(object,'hidden',{get:function(){reads++;throw 'getter invoked';}});
                    if(!Object.hasOwn(object,'hasOwnProperty')||!Object.hasOwn(object,'empty')||!Object.hasOwn(object,'hidden')||!Object.hasOwn(object,key))throw 'own keys';
                    if(Object.hasOwn(object,'inherited')||Object.hasOwn(object,inherited)||Object.hasOwn(object,'absent')||reads)throw 'inherited or getter';
                    let bare=Object.create(null);bare.key=1;
                    if(!Object.hasOwn(bare,'key')||Object.hasOwn(bare,'hasOwnProperty'))throw 'null prototype';
                    if(!Object.hasOwn('😀',0)||!Object.hasOwn('😀',1)||!Object.hasOwn('😀','length')||Object.hasOwn('😀',2)||Object.hasOwn('😀','charAt'))throw 'string own keys';
                    if(Object.hasOwn(7,'toString')||Object.hasOwn(false,'valueOf')||Object.hasOwn(key,'description'))throw 'primitive own keys';
                    let array=Array(2);array[1]=undefined;
                    if(Object.hasOwn(array,0)||!Object.hasOwn(array,1)||!Object.hasOwn(array,'length'))throw 'array holes';
                    native.expando=4;native[key]=5;
                    if(!Object.hasOwn(native,'expando')||!Object.hasOwn(native,key)||Object.hasOwn(native,'absent'))throw 'registered native expandos';
                    'ok';
                    "#,
                    &mut NoHost,
                )
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn object_has_own_checks_null_before_key_conversion_and_has_static_metadata() {
        succeeds(
            r#"
            let f=Object.hasOwn, d=Object.getOwnPropertyDescriptor(Object,'hasOwn');
            if(f.name!=='hasOwn'||f.length!==2||d.value!==f||d.enumerable||!d.writable||!d.configurable||f.hasOwnProperty('prototype'))throw 'metadata';
            let order='', key={toString:function(){order+='k';return 'name';}}, object={name:1,undefined:2};
            function argument(value){order+='a';return value;}
            let caught=0;
            try{Object.hasOwn(null,argument(key));}catch(e){if(e.name==='TypeError')caught++;}
            try{Object.hasOwn(undefined,key);}catch(e){if(e.name==='TypeError')caught++;}
            try{Object.hasOwn();}catch(e){if(e.name==='TypeError')caught++;}
            if(caught!==3||order!=='a')throw 'null before coercion';
            if(!f.call(null,object,key)||order!=='ak'||!Object.hasOwn(object))throw 'static receiver or key default';
            let symbol=Symbol('key');object[symbol]=3;
            if(!Object.hasOwn(object,{toString:function(){return symbol;}}))throw 'symbol key conversion';
            let thrown=Symbol('throw'), seen=false;
            try{Object.hasOwn(object,{toString:function(){throw thrown;}});}catch(e){seen=e===thrown;}
            if(!seen)throw 'key exception';
            try{new f(object,'name');}catch(e){if(e.name==='TypeError')caught++;}
            if(caught!==4)throw 'constructible';
            'ok';
            "#,
        );
    }

    #[test]
    fn string_from_char_code_coerces_uint16_and_combines_surrogate_pairs() {
        succeeds(
            r#"
            if(String.fromCharCode()!=='')throw 'empty';
            let text=String.fromCharCode(NaN,Infinity,-Infinity,0,-0,null,undefined,false,true,'65.9',65.9,-65535.9,-1.9,65537,131138,4294967363,9007199254740991,1e100,-0.5,'invalid');
            let expected=[0,0,0,0,0,0,0,0,1,65,65,1,65535,1,66,67,65535,0,0,0];
            if(text.length!==expected.length)throw 'unit count';
            for(let i=0;i<expected.length;i++)if(text.charCodeAt(i)!==expected[i])throw 'ToUint16';
            text=String.fromCharCode(0x41,0x3A9,0xD7FF,0xE000,0xFFFF);
            if(text.charAt(0)!=='A'||text.charAt(1)!=='Ω'||text.charCodeAt(2)!==0xD7FF||text.charCodeAt(3)!==0xE000||text.charCodeAt(4)!==0xFFFF)throw 'BMP';
            text=String.fromCharCode(0xD83D,0xDE00,0xD834,0xDD1E,0xDBFF,0xDFFF);
            if(text.length!==6||text.codePointAt(0)!==0x1F600||text.codePointAt(2)!==0x1D11E||text.codePointAt(4)!==0x10FFFF)throw 'surrogate pairs';
            if(String.fromCharCode(0x1F600).charCodeAt(0)!==0xF600)throw 'not fromCodePoint';
            'ok';
            "#,
        );
    }

    #[test]
    fn string_from_char_code_orders_coercions_and_stops_at_abrupt_completion() {
        succeeds(
            r#"
            let order='';
            function argument(letter,value){order+=letter;return value;}
            let first={valueOf:function(){order+='v';return {};},toString:function(){order+='s';return '65.9';}};
            let second={valueOf:function(){order+='w';return 66;}};
            if(String.fromCharCode(argument('a',first),argument('b',second))!=='AB'||order!=='abvsw')throw 'conversion order';
            let key=Symbol('failure'), caught=false;
            order='';
            try{String.fromCharCode(argument('a',key),argument('b',second));}catch(e){caught=e.name==='TypeError';}
            if(!caught||order!=='ab')throw 'Symbol must stop coercion';
            let failing={valueOf:function(){order+='x';throw key;}};
            order='';caught=false;
            try{String.fromCharCode(argument('a',failing),argument('b',second));}catch(e){caught=e===key;}
            if(!caught||order!=='abx')throw 'throw identity or later conversion';
            let observed={};
            Object.defineProperty(observed,'valueOf',{get:function(){order+='g';return function(){if(this!==observed)throw 'receiver';order+='c';return 67;};}});
            order='';
            if(String.fromCharCode(observed)!=='C'||order!=='gc')throw 'conversion accessor';
            'ok';
            "#,
        );
    }

    #[test]
    fn string_from_char_code_has_standard_metadata_and_ignores_receiver() {
        succeeds(
            r#"
            let f=String.fromCharCode, d=Object.getOwnPropertyDescriptor(String,'fromCharCode');
            if(typeof f!=='function'||f.name!=='fromCharCode'||f.length!==1||d.value!==f||d.enumerable||!d.writable||!d.configurable)throw 'static descriptor';
            let name=Object.getOwnPropertyDescriptor(f,'name'), length=Object.getOwnPropertyDescriptor(f,'length');
            if(name.enumerable||name.writable||!name.configurable||length.enumerable||length.writable||!length.configurable||f.hasOwnProperty('prototype'))throw 'function descriptor';
            let receiver={valueOf:function(){throw 'receiver conversion';},toString:function(){throw 'receiver conversion';}};
            if(f.call(receiver,65,66)!=='AB'||f.call(Symbol(),67)!=='C'||f.apply(null,[68,69])!=='DE')throw 'borrowed static';
            let caught=false;
            try{new f(65);}catch(e){caught=e.name==='TypeError';}
            if(!caught)throw 'constructible';
            String.fromCharCode=function(){return 'changed';};
            if(String.fromCharCode()!=='changed'||f(70)!=='F')throw 'writable method';
            if(!delete String.fromCharCode||'fromCharCode' in String)throw 'configurable method';
            'ok';
            "#,
        );
    }

    #[test]
    fn string_from_char_code_preserves_explicit_lone_surrogate_limitation() {
        for source in [
            "String.fromCharCode(0xD800);",
            "String.fromCharCode(0xDC00);",
            "String.fromCharCode(0xD800,65);",
            "String.fromCharCode(0xDC00,0xD800);",
        ] {
            let mut runtime = Runtime::for_test();
            let error = runtime.execute(source, &mut NoHost).unwrap_err();
            assert!(error.contains("lone UTF-16 surrogate"), "{source}: {error}");
        }
        let mut runtime = Runtime::for_test();
        let error = runtime
            .execute(
                "let calls=0,done=false;try{String.fromCharCode(0xD800,{valueOf:function(){calls++;return 65;}});}catch(e){calls+=10;}done=true;",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("lone UTF-16 surrogate"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("calls+':'+done;", &mut NoHost)
                .unwrap()
                .to_text(),
            "1:false"
        );
    }

    #[test]
    fn string_from_char_code_retains_call_and_work_limits() {
        let arguments = vec!["65"; super::super::MAX_ARGUMENTS].join(",");
        let mut runtime = Runtime::for_test();
        let result = runtime
            .execute(&format!("String.fromCharCode({arguments});"), &mut NoHost)
            .unwrap();
        assert_eq!(result.to_text(), "A".repeat(super::super::MAX_ARGUMENTS));
        let error = runtime
            .execute(
                &format!("String.fromCharCode({arguments},65);"),
                &mut NoHost,
            )
            .unwrap_err();
        assert!(
            error.contains("Call argument limit reached (256)"),
            "{error}"
        );
        runtime.begin_job();
        let error = runtime
            .execute("String.fromCharCode.apply(null,Array(257));", &mut NoHost)
            .unwrap_err();
        assert!(
            error.contains("Call argument limit reached (256)"),
            "{error}"
        );

        let mut runtime = Runtime::for_test();
        let constructor = runtime.binding_value(0, "String").unwrap();
        let function = runtime
            .get_ordinary_property_with_host(
                Ordinary::from_value(&constructor).unwrap(),
                "fromCharCode",
                &mut NoHost,
            )
            .unwrap();
        runtime.copy_bytes = 1;
        let error = runtime
            .call_value(
                function,
                &[Value::Number(65.0)],
                Value::Undefined,
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.display().contains("String/copy work limit"));
        assert_eq!(runtime.copy_bytes, 0);
    }

    #[test]
    fn accessor_reads_writes_and_inspection_preserve_receivers_and_identity() {
        succeeds(
            r#"
            let reads=0, writes=0, parent={}, child=Object.create(parent), key=Symbol('accessor');
            let get=function(){reads++;return this.base;};
            let set=function(value){writes++;this.base=value;return 999;};
            Object.defineProperty(parent,'amount',{get:get,set:set,enumerable:true,configurable:true});
            Object.defineProperty(parent,key,{get:get,set:set,enumerable:true,configurable:true});
            child.base=4;
            let d=Object.getOwnPropertyDescriptor(parent,'amount');
            let sd=Object.getOwnPropertyDescriptor(parent,key);
            if(d.get!==get||d.set!==set||'value' in d||'writable' in d||sd.get!==get)throw 'descriptor';
            if(!parent.hasOwnProperty('amount')||child.hasOwnProperty('amount')||!('amount' in child)||!(key in child))throw 'presence';
            if(Object.keys(parent).join(',')!=='amount'||Object.getOwnPropertySymbols(parent)[0]!==key||reads)throw 'inspection invoked getter';
            if(child.amount!==4||child[key]!==4||reads!==2)throw 'getter receiver';
            if((child.amount=7)!==7||(child[key]=8)!==8||writes!==2||child.base!==8||parent.base!==undefined)throw 'setter receiver';
            if(child.hasOwnProperty('amount')||child.hasOwnProperty(key))throw 'inherited accessor shadowed';
            Object.defineProperty(child,'empty',{get:undefined,set:undefined});
            if(child.empty!==undefined)throw 'empty accessor';
            child.empty=5; let caught=false;
            try{(function(){'use strict';child.empty=6;})();}catch(e){caught=e.name==='TypeError';}
            if(!caught||child.empty!==undefined)throw 'missing setter';
            'ok';
            "#,
        );
    }

    #[test]
    fn accessor_descriptor_fields_follow_order_inheritance_and_early_errors() {
        succeeds(
            r#"
            let order='', descriptor={}, target={};
            function field(name,letter,value){Object.defineProperty(descriptor,name,{get:function(){order+=letter;return value;},configurable:true});}
            field('enumerable','e',true);field('configurable','c',true);field('value','v',1);
            field('writable','w',true);field('get','g',undefined);field('set','s',undefined);
            let caught=false;
            try{Object.defineProperty(target,'mixed',descriptor);}catch(e){caught=e.name==='TypeError';}
            if(!caught||order!=='ecvwgs'||'mixed' in target)throw 'field order or mixed descriptor';
            order='';field('get','g',42);caught=false;
            try{Object.defineProperty(target,'invalid',descriptor);}catch(e){caught=e.name==='TypeError';}
            if(!caught||order!=='ecvwg'||'invalid' in target)throw 'invalid getter did not stop';
            let inherited={};Object.defineProperty(inherited,'value',{get:function(){if(this===inherited)throw 'descriptor receiver';return 13;}});
            Object.defineProperty(target,'inherited',Object.create(inherited));
            if(target.inherited!==13)throw 'inherited descriptor field';
            let thrown=Symbol('throw'), failure={};
            Object.defineProperty(failure,'value',{get:function(){throw thrown;}});caught=false;
            try{Object.defineProperty(target,'failed',failure);}catch(e){caught=e===thrown;}
            if(!caught||'failed' in target)throw 'getter exception changed';
            'ok';
            "#,
        );
    }

    #[test]
    fn accessor_redefinitions_switch_kinds_without_calling_getters() {
        succeeds(
            r#"
            let reads=0, stored=0, o={}, key=Symbol();
            let get=function(){reads++;return 1;}, set=function(value){stored=value;};
            Object.defineProperty(o,key,{get:get,configurable:true});
            Object.defineProperty(o,key,{enumerable:true});
            if(Object.getOwnPropertyDescriptor(o,key).get!==get||reads)throw 'generic descriptor';
            Object.defineProperty(o,key,{value:5});
            let d=Object.getOwnPropertyDescriptor(o,key);
            if(d.value!==5||d.writable||!d.enumerable||!d.configurable||'get' in d||reads)throw 'accessor to data';
            Object.defineProperty(o,key,{set:set});d=Object.getOwnPropertyDescriptor(o,key);
            if(d.get!==undefined||d.set!==set||'value' in d||reads)throw 'data to accessor';
            Object.defineProperty(o,key,{configurable:false});
            Object.defineProperty(o,key,{set:set});Object.defineProperty(o,key,{});
            let failures=0;
            function reject(descriptor){try{Object.defineProperty(o,key,descriptor);}catch(e){if(e.name==='TypeError'){failures++;return;}}throw 'accepted invalid descriptor';}
            reject({get:get});reject({set:undefined});reject({set:function(){}});reject({value:4});
            reject({writable:false});reject({enumerable:false});reject({configurable:true});
            if(failures!==7||reads)throw 'nonconfigurable accessor';
            o[key]=9;if(stored!==9||o[key]!==undefined)throw 'retained setter';
            let plain={};Object.defineProperty(plain,'data',{value:7,configurable:true});
            Object.defineProperty(plain,'data',{get:get});
            if(plain.data!==1||reads!==1)throw 'string transition';
            'ok';
            "#,
        );
    }

    #[test]
    fn frozen_accessors_keep_their_setters_and_do_not_run_getters_during_integrity_checks() {
        succeeds(
            r#"
            let reads=0, result=0, key=Symbol(), o={};
            let get=function(){reads++;return result;}, set=function(value){result=value;};
            Object.defineProperty(o,'named',{get:get,set:set,enumerable:true,configurable:true});
            Object.defineProperty(o,key,{get:get,set:set,configurable:true});
            Object.freeze(o);
            if(!Object.isFrozen(o)||!Object.isSealed(o)||Object.isExtensible(o)||reads)throw 'integrity';
            let d=Object.getOwnPropertyDescriptor(o,'named'), sd=Object.getOwnPropertyDescriptor(o,key);
            if(d.configurable||sd.configurable||d.set!==set||sd.get!==get||reads)throw 'frozen descriptor';
            o.named=4;o[key]=8;if(result!==8||reads)throw 'frozen setter';
            if(o.named!==8||reads!==1||delete o.named||delete o[key])throw 'frozen getter';
            let child=Object.create(o);child.named=11;
            if(result!==11||child.hasOwnProperty('named'))throw 'frozen inherited setter';
            'ok';
            "#,
        );
    }

    #[test]
    fn accessor_assign_snapshots_all_keys_and_rechecks_own_enumerability() {
        succeeds(
            r#"
            let order='', key=Symbol('old'), added=Symbol('new'), parent={}, source=Object.create(parent), target={};
            Object.defineProperty(parent,'deleted',{get:function(){throw 'inherited getter ran';}});
            Object.defineProperty(source,'first',{enumerable:true,get:function(){
              order+='a';delete source.deleted;Object.defineProperty(source,'later',{enumerable:true});
              Object.defineProperty(source,key,{enumerable:true});source.added=9;source[added]=10;return 1;
            }});
            Object.defineProperty(source,'deleted',{value:2,enumerable:true,configurable:true});
            Object.defineProperty(source,'later',{get:function(){order+='b';return 3;},configurable:true});
            Object.defineProperty(source,key,{get:function(){order+='c';return 4;},configurable:true});
            Object.defineProperty(target,'first',{set:function(value){if(this!==target||value!==1)throw 'first setter';order+='A';}});
            Object.defineProperty(target,'later',{set:function(value){if(value!==3)throw 'later setter';order+='B';}});
            Object.defineProperty(target,key,{set:function(value){if(value!==4)throw 'symbol setter';order+='C';}});
            if(Object.assign(target,source)!==target||order!=='aAbBcC')throw 'copy order';
            if('deleted' in target||'added' in target||added in target)throw 'key snapshot';
            'ok';
            "#,
        );
    }

    #[test]
    fn accessor_define_properties_snapshots_converted_descriptors_before_mutation() {
        succeeds(
            r#"
            let order='', calls=0, target={}, descriptors={}, key=Symbol(), get=function(){calls++;return 7;};
            Object.defineProperty(descriptors,'first',{enumerable:true,get:function(){
              order+='a';Object.defineProperty(descriptors,'later',{enumerable:true});
              Object.defineProperty(descriptors,key,{enumerable:true});return {get:get};
            }});
            Object.defineProperty(descriptors,'later',{configurable:true,get:function(){
              order+='b';if('first' in target)throw 'early mutation';return {value:2};
            }});
            Object.defineProperty(descriptors,key,{configurable:true,get:function(){
              order+='c';if('later' in target)throw 'early symbol mutation';return {get:get};
            }});
            Object.defineProperties(target,descriptors);
            if(order!=='abc'||calls||target.later!==2)throw 'descriptor conversion';
            if(Object.getOwnPropertyDescriptor(target,'first').get!==get||Object.getOwnPropertyDescriptor(target,key).get!==get||calls)throw 'inspection';
            if(target.first!==7||target[key]!==7||calls!==2)throw 'stored accessors';
            let rejected={}, invalid={ok:{value:1}};
            Object.defineProperty(invalid,key,{enumerable:true,get:function(){return {get:3};}});
            let caught=false;try{Object.defineProperties(rejected,invalid);}catch(e){caught=e.name==='TypeError';}
            if(!caught||'ok' in rejected||Object.getOwnPropertySymbols(rejected).length)throw 'partial definition';
            'ok';
            "#,
        );
    }

    #[test]
    fn accessor_raw_reads_fail_without_invoking_script() {
        let mut runtime = Runtime::for_test();
        let Value::Object(object) = runtime.execute(
            "var reads=0;var object={};Object.defineProperty(object,'x',{get:function(){reads++;return 9;}});object;",
            &mut NoHost,
        ).unwrap() else { panic!("expected an object"); };
        assert!(runtime.get_property(object, "x").is_err());
        assert!(runtime.own_property(object, "x").is_err());
        assert_eq!(
            runtime.execute("reads;", &mut NoHost).unwrap().to_text(),
            "0"
        );
        assert_eq!(
            runtime
                .get_property_with_host(object, "x", &mut NoHost)
                .unwrap()
                .to_text(),
            "9"
        );
        assert_eq!(
            runtime.execute("reads;", &mut NoHost).unwrap().to_text(),
            "1"
        );
    }

    #[test]
    fn accessor_registered_host_expandos_keep_the_native_receiver_and_reserved_names() {
        let mut runtime = Runtime::for_test();
        runtime.bind_host_object(11, &[]).unwrap();
        runtime.define_global("native", Value::Host(11)).unwrap();
        assert_eq!(
            runtime
                .execute(
                    r#"
            var stored=0, reads=0, key=Symbol('host');
            var get=function(){if(this!==native)throw 'getter receiver';reads++;return stored;};
            var set=function(value){if(this!==native)throw 'setter receiver';stored=value;};
            var descriptors={amount:{get:get,set:set,enumerable:true}};
            descriptors[key]={get:get,set:set,enumerable:true};
            Object.defineProperties(native,descriptors);
            native.amount=4;native[key]=5;
            if(stored!==5||native.amount!==5||native[key]!==5||reads!==2)throw 'host accessor';
            var source={amount:7};source[key]=8;Object.assign(native,source);
            if(stored!==8)throw 'host assign setter';
            var copy=Object.assign({},native);
            if(copy.amount!==8||copy[key]!==8||reads!==4)throw 'host assign getter';
            'ok';
        "#,
                    &mut NoHost
                )
                .unwrap()
                .to_text(),
            "ok"
        );
        assert!(runtime
            .execute(
                "Object.defineProperty(native,'reserved',{get:function(){return 1;}});",
                &mut NoHost,
            )
            .is_err());
        let backing = runtime.host_objects[&11];
        assert!(!runtime.objects[backing].properties.contains_key("reserved"));
    }

    #[test]
    fn symbol_descriptors_enumerate_own_keys_by_insertion_order() {
        succeeds(
            r#"
            let a=Symbol('same'), b=Symbol('same'), inherited=Symbol('parent');
            let parent={}; parent[inherited]=8;
            let o=Object.create(parent); o[b]=2; o.plain=3; o[a]=1;
            Object.defineProperty(o,b,{enumerable:false});
            let keys=Object.getOwnPropertySymbols(o);
            let d=Object.getOwnPropertyDescriptor(o,b);
            if(keys.length!==2||keys[0]!==b||keys[1]!==a)throw 'order';
            if(d.value!==2||d.enumerable||!d.writable||!d.configurable)throw 'descriptor';
            if(!o.hasOwnProperty(b)||o.propertyIsEnumerable(b)||!o.propertyIsEnumerable(a))throw 'own';
            if(o.hasOwnProperty(inherited)||Object.getOwnPropertyDescriptor(o,inherited)!==undefined)throw 'inherited descriptor';
            if(o[inherited]!==8||!(inherited in o)||Object.keys(o).join(',')!=='plain')throw 'inheritance';
            let seen=''; for(let key in o)seen+=key;
            if(seen!=='plain')throw 'for-in';
            delete o[b]; o[b]=4; keys=Object.getOwnPropertySymbols(o);
            if(keys[0]!==a||keys[1]!==b)throw 'reinsertion';
            function F(){}; Object.defineProperty(F,a,{value:9});
            if(F[a]!==9||Object.getOwnPropertySymbols(F)[0]!==a)throw 'function';
            'ok';
            "#,
        );
    }

    #[test]
    fn symbol_descriptor_redefinition_preserves_same_value_rules() {
        succeeds(
            r#"
            let key=Symbol(), value=Symbol('value'), o={};
            Object.defineProperty(o,key,{value:value});
            Object.defineProperty(o,key,{value:value});
            let d=Object.getOwnPropertyDescriptor(o,key), failures=0;
            if(d.value!==value||d.writable||d.enumerable||d.configurable)throw 'defaults';
            try{Object.defineProperty(o,key,{value:Symbol('value')});}catch(e){if(e.name==='TypeError')failures++;}
            try{Object.defineProperty(o,key,{writable:true});}catch(e){if(e.name==='TypeError')failures++;}
            try{Object.defineProperty(o,key,{configurable:true});}catch(e){if(e.name==='TypeError')failures++;}
            try{Object.defineProperty(o,key,{enumerable:true});}catch(e){if(e.name==='TypeError')failures++;}
            let zero=Symbol(), nan=Symbol();
            Object.defineProperty(o,zero,{value:-0});
            Object.defineProperty(o,nan,{value:NaN});
            Object.defineProperty(o,nan,{value:NaN});
            try{Object.defineProperty(o,zero,{value:0});}catch(e){if(e.name==='TypeError')failures++;}
            if(failures!==5||o[key]!==value||delete o[key])throw 'nonconfigurable';
            'ok';
            "#,
        );
    }

    #[test]
    fn symbol_define_properties_snapshots_strings_and_symbols_before_writes() {
        succeeds(
            r#"
            let a=Symbol(), b=Symbol(), target={value:7}, defs={value:{value:9}};
            defs[a]=target; defs[b]={value:5,enumerable:true};
            Object.defineProperties(target,defs);
            if(target.value!==9||target[a]!==7||target[b]!==5)throw 'snapshot';
            let keys=Object.getOwnPropertySymbols(target);
            if(keys[0]!==a||keys[1]!==b||!target.propertyIsEnumerable(b))throw 'symbol descriptors';
            let rejected={}, invalid={first:{value:1}}; invalid[a]=0;
            let caught=false;
            try{Object.defineProperties(rejected,invalid);}catch(e){caught=e.name==='TypeError';}
            if(!caught||'first' in rejected||Object.getOwnPropertySymbols(rejected).length)throw 'partial mutation';
            let created=Object.create(null,defs);
            if(created[a]!==9||created[b]!==5||Object.getPrototypeOf(created)!==null)throw 'create';
            'ok';
            "#,
        );
    }

    #[test]
    fn symbol_assign_copies_enumerable_own_keys_and_uses_throwing_set() {
        succeeds(
            r#"
            let a=Symbol(), b=Symbol(), hidden=Symbol(), inherited=Symbol();
            let parent={}; parent[inherited]=99;
            let source=Object.create(parent); source[b]=2; source.text=3; source[a]=1;
            Object.defineProperty(source,hidden,{value:4});
            let copy=Object.assign({},source), keys=Object.getOwnPropertySymbols(copy);
            if(keys.length!==2||keys[0]!==b||keys[1]!==a)throw 'copy order';
            if(copy[a]!==1||copy[b]!==2||copy.text!==3||hidden in copy||inherited in copy)throw 'copy values';
            let target={}; Object.defineProperty(target,b,{value:8});
            let caught=false;
            try{Object.assign(target,source);}catch(e){caught=e.name==='TypeError';}
            if(!caught||target.text!==3||target[b]!==8||a in target)throw 'throwing order';
            let frozen=Object.freeze({}); caught=false;
            try{Object.assign(frozen,copy);}catch(e){caught=e.name==='TypeError';}
            if(!caught||Object.getOwnPropertySymbols(frozen).length)throw 'not extensible';
            'ok';
            "#,
        );
    }

    #[test]
    fn symbol_integrity_includes_symbol_properties_and_inherited_writability() {
        succeeds(
            r#"
            let key=Symbol(), o={}; o[key]=1;
            Object.preventExtensions(o);
            if(Object.isSealed(o)||Object.isFrozen(o))throw 'unsealed symbol';
            Object.seal(o); o[key]=2;
            if(!Object.isSealed(o)||Object.isFrozen(o)||delete o[key]||o[key]!==2)throw 'seal';
            Object.freeze(o); o[key]=3;
            if(!Object.isFrozen(o)||o[key]!==2)throw 'freeze';
            let d=Object.getOwnPropertyDescriptor(o,key);
            if(d.writable||d.configurable||!d.enumerable)throw 'frozen descriptor';
            let child=Object.create(o); child[key]=4;
            if(child.hasOwnProperty(key)||child[key]!==2)throw 'inherited readonly';
            let caught=false;
            try{(function(){'use strict';child[key]=5;})();}catch(e){caught=e.name==='TypeError';}
            if(!caught)throw 'strict inherited readonly';
            let fresh=Symbol(); o[fresh]=9;
            if(fresh in o)throw 'prevent extension';
            'ok';
            "#,
        );
    }

    #[test]
    fn symbol_primitive_introspection_and_borrowed_string_conversion() {
        succeeds(
            r#"
            let symbol=Symbol('token');
            if(Object.getPrototypeOf(symbol)!==Symbol.prototype)throw 'prototype';
            if(Object.prototype.toString.call(symbol)!=='[object Symbol]')throw 'tag';
            if(Object.getOwnPropertySymbols(symbol).length)throw 'primitive own keys';
            if(Object.prototype.hasOwnProperty.call(symbol,symbol))throw 'primitive own property';
            let failures=0;
            try{String.prototype.trim.call(symbol);}catch(e){if(e.name==='TypeError')failures++;}
            try{Object.getOwnPropertySymbols(null);}catch(e){if(e.name==='TypeError')failures++;}
            if(failures!==2)throw 'conversion';
            'ok';
            "#,
        );
    }

    #[test]
    fn symbol_descriptors_use_registered_host_backing_without_native_calls() {
        let mut runtime = Runtime::for_test();
        runtime.bind_host_object(11, &[]).unwrap();
        runtime.define_global("native", Value::Host(11)).unwrap();
        runtime.define_global("opaque", Value::Host(12)).unwrap();
        assert_eq!(
            runtime
                .execute(
                    r#"
                    let key=Symbol('native'), source={}; source[key]=8;
                    Object.assign(native,source);
                    Object.defineProperty(native,key,{enumerable:false});
                    let d=Object.getOwnPropertyDescriptor(native,key);
                    if(d.value!==8||d.enumerable||!d.writable||!d.configurable)throw 'host descriptor';
                    if(!Object.prototype.hasOwnProperty.call(native,key))throw 'host own';
                    if(Object.getOwnPropertySymbols(native)[0]!==key)throw 'host keys';
                    if(Object.getOwnPropertySymbols(opaque).length)throw 'opaque keys';
                    if(Object.getOwnPropertyDescriptor(opaque,key)!==undefined)throw 'opaque descriptor';
                    'ok';
                    "#,
                    &mut NoHost,
                )
                .unwrap()
                .to_text(),
            "ok"
        );
        assert!(runtime
            .execute("Object.defineProperty(opaque,key,{value:9});", &mut NoHost)
            .is_err());
        assert!(!runtime.host_objects.contains_key(&12));
    }
}
