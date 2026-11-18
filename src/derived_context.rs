//! Function-environment invocation data. Arrows retain the environment identity,
//! so an uninitialized derived `this` is not read or copied during creation.
use super::*;

pub(super) enum ThisBinding { Uninitialized, Initialized(Value) }

pub(super) struct Data {
    pub(super) callee: usize,
    pub(super) new_target: Option<usize>,
    pub(super) this_binding: ThisBinding,
    pub(super) home_object: Option<Ordinary>,
    pub(super) derived_constructor: bool,
}

impl Data {
    pub(super) fn bytes(&self) -> usize {
        match &self.this_binding { ThisBinding::Initialized(value) => value.bytes(), ThisBinding::Uninitialized => 0 }
    }
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(function_reachability::Edge)) {
        use function_reachability::Edge;
        add(Edge::Function(self.callee));
        if let Some(target) = self.new_target { add(Edge::Function(target)); }
        if let ThisBinding::Initialized(value) = &self.this_binding { Edge::value(value, add); }
        if let Some(home) = self.home_object { Edge::value(&home.value(), add); }
    }
}

pub(super) struct SuperCallTarget {
    pub(super) constructor: Value,
    pub(super) new_target: usize,
    pub(super) environment: usize,
}

impl Runtime {
    pub(super) fn invocation_data(&self, context: InvocationContext) -> Result<&Data, Fault> {
        self.environments.get(context.environment).and_then(Option::as_ref)
            .and_then(|environment| environment.invocation.as_ref())
            .ok_or_else(|| Fault::new("Invalid function invocation environment."))
    }

    pub(super) fn context_this(&mut self, fallback: &Value) -> Result<Value, Fault> {
        let Some(context) = self.invocation else { return self.copy_value(fallback); };
        let bytes = match &self.invocation_data(context)?.this_binding {
            ThisBinding::Uninitialized => return Err(Self::uninitialized_this_fault()),
            ThisBinding::Initialized(value) => value.bytes(),
        };
        self.work_as(bytes, WorkKind::ValueCopy)?;
        let ThisBinding::Initialized(value) = &self.invocation_data(context)?.this_binding else { unreachable!() };
        Ok(value.clone())
    }

    pub(super) fn capture_arrow_this(&mut self, fallback: &Value) -> Result<Value, Fault> {
        let bytes = match self.invocation {
            Some(context) => self.invocation_data(context)?.bytes(),
            None => fallback.bytes(),
        };
        if self.value_bytes.saturating_add(bytes) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work_as(bytes, WorkKind::ValueCopy)?;
        // This fallback/input-root snapshot is not GetThisBinding. The traced
        // environment handle remains authoritative, including after super().
        match self.invocation {
            Some(context) => match &self.invocation_data(context)?.this_binding {
                ThisBinding::Initialized(value) => Ok(value.clone()),
                ThisBinding::Uninitialized => Ok(Value::Undefined),
            },
            None => Ok(fallback.clone()),
        }
    }

    pub(super) fn release_unentered_invocation(&mut self, function: usize) {
        let ordinary = matches!(self.functions.get(function), Some(Function::Script { program, template, .. }) if !program.templates[*template].arrow);
        if ordinary {
            if let Some(context) = self.invocation { self.release_environment(context.environment); }
        }
    }

    #[cold]
    #[inline(never)]
    fn uninitialized_this_fault() -> Fault {
        Fault::exception("ReferenceError", "Derived this is uninitialized before super construction.")
    }

    pub(super) fn super_reference(&mut self, name: &str) -> Result<Reference, Fault> {
        self.step()?;
        let context = self.invocation.ok_or_else(|| Fault::exception("SyntaxError", "super requires a method environment."))?;
        let home = self.invocation_data(context)?.home_object
            .ok_or_else(|| Fault::exception("SyntaxError", "super requires a home object."))?;
        let receiver = self.context_this(&Value::Undefined)?;
        let base = self.ordinary_prototype(home)?;
        self.property_name(name)?;
        self.work(name.len())?;
        Ok(Reference::SuperProperty { base, receiver, name: name.into() })
    }

