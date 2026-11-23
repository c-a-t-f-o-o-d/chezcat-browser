//! Class definitions with Script/null, selected DOM-interface, collection and Error bases.
//! Heritage and computed member names evaluate in the private TDZ scope.
//! https://tc39.es/ecma262/#sec-runtime-semantics-classdefinitionevaluation
use super::*;

impl Runtime {
    pub(super) fn make_class(
        &mut self, program: &Rc<Program>, class: usize, environment: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let marker = self.transient_marker();
        let scope = self.new_environment(environment, false)?;
        let syntax = &program.classes[class];
        let result = (|| {
            self.push_transient_root(function_reachability::Edge::Environment(scope))?;
            if let Some(name) = &syntax.name {
                self.put_binding(scope, name.clone(), None, Declaration::Const, false)?;
            }
            let (constructor_parent, instance_parent) = if let Some(heritage) = syntax.heritage {
                let base = self.eval(program, heritage, scope, this, host)?;
                self.push_transient_value(&base)?;
                if matches!(base, Value::Null) {
                    (self.function_prototype.map(Ordinary::Function), None)
                } else {
                    if !self.from_constructor(&base)? {
                        return Err(Fault::exception("TypeError", "A class base must be a constructor or null."));
                    }
                    let Value::Function(base) = base else { unreachable!() };
                    if !self.supported_class_base(base, host)? {
                        return Err(Fault::new("This native, bound or builtin class base is not implemented."));
                    }
                    let prototype = self.get_ordinary_property_with_host(Ordinary::Function(base), "prototype", host)?;
                    self.push_transient_value(&prototype)?;
                    let instance_parent = match prototype {
                        Value::Null => None,
                        Value::Host(_) => return Err(Fault::new("Native host object class prototypes are not implemented.")),
                        _ => Some(Ordinary::from_value(&prototype).ok_or_else(|| Fault::exception(
                            "TypeError", "A class base prototype must be an object or null."))?),
                    };
                    (Some(Ordinary::Function(base)), instance_parent)
                }
            } else {
                (self.function_prototype.map(Ordinary::Function), self.object_prototype.map(Ordinary::Object))
            };
            self.collect_before_function_literal()?;
            let inhibited = self.inhibit_transient_collection_for("class-definition");
            let result = self.make_class_unpublished(program, class, scope, this, constructor_parent, instance_parent, host);
            self.restore_transient_marker(inhibited);
            result
        })();
        self.release_environment(scope);
        self.restore_transient_marker(marker);
        result
    }

    fn make_class_unpublished(
        &mut self, program: &Rc<Program>, class: usize, scope: usize, this: &Value,
        constructor_parent: Option<Ordinary>, instance_parent: Option<Ordinary>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        let syntax = &program.classes[class];
        let functions = syntax.members.len().checked_add(1)
            .ok_or_else(|| Fault::new("Class member capacity overflow."))?;
        if self.function_count_refused(functions) {
            return Err(self.function_limit_fault("class"));
        }
        // Every callable owns one metadata object; the constructor additionally
        // owns its genuine instance prototype. No class-member getter executes.
        let objects = functions.checked_add(1).ok_or_else(|| Fault::new("Class object capacity overflow."))?;
        if objects > self.limits.heap.objects.saturating_sub(self.objects.len()) {
            return Err(Fault::new(format!("Object identity limit reached ({} per document).", self.limits.heap.objects)));
        }
        self.work_as(functions.saturating_mul(std::mem::size_of::<class_parser::Member>()), WorkKind::PatternPreparation)?;
        let result = (|| {
            let constructor = self.make_function(program, syntax.constructor, scope, this)?;
            let Value::Function(constructor_id) = constructor else { unreachable!() };
            self.scope_assigned_closures.insert(constructor_id);
            self.push_transient_root(function_reachability::Edge::Function(constructor_id))?;
            let properties = self.ordinary_id(Ordinary::Function(constructor_id))?;
            let StoredValue::Value(Value::Object(prototype)) = self.objects[properties].properties["prototype"].value else {
                return Err(Fault::new("Invalid class prototype identity."));
            };
            self.set_prototype(Ordinary::Function(constructor_id), constructor_parent)?;
            self.set_prototype(Ordinary::Object(prototype), instance_parent)?;
            if let Function::Script { home_object, .. } = &mut self.functions[constructor_id] {
                *home_object = Some(Ordinary::Object(prototype));
            }
            self.store_property(properties, "prototype".into(), Value::Object(prototype), Some((false, false, false)))?;
            self.class_function_name(constructor_id, syntax.name.as_deref().unwrap_or(""), object_literals::Kind::Method)?;
            for member in &syntax.members {
                self.step()?;
                let key = match &member.key {
                    class_parser::MemberKey::Literal(name) => {
                        self.work(name.len())?;
                        PropertyKey::String(name.clone())
                    }
                    class_parser::MemberKey::Computed(expression) => {
                        let previous_strict = std::mem::replace(&mut self.strict, true);
                        let key = self.eval(program, *expression, scope, this, host)
                            .and_then(|value| self.property_key(value, host));
                        self.strict = previous_strict;
                        key?
                    }
                };
                if member.is_static && matches!(&key, PropertyKey::String(name) if name == "prototype") {
                    return Err(Fault::exception("TypeError", "Cannot redefine the class prototype."));
                }
                let method = self.make_function(program, member.template, scope, this)?;
                let Value::Function(method_id) = method else { unreachable!() };
                self.object_assigned_closures.insert(method_id);
                match &key {
                    PropertyKey::String(name) => self.class_function_name(method_id, name, member.kind)?,
                    PropertyKey::StringUnits(_) | PropertyKey::Symbol(_) => {
                        self.literal_function_name(method_id, &key, member.kind)?;
                    }
                }
                let owner = if member.is_static { properties } else { prototype };
                if let Function::Script { home_object, .. } = &mut self.functions[method_id] {
                    *home_object = Some(if member.is_static { Ordinary::Function(constructor_id) } else { Ordinary::Object(prototype) });
                }
                self.define_member_property(owner, key, method, member.kind, false)?;
            }
            if let Some(name) = &syntax.name {
                self.put_binding(scope, name.clone(), Some(Value::Function(constructor_id)), Declaration::Const, true)?;
            }
            Ok(Value::Function(constructor_id))
        })();
        result
    }

