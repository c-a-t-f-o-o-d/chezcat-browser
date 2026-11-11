//! Original arguments objects for simple and identifier-rest parameter lists.
//! Values are retained lazily, so calls that never read arguments allocate no
//! object. Sloppy parameter mappings share the actual environment binding;
//! strict and rest-list arguments are independent, with a throwing callee slot.
//! https://tc39.es/ecma262/#sec-arguments-exotic-objects

use super::{
    Builtin, Declaration, Fault, NativeGetter, Ordinary, Program, Runtime, StmtKind, Value,
    MAX_RETAINED_VALUES,
};
use std::collections::{BTreeMap, HashSet};

pub(super) struct Pending {
    function: usize,
    unmapped: bool,
    values: Vec<Value>,
    mapped: BTreeMap<String, String>,
}

impl Pending {
    pub(super) fn bytes(&self) -> usize {
        self.values.iter().map(Value::bytes).sum()
    }
}

pub(super) struct Data {
    pub(super) environment: usize,
    pub(super) mapped: BTreeMap<String, String>,
}

impl Runtime {
    pub(super) fn prepare_arguments(
        &mut self,
        environment: usize,
        callee: usize,
        program: &Program,
        template: usize,
        values: &[Value],
    ) -> Result<(), Fault> {
        let function = &program.templates[template];
        // A named arguments parameter suppresses the implicit binding, including
        // a rest binding. Arrows keep their outer arguments binding.
        if function.arrow || function.params.iter().any(|name| name == "arguments") {
            return Ok(());
        }
        for &statement in &function.body {
            self.step()?;
            if match &program.stmts[statement].kind {
                StmtKind::Function(name, _) => name == "arguments",
                StmtKind::Declare(Declaration::Let | Declaration::Const, names) => {
                    names.iter().any(|(name, _)| name == "arguments")
                }
                _ => false,
            } {
                return Ok(());
            }
        }
        let bytes = values.iter().map(Value::bytes).sum::<usize>();
        if self.value_bytes.saturating_add(bytes) > MAX_RETAINED_VALUES {
            return Err(Fault::new("Retained value limit reached (2 MiB)."));
        }
        self.work(bytes)?;
        let mut mapped = BTreeMap::new();
        if !function.strict && !function.rest {
            let mut seen = HashSet::new();
            // Only the final occurrence of a duplicate parameter can map.
            // A missing final argument also prevents an earlier duplicate
            // from mapping, as required by CreateMappedArgumentsObject.
            for (index, name) in function.params.iter().enumerate().rev() {
                self.work(name.len())?;
                if seen.insert(name.as_str()) && index < values.len() {
                    mapped.insert(index.to_string(), name.clone());
                }
            }
        }
        self.put_binding(
            environment,
            "arguments".into(),
            Some(Value::Undefined),
            if function.strict {
                Declaration::Const
            } else {
                Declaration::Var
            },
            false,
        )?;
        self.environment_mut(environment).arguments = Some(Pending {
            function: callee,
            unmapped: function.strict || function.rest,
            values: values.to_vec(),
            mapped,
        });
        self.value_bytes += bytes;
        Ok(())
    }

    pub(super) fn materialize_arguments(&mut self, environment: usize) -> Result<(), Fault> {
        let pending = self
            .environment_mut(environment)
            .arguments
            .take()
            .ok_or_else(|| Fault::new("Invalid pending arguments object."))?;
        // Transfer the retained values into ordinary properties, rather than
        // retaining the seed and the completed object at the same time.
        self.value_bytes -= pending.bytes();
        let id = self.new_object(None)?;
        let length = pending.values.len();
        for (index, value) in pending.values.into_iter().enumerate() {
            self.set_property(id, index.to_string(), value)?;
        }
        self.store_property(
            id,
            "length".into(),
            Value::Number(length as f64),
            Some((false, true, true)),
        )?;
        if pending.unmapped {
            let thrower = self.arguments_thrower()?;
            self.store_property(id, "callee".into(), thrower, Some((false, false, false)))?;
            self.objects[id]
                .properties
                .get_mut("callee")
                .unwrap()
                .getter = Some(NativeGetter::ArgumentsCallee);
        } else {
            self.store_property(
                id,
                "callee".into(),
                Value::Function(pending.function),
                Some((false, true, true)),
            )?;
        }
        self.store_symbol_property(
            id,
            self.symbol_iterator.unwrap(),
            self.arguments_iterator_method(),
            Some((false, true, true)),
        )?;
        self.objects[id].arguments = Some(Data {
            environment,
            mapped: pending.mapped,
        });
        let kind = self.environment(environment).bindings["arguments"].kind;
        self.put_binding(
            environment,
            "arguments".into(),
            Some(Value::Object(id)),
            kind,
            true,
        )
    }

    pub(super) fn arguments_thrower(&mut self) -> Result<Value, Fault> {
        if let Some(function) = self.arguments_thrower {
            return Ok(Value::Function(function));
        }
        let value = self.native_function(Builtin::ThrowTypeError, "")?;
        let Value::Function(function) = value else {
            unreachable!()
        };
        let properties = self.ordinary_id(Ordinary::Function(function))?;
        let object = &mut self.objects[properties];
        object.extensible = false;
        for property in object.properties.values_mut() {
            property.configurable = false;
        }
        self.arguments_thrower = Some(function);
        Ok(value)
    }

    pub(super) fn mapped_argument(&self, id: usize, name: &str) -> Option<(usize, String)> {
        let data = self.objects.get(id)?.arguments.as_ref()?;
        data.mapped
            .get(name)
            .map(|binding| (data.environment, binding.clone()))
    }

    pub(super) fn disconnect_arguments(&mut self, id: usize) -> Result<(), Fault> {
        let Some(data) = self.objects[id].arguments.as_ref() else {
            return Ok(());
        };
        let environment = data.environment;
        let names = data
            .mapped
            .iter()
            .map(|(key, name)| (key.clone(), name.clone()))
            .collect::<Vec<_>>();
        for (key, name) in names {
            let value = self.binding_value(environment, &name)?;
            self.set_property(id, key, value)?;
        }
        self.objects[id].arguments.as_mut().unwrap().mapped.clear();
        Ok(())
    }
}
