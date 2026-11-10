//! Original ordinary-object prototype support. Links and callable properties
//! share the interpreter's arenas and budgets; no host object can be inserted
//! into a script-controlled prototype chain. Traversal is iterative.
//!
//! ECMAScript references: OrdinaryGet/SetPrototypeOf, OrdinaryHasInstance,
//! Object.create/getPrototypeOf/setPrototypeOf, and data property descriptors.
//! Accessor descriptors, proxies, boxed primitives and dynamic Function source
//! compilation remain explicit limitations.

use super::{
    strict_equal, Builtin, Fault, Function, Host, Ordinary, Runtime, Value, MAX_CALL_DEPTH,
    MAX_FUNCTIONS, MAX_OBJECT_PROPERTIES,
};
use std::collections::HashSet;

enum DescriptorTarget {
    Ordinary(Ordinary),
    GlobalBinding,
}

struct DataDescriptor {
    value: Option<Value>,
    enumerable: Option<bool>,
    writable: Option<bool>,
    configurable: Option<bool>,
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
        if self.functions.len() >= MAX_FUNCTIONS {
            return Err(Fault::new("Function limit reached (1024)."));
        }
        let length = match builtin {
            Builtin::Binary(operation) => operation.arity(),
            Builtin::Collection(operation) => operation.arity(),
            Builtin::TextEncoding(operation) => operation.arity(),
            Builtin::EmptyFunction | Builtin::ThrowTypeError => 0,
            Builtin::RegExp | Builtin::ParseInt | Builtin::JsonParse => 2,
            Builtin::JsonStringify => 3,
            Builtin::Math("random") => 0,
            Builtin::Math("atan2" | "hypot" | "imul" | "max" | "min" | "pow") => 2,
            Builtin::ObjectStatic(
                "create"
                | "setPrototypeOf"
                | "getOwnPropertyDescriptor"
                | "defineProperties"
                | "assign",
            ) => 2,
            Builtin::ObjectStatic("defineProperty") => 3,
            Builtin::FunctionMethod("apply") => 2,
            Builtin::FunctionMethod("toString")
            | Builtin::ObjectMethod("toString" | "valueOf")
            | Builtin::ErrorToString => 0,
            Builtin::ArrayMethod("pop" | "toString" | "shift" | "reverse" | "toReversed") => 0,
            Builtin::ArrayMethod("slice" | "splice" | "copyWithin" | "with" | "toSpliced") => 2,
            Builtin::RegExpMethod("toString") => 0,
            Builtin::StringMethod(
                "toString" | "valueOf" | "trim" | "toLowerCase" | "toUpperCase",
            ) => 0,
            Builtin::StringMethod("split" | "slice" | "substring" | "replace") => 2,
            Builtin::PrimitiveMethod(_, _) => 0,
            _ => 1,
        };
        let index = self.functions.len();
        self.functions.push(Function::Native(builtin));
        self.initialize_function_properties(name, length, false)?;
        Ok(Value::Function(index))
    }

    pub(super) fn initialize_function_properties(
        &mut self,
        name: &str,
        length: usize,
        constructible: bool,
    ) -> Result<(), Fault> {
        let index = self.functions.len() - 1;
        let properties = match self.new_object(None) {
            Ok(properties) => properties,
            Err(error) => {
                self.functions.pop();
                return Err(error);
            }
        };
        self.function_properties.push(properties);
        if let Some(prototype) = self.function_prototype {
            self.objects[properties].prototype = Some(Ordinary::Function(prototype));
        }
        self.store_property(
            properties,
            "name".into(),
            Value::String(name.into()),
            Some((false, false, true)),
        )?;
        self.store_property(
            properties,
            "length".into(),
            Value::Number(length as f64),
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
        let value = self.native_function(builtin, name)?;
        self.store_property(owner, name.into(), value, Some((false, true, true)))
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
        }
    }

    pub(super) fn constructor_prototype(
        &mut self,
        name: &str,
        prototype: Value,
    ) -> Result<(), Fault> {
        let constructor = self.binding_value(0, name)?;
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
        let function_properties = self.ordinary_id(Ordinary::Function(function))?;
        for name in ["call", "apply", "toString"] {
            self.prototype_method(function_properties, name, Builtin::FunctionMethod(name))?;
        }
        let Value::Function(object_constructor) = self.binding_value(0, "Object")? else {
            unreachable!()
        };
        let object_constructor = self.ordinary_id(Ordinary::Function(object_constructor))?;
        for name in [
            "keys",
            "create",
            "getPrototypeOf",
            "setPrototypeOf",
            "getOwnPropertyDescriptor",
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
        ] {
            self.prototype_method(array, name, Builtin::ArrayMethod(name))?;
        }
        let Value::Function(array_constructor) = self.binding_value(0, "Array")? else {
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
        self.constructor_prototype("String", Value::Object(string))?;
        for name in [
            "toString",
            "valueOf",
            "split",
            "slice",
            "substring",
            "charAt",
            "charCodeAt",
            "codePointAt",
            "trim",
            "toLowerCase",
            "toUpperCase",
            "includes",
            "indexOf",
            "startsWith",
            "endsWith",
            "search",
            "match",
            "replace",
        ] {
            self.prototype_method(string, name, Builtin::StringMethod(name))?;
        }
        for (name, number) in [("Number", true), ("Boolean", false)] {
            let prototype = self.new_object(None)?;
            if number {
                self.number_prototype = Some(prototype);
            } else {
                self.boolean_prototype = Some(prototype);
            }
            self.constructor_prototype(name, Value::Object(prototype))?;
            for method in ["toString", "valueOf"] {
                self.prototype_method(prototype, method, Builtin::PrimitiveMethod(method, number))?;
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
                .ok_or_else(|| Fault::new("Invalid function handle.")),
        }
    }

    pub(super) fn ordinary_prototype(&self, value: Ordinary) -> Result<Option<Ordinary>, Fault> {
        Ok(self.object(self.ordinary_id(value)?)?.prototype)
    }

    pub(super) fn owns_property(&self, id: usize, name: &str) -> Result<bool, Fault> {
        let object = self.object(id)?;
        if let Some(index) = self.typed_numeric_key(id, name)? {
            return Ok(self.typed_index_valid(id, index));
        }
        Ok(object.properties.contains_key(name)
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
        mut value: Ordinary,
        name: &str,
    ) -> Result<Option<usize>, Fault> {
        loop {
            self.step()?;
            let id = self.ordinary_id(value)?;
            if self.owns_property(id, name)? {
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
                if let Some(field) = self.objects[owner]
                    .properties
                    .get(name)
                    .and_then(|property| property.getter)
                {
                    self.native_getter(field, value.value())
                } else {
                    self.own_property(owner, name)
                }
            }
            None => Ok(Value::Undefined),
        }
    }

    pub(super) fn assign_ordinary_property(
        &mut self,
        object: Ordinary,
        name: String,
        value: Value,
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
        if let Some(owner) = self.property_owner(object, &name)? {
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
            {
                return if self.strict {
                    Err(Fault::exception("TypeError", "The property is read-only."))
                } else {
                    Ok(())
                };
            }
        }
        self.set_property(id, name, value)
    }

    pub(super) fn set_prototype(
        &mut self,
        object: Ordinary,
        prototype: Option<Ordinary>,
    ) -> Result<(), Fault> {
        self.step()?;
        let id = self.ordinary_id(object)?;
        if self.objects[id].prototype == prototype {
            return Ok(());
        }
        if !self.objects[id].extensible {
            return Err(Fault::exception(
                "TypeError",
                "The object is not extensible.",
            ));
        }
        if Some(id) == self.object_prototype {
            return Err(Fault::exception(
                "TypeError",
                "Object.prototype has an immutable prototype.",
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
            current = self.ordinary_prototype(parent)?;
        }
        self.objects[id].prototype = prototype;
        Ok(())
    }

    pub(super) fn inherits_from(
        &mut self,
        object: Ordinary,
        prototype: Ordinary,
    ) -> Result<bool, Fault> {
        let mut current = self.ordinary_prototype(object)?;
        while let Some(parent) = current {
            self.step()?;
            if parent == prototype {
                return Ok(true);
            }
            current = self.ordinary_prototype(parent)?;
        }
        Ok(false)
    }

    pub(super) fn inherited_keys(&mut self, object: Ordinary) -> Result<Vec<String>, Fault> {
        let mut current = Some(object);
        let mut seen = HashSet::new();
        let mut keys = Vec::new();
        while let Some(object) = current {
            self.step()?;
            let id = self.ordinary_id(object)?;
            for key in self.enumerable_keys(id)? {
                if seen.insert(key.clone()) {
                    if keys.len() >= MAX_OBJECT_PROPERTIES {
                        return Err(Fault::new("for-in enumeration limit reached (4096 keys)."));
                    }
                    keys.push(key);
                }
            }
            // A nonenumerable own property also shadows an inherited name.
            self.work(
                self.objects[id]
                    .properties
                    .len()
                    .saturating_mul(super::MAX_PROPERTY_BYTES),
            )?;
            seen.extend(self.objects[id].properties.keys().cloned());
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

    pub(super) fn own_descriptor(&mut self, object: Ordinary, name: &str) -> Result<Value, Fault> {
        self.property_name(name)?;
        let id = self.ordinary_id(object)?;
        if !self.owns_property(id, name)? {
            return Ok(Value::Undefined);
        }
        if let Some(field) = self.objects[id]
            .properties
            .get(name)
            .and_then(|property| property.getter)
        {
            let getter = self.copy_value(&self.objects[id].properties[name].value.clone())?;
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

    fn data_descriptor(&mut self, descriptor: Ordinary) -> Result<DataDescriptor, Fault> {
        // Accessors require ordinary [[Get]]/[[Set]] callback semantics; do not
        // install a partial descriptor or silently replace one with a value.
        if self.property_owner(descriptor, "get")?.is_some()
            || self.property_owner(descriptor, "set")?.is_some()
        {
            return Err(Fault::new(
                "Accessor property descriptors are not implemented.",
            ));
        }
        let mut data = DataDescriptor {
            value: None,
            enumerable: None,
            writable: None,
            configurable: None,
        };
        for (key, target) in [
            ("enumerable", &mut data.enumerable),
            ("writable", &mut data.writable),
            ("configurable", &mut data.configurable),
        ] {
            if self.property_owner(descriptor, key)?.is_some() {
                *target = Some(self.get_ordinary_property(descriptor, key)?.truthy());
            }
        }
        if self.property_owner(descriptor, "value")?.is_some() {
            data.value = Some(self.get_ordinary_property(descriptor, "value")?);
        }
        Ok(data)
    }

    fn define_data_property(
        &mut self,
        object: Ordinary,
        name: String,
        descriptor: DataDescriptor,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.property_name(&name)?;
        let id = self.ordinary_id(object)?;
        if let Some(index) = self.typed_numeric_key(id, &name)? {
            if !self.typed_index_valid(id, index)
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
        if self.objects[id]
            .properties
            .get(&name)
            .is_some_and(|property| {
                matches!(property.getter, Some(super::NativeGetter::ArgumentsCallee))
            })
        {
            return if descriptor.value.is_some()
                || descriptor.writable.is_some()
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
        if self.objects[id]
            .properties
            .get(&name)
            .is_some_and(|property| property.getter.is_some())
        {
            return Err(Fault::new(
                "Changing built-in accessor descriptors is not implemented.",
            ));
        }
        if self.objects[id].regexp.is_some()
            && self.owns_property(id, &name)?
            && !self.objects[id].properties.contains_key(&name)
            || self.objects[id].length.is_some()
                && (name == "length" || super::property_index(&name).is_some())
        {
            return Err(Fault::new(
                "Array-index/length and RegExp accessor descriptor changes are not implemented.",
            ));
        }
        let previous = self.objects[id].properties.get(&name);
        let old_attrs = previous.map(|property| {
            (
                property.enumerable,
                property.writable,
                property.configurable,
            )
        });
        let old = old_attrs.unwrap_or((false, false, false));
        let attrs = (
            descriptor.enumerable.unwrap_or(old.0),
            descriptor.writable.unwrap_or(old.1),
            descriptor.configurable.unwrap_or(old.2),
        );
        let value = if let Some(value) = descriptor.value {
            value
        } else if old_attrs.is_some() {
            self.own_property(id, &name)?
        } else {
            Value::Undefined
        };
        if let Some(old) = old_attrs {
            if !old.2
                && (attrs.2
                    || old.0 != attrs.0
                    || !old.1
                        && (attrs.1
                            || !same_value(&self.objects[id].properties[&name].value, &value)))
            {
                return Err(Fault::exception(
                    "TypeError",
                    "Cannot redefine a nonconfigurable property.",
                ));
            }
        }
        self.store_property(id, name, value, Some(attrs))
    }

    fn target_descriptor(&mut self, target: DescriptorTarget, name: &str) -> Result<Value, Fault> {
        if let DescriptorTarget::Ordinary(object) = target {
            return self.own_descriptor(object, name);
        }
        let value = self.binding_value(0, name)?;
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
        descriptor: DataDescriptor,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if let DescriptorTarget::Ordinary(object) = target {
            return self.define_data_property(object, name, descriptor, host);
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
        let previous = self.binding_value(0, &name)?;
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
        let kind = self.environment(0).bindings[&name].kind;
        self.put_binding(0, name.clone(), Some(value), kind, true)?;
        self.global_attributes.insert(name, attrs);
        Ok(())
    }

    fn object_integrity(&mut self, method: &str, value: &Value) -> Result<Value, Fault> {
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
        self.work(
            self.objects[id]
                .properties
                .len()
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
                && object.properties.values().all(|property| {
                    !property.configurable
                        && (!frozen || property.getter.is_some() || !property.writable)
                })
                && (!frozen
                    || (!has_elements && (object.length.is_none() || !object.length_writable)));
            return Ok(Value::Bool(result));
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
        for property in object.properties.values_mut() {
            property.configurable = false;
            if frozen && property.getter.is_none() {
                property.writable = false;
            }
        }
        if frozen && object.length.is_some() {
            object.length_writable = false;
        }
        self.copy_value(value)
    }

    fn descriptor_target(
        &self,
        value: &Value,
        name: &str,
        host: &dyn Host,
    ) -> Result<DescriptorTarget, Fault> {
        if let Value::Host(handle) = value {
            if self.is_browser_global(*handle) {
                if host.owns_global_property(name) || self.host_globals.contains(name) {
                    return Err(Fault::new("Descriptors of native or declared global properties cannot be changed by this subset."));
                }
                if self.global_binding(name) {
                    return Ok(DescriptorTarget::GlobalBinding);
                }
                let (_, properties) = self.browser_global.unwrap();
                return Ok(DescriptorTarget::Ordinary(Ordinary::Object(properties)));
            }
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
        match method {
            "preventExtensions" | "seal" | "freeze" | "isExtensible" | "isSealed" | "isFrozen" => {
                self.object_integrity(method, first)
            }
            "keys" => self.object_keys(first),
            "isArray" => Ok(Value::Bool(
                matches!(first, Value::Object(id) if self.object(*id)?.length.is_some()),
            )),
            "getPrototypeOf" => {
                let prototype = match first {
                    Value::String(_) => self.string_prototype.map(Ordinary::Object),
                    Value::Number(_) => self.number_prototype.map(Ordinary::Object),
                    Value::Bool(_) => self.boolean_prototype.map(Ordinary::Object),
                    _ => {
                        let object = self.ordinary_required(first)?;
                        self.ordinary_prototype(object)?
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
                } else if matches!(first, Value::Host(_)) {
                    return Err(Fault::new(
                        "Native host object prototypes cannot be changed.",
                    ));
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
                let object = self.descriptor_target(first, &key, host)?;
                if method == "getOwnPropertyDescriptor" {
                    return self.target_descriptor(object, &key);
                }
                let descriptor =
                    self.ordinary_required(args.get(2).unwrap_or(&Value::Undefined))?;
                let descriptor = self.data_descriptor(descriptor)?;
                self.define_target_property(object, key, descriptor, host)?;
                self.copy_value(first)
            }
            "defineProperties" => {
                self.define_data_properties(first, args.get(1).unwrap_or(&Value::Undefined), host)?;
                self.copy_value(first)
            }
            "assign" => {
                let target = self.ordinary_required(first)?;
                for source in args.get(1..).unwrap_or(&[]) {
                    if matches!(source, Value::Null | Value::Undefined) {
                        continue;
                    }
                    let source = self.ordinary_required(source)?;
                    let id = self.ordinary_id(source)?;
                    for key in self.enumerable_keys(id)? {
                        let value = self.get_ordinary_property(source, &key)?;
                        // Object.assign uses Set with Throw=true in every
                        // caller, including a non-strict script.
                        let previous_strict = self.strict;
                        self.strict = true;
                        let result = self.assign_ordinary_property(target, key, value, host);
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
        // Reject an invalid target even when the descriptor list is empty.
        if !matches!(target, Value::Host(handle) if self.is_browser_global(*handle)) {
            self.ordinary_required(target)?;
        }
        let descriptors = self.ordinary_required(descriptors)?;
        let id = self.ordinary_id(descriptors)?;
        let mut entries = Vec::new();
        let mut snapshot_bytes = 0usize;
        for key in self.enumerable_keys(id)? {
            let value = self.get_ordinary_property(descriptors, &key)?;
            let descriptor = self.ordinary_required(&value)?;
            // Snapshot every field before applying the first property. A
            // descriptor object may itself be the destination of an earlier
            // definition; retaining its handle would read the mutated value.
            let descriptor = self.data_descriptor(descriptor)?;
            snapshot_bytes =
                snapshot_bytes.saturating_add(descriptor.value.as_ref().map_or(0, Value::bytes));
            if snapshot_bytes > super::MAX_RETAINED_VALUES {
                return Err(Fault::new("Descriptor snapshot limit reached (2 MiB)."));
            }
            entries.push((key, descriptor));
        }
        for (key, descriptor) in entries {
            let target = self.descriptor_target(target, &key, host)?;
            self.define_target_property(target, key, descriptor, host)?;
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
        if method == "toString" {
            let name = match &this {
                Value::Undefined => "Undefined",
                Value::Null => "Null",
                Value::Bool(_) => "Boolean",
                Value::Number(_) => "Number",
                Value::String(_) => "String",
                Value::Function(_) => "Function",
                Value::Object(id) => {
                    let object = self.object(*id)?;
                    if let Some(binary) = &object.binary {
                        binary.tag()
                    } else if let Some(collection) = &object.collection {
                        collection.tag()
                    } else if let Some(iterator) = &object.iterator {
                        iterator.tag()
                    } else if let Some(text) = &object.text {
                        text.tag()
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
                    } else {
                        "Object"
                    }
                }
                Value::Host(_) => {
                    return Err(Fault::new("Native host object tags are not implemented."))
                }
            };
            return self.checked_value(Value::String(format!("[object {name}]")));
        }
        if method == "isPrototypeOf" {
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
            return self.inherits_from(value, prototype).map(Value::Bool);
        }
        if method == "valueOf" {
            Self::require_property_receiver(&this)?;
            if matches!(this, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
                return Ok(this);
            }
            return Err(Fault::new("Primitive wrapper objects are not implemented."));
        }
        // Both own-property methods convert their argument before ToObject on
        // the receiver, including when that receiver is null or undefined.
        let key = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        let key = self.property_key(key, host)?;
        Self::require_property_receiver(&this)?;
        let owned = if let Some(value) = Ordinary::from_value(&this) {
            let id = self.ordinary_id(value)?;
            if method == "propertyIsEnumerable" {
                self.own_enumerable(id, &key)?
            } else {
                self.owns_property(id, &key)?
            }
        } else if let Value::String(text) = this {
            self.work(text.len())?;
            let index = super::property_index(&key)
                .is_some_and(|index| index < text.encode_utf16().count());
            index || method != "propertyIsEnumerable" && key == "length"
        } else if matches!(this, Value::Host(_)) {
            return Err(Fault::new(
                "Native host own-property descriptors are not implemented.",
            ));
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
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = self.prototype_builtin_inner(builtin, args, this, host);
        self.call_depth -= 1;
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
                if method == "toString" {
                    if matches!(self.functions.get(index), Some(Function::Script { .. })) {
                        return Err(Fault::new(
                            "Script function source-string conversion is not implemented.",
                        ));
                    }
                    let name = self.get_ordinary_property(Ordinary::Function(index), "name")?;
                    let name = if let Value::String(name) = name {
                        name
                    } else {
                        String::new()
                    };
                    return self.checked_value(Value::String(format!(
                        "function {name}() {{ [native code] }}"
                    )));
                }
                let receiver = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                if method == "apply" {
                    let arguments =
                        self.apply_arguments(args.get(1).unwrap_or(&Value::Undefined))?;
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
                let Value::Object(id) = this else {
                    return Err(Fault::new(
                        "Borrowed array methods require an ordinary object in this subset.",
                    ));
                };
                if method == "toString" {
                    let join = self.get_property(id, "join")?;
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
                self.regexp_method(id, method, args)
            }
            Builtin::ErrorToString => {
                let object = self.ordinary_required(&this)?;
                let id = self.ordinary_id(object)?;
                self.error_text(id)
            }
            Builtin::StringMethod(method) => {
                let receiver = match this {
                    Value::Object(id) if Some(id) == self.string_prototype => {
                        Value::String(String::new())
                    }
                    Value::String(_) => this,
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
                        value.primitive()?;
                        self.work(value.bytes())?;
                        Value::String(value.to_text())
                    }
                };
                self.call_member(receiver, method, args, host)
            }
            Builtin::PrimitiveMethod(method, number) => {
                let receiver = match this {
                    Value::Object(id) if number && Some(id) == self.number_prototype => {
                        Value::Number(0.0)
                    }
                    Value::Object(id) if !number && Some(id) == self.boolean_prototype => {
                        Value::Bool(false)
                    }
                    Value::Number(_) if number => this,
                    Value::Bool(_) if !number => this,
                    _ => {
                        return Err(Fault::exception(
                            "TypeError",
                            "Primitive value method has an incompatible receiver.",
                        ))
                    }
                };
                if method == "valueOf" {
                    Ok(receiver)
                } else {
                    self.call_member(receiver, method, args, host)
                }
            }
            _ => Err(Fault::new("Invalid prototype builtin.")),
        }
    }
}

fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            a.is_nan() && b.is_nan() || a.to_bits() == b.to_bits()
        }
        _ => strict_equal(left, right),
    }
}