    fn class_function_name(&mut self, function: usize, key: &str, kind: object_literals::Kind) -> Result<(), Fault> {
        let prefix = match kind {
            object_literals::Kind::Getter => "get ", object_literals::Kind::Setter => "set ", _ => "",
        };
        let bytes = prefix.len().saturating_add(key.len());
        if bytes > self.limits.capacity.string_bytes { return Err(self.string_limit_fault()); }
        self.work(bytes)?;
        let mut name = String::new();
        name.try_reserve_exact(bytes).map_err(|_| Fault::new("Unable to reserve bounded class member name."))?;
        name.push_str(prefix);
        name.push_str(key);
        let properties = self.ordinary_id(Ordinary::Function(function))?;
        self.store_property(properties, "name".into(), Value::String(name), Some((false, false, true)))
    }

    pub(super) fn infer_class_name(&mut self, program: &Program, expression: ExprId, name: &str, value: &Value) -> Result<(), Fault> {
        let ExprKind::Class(class) = program.exprs[expression].kind else { return Ok(()) };
        let syntax = &program.classes[class];
        // NamedEvaluation supplies metadata, never a private lexical binding.
        // A static name method/accessor is installed after that metadata and
        // therefore takes precedence even for an inferred anonymous class.
        if syntax.name.is_some() { return Ok(()); }
        let Value::Function(function) = value else { return Err(Fault::new("Invalid class name target.")) };
        if self.class_has_static_name(*function)? { return Ok(()); }
        self.class_function_name(*function, name, object_literals::Kind::Method)
    }

    pub(super) fn infer_class_property_name(&mut self, program: &Program, expression: ExprId, key: &PropertyKey, value: &Value) -> Result<(), Fault> {
        let ExprKind::Class(class) = program.exprs[expression].kind else { return Ok(()) };
        let syntax = &program.classes[class];
        if syntax.name.is_some() { return Ok(()); }
        let Value::Function(function) = value else { return Err(Fault::new("Invalid class name target.")) };
        if self.class_has_static_name(*function)? { return Ok(()); }
        self.literal_function_name(*function, key, object_literals::Kind::Method)
    }

    fn class_has_static_name(&self, function: usize) -> Result<bool, Fault> {
        let properties = self.ordinary_id(Ordinary::Function(function))?;
        Ok(self.objects[properties].properties.get("name").is_some_and(|descriptor| {
            descriptor.accessor.is_some() || matches!(descriptor.value, StoredValue::Value(Value::Function(_)))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("Class keys must not read a native object")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("Class keys must not write a native object")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("Class keys must not call a native object")
        }
    }

    #[test]
    fn class_raw_utf16_member_keys_names_and_descriptors_preserve_units() {
        let mut runtime = Runtime::for_test();
        let result = runtime.execute(r#"
var raw='\ud800',access='access\udfff',staticKey='static\ud800';
class Example {
    [raw](){return this.value;}
    get [access](){return this.value+1;}
    set [access](value){this.value=value;}
    '\udfff'(){return 7;}
    static [staticKey](){return 11;}
}
var instance=new Example();instance.value=3;
if(instance[raw]()!==3||instance['\udfff']()!==7||Example[staticKey]()!==11||
    instance[raw].name!==raw||instance['\udfff'].name!=='\udfff'||Example[staticKey].name!==staticKey)
    throw 'class raw method identity';
instance[access]=13;
if(instance[access]!==14||Object.hasOwn(instance,access))throw 'class raw accessor receiver';
var descriptor=Object.getOwnPropertyDescriptor(Example.prototype,access);
if(descriptor.enumerable||!descriptor.configurable||descriptor.get.name!=='get '+access||
    descriptor.set.name!=='set '+access)throw 'class raw accessor descriptor';
descriptor=Object.getOwnPropertyDescriptor(Example.prototype,raw);
if(descriptor.enumerable||!descriptor.writable||!descriptor.configurable||Object.hasOwn(Example.prototype,'\ufffd'))
    throw 'class raw method descriptor';
var inferred={[raw]:class {}};
if(inferred[raw].name!==raw)throw 'inferred raw class name';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(result.to_text(), "ok");
        assert!(runtime.transient_registration_is_empty());
    }
}