    pub(super) fn prepare_super_call(&mut self) -> Result<SuperCallTarget, Fault> {
        self.step()?;
        let context = self.invocation.ok_or_else(|| Fault::exception("SyntaxError", "super construction requires a derived constructor."))?;
        let data = self.invocation_data(context)?;
        if !data.derived_constructor {
            return Err(Fault::exception("SyntaxError", "super construction requires a derived constructor."));
        }
        let callee = data.callee;
        let new_target = data.new_target.ok_or_else(|| Fault::new("Derived invocation has no new target."))?;
        let constructor = self.ordinary_prototype(Ordinary::Function(callee))?.map_or(Value::Null, Ordinary::value);
        Ok(SuperCallTarget { constructor, new_target, environment: context.environment })
    }

    pub(super) fn finish_super_call(&mut self, target: SuperCallTarget, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        if !self.from_constructor(&target.constructor)? {
            return Err(Fault::exception("TypeError", "The super value is not a constructor."));
        }
        let Value::Function(constructor) = target.constructor else { unreachable!() };
        if !self.supported_class_base(constructor, host)? {
            return Err(Fault::new("This native, bound or builtin super constructor is not implemented."));
        }
        let value = self.construct_value_with_target(Value::Function(constructor), args, Some(target.new_target), host)?;
        // Construct runs first. A duplicate super() must retain its real base
        // constructor effects before BindThisValue refuses the second binding.
        let context = InvocationContext { environment: target.environment };
        if !matches!(self.invocation_data(context)?.this_binding, ThisBinding::Uninitialized) {
            return Err(Fault::exception("ReferenceError", "Derived this has already been initialized."));
        }
        if !matches!(value, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
            return Err(Fault::new("Super construction did not return an object."));
        }
        let retained = value.bytes();
        if self.value_bytes.saturating_add(retained) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        let bound = self.copy_value(&value)?;
        self.environment_mut(target.environment).invocation.as_mut().expect("validated function environment")
            .this_binding = ThisBinding::Initialized(bound);
        self.value_bytes += retained;
        Ok(value)
    }

    pub(super) fn default_derived_super(&mut self, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        let target = self.prepare_super_call()?;
        let marker = self.root_marker();
        let result = (|| {
            self.push_transient_value(&target.constructor)?;
            self.push_transient_root(function_reachability::Edge::Function(target.new_target))?;
            self.push_transient_root(function_reachability::Edge::Environment(target.environment))?;
            self.finish_super_call(target, args, host)
        })();
        self.restore_root_marker(marker);
        result
    }

    pub(super) fn complete_derived_return(&mut self, value: Value) -> Result<Value, Fault> {
        match value {
            object @ (Value::Object(_) | Value::Function(_) | Value::Host(_)) => Ok(object),
            Value::Undefined => self.context_this(&Value::Undefined),
            _ => Err(Fault::exception("TypeError", "A derived constructor may only return an object or undefined.")),
        }
    }

    #[inline(never)]
    pub(super) fn finish_script_constructor_result(&mut self, template: &Template,
        args: &[Value], host: &mut dyn Host, result: &mut Result<Value, Fault>) {
        if !template.derived_constructor || result.is_err() { return; }
        // The parser's synthetic default constructor has no source body. Its
        // real forwarding algorithm runs after that empty body, before cleanup.
        // Mutate the caller's existing Result so ordinary recursive body frames
        // do not retain a second constructor-only completion temporary.
        let previous = std::mem::replace(result, Ok(Value::Undefined));
        *result = previous.and_then(|value| {
            if template.default_constructor { self.default_derived_super(args, host)?; }
            self.complete_derived_return(value)
        });
    }
}
