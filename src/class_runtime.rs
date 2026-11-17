//! Genuine finite base classes. Literal members install ordinary descriptors
//! without global Object lookups or author callbacks. The private transaction
//! stays rooted/inhibited until its constructor is returned to the evaluator.
//! https://tc39.es/ecma262/#sec-runtime-semantics-classdefinitionevaluation
use super::*;

impl Runtime {
    pub(super) fn make_class(
        &mut self, program: &Rc<Program>, class: usize, environment: usize, this: &Value,
    ) -> Result<Value, Fault> {
        let marker = self.inhibit_transient_collection_for("class-definition");
        let result = self.make_class_unpublished(program, class, environment, this);
        self.restore_transient_marker(marker);
        result
    }

    fn make_class_unpublished(
        &mut self, program: &Rc<Program>, class: usize, environment: usize, this: &Value,
    ) -> Result<Value, Fault> {
        self.step()?;
        let syntax = &program.classes[class];
        let functions = syntax.members.len().checked_add(1)
            .ok_or_else(|| Fault::new("Class member capacity overflow."))?;
        if functions > self.limits.capacity.functions.saturating_sub(self.function_arena.occupied) {
            return Err(self.function_limit_fault("class"));
        }
        // Every callable owns one metadata object; the constructor additionally
        // owns its genuine instance prototype. No class-member getter executes.
        let objects = functions.checked_add(1).ok_or_else(|| Fault::new("Class object capacity overflow."))?;
        if objects > self.limits.heap.objects.saturating_sub(self.objects.len()) {
            return Err(Fault::new(format!("Object identity limit reached ({} per document).", self.limits.heap.objects)));
        }
        self.work_as(functions.saturating_mul(std::mem::size_of::<class_parser::Member>()), WorkKind::PatternPreparation)?;
        let scope = self.new_environment(environment, false)?;
        let result = (|| {
            self.push_transient_root(function_reachability::Edge::Environment(scope))?;
            if let Some(name) = &syntax.name {
                self.put_binding(scope, name.clone(), None, Declaration::Const, false)?;
            }
            let constructor = self.make_function(program, syntax.constructor, scope, this)?;
            let Value::Function(constructor_id) = constructor else { unreachable!() };
            self.scope_assigned_closures.insert(constructor_id);
            self.push_transient_root(function_reachability::Edge::Function(constructor_id))?;
            let properties = self.ordinary_id(Ordinary::Function(constructor_id))?;
            let StoredValue::Value(Value::Object(prototype)) = self.objects[properties].properties["prototype"].value else {
                return Err(Fault::new("Invalid class prototype identity."));
            };
            self.store_property(properties, "prototype".into(), Value::Object(prototype), Some((false, false, false)))?;
            self.class_function_name(constructor_id, syntax.name.as_deref().unwrap_or(""), object_literals::Kind::Method)?;
            if let Some(name) = &syntax.name {
                self.put_binding(scope, name.clone(), Some(Value::Function(constructor_id)), Declaration::Const, true)?;
            }
            for member in &syntax.members {
                self.step()?;
                let method = self.make_function(program, member.template, scope, this)?;
                let Value::Function(method_id) = method else { unreachable!() };
                self.object_assigned_closures.insert(method_id);
                self.class_function_name(method_id, &member.name, member.kind)?;
                let owner = if member.is_static { properties } else { prototype };
                if matches!(member.kind, object_literals::Kind::Getter | object_literals::Kind::Setter) {
                    let mut accessor = self.objects[owner].properties.get(&member.name)
                        .and_then(|property| property.accessor).unwrap_or(Accessors { get: None, set: None });
                    if member.kind == object_literals::Kind::Getter { accessor.get = Some(method_id); }
                    else { accessor.set = Some(method_id); }
                    self.store_accessor_property(owner, member.name.clone(), accessor, (false, true))?;
                } else {
                    self.store_property(owner, member.name.clone(), Value::Function(method_id), Some((false, true, true)))?;
                }
            }
            Ok(Value::Function(constructor_id))
        })();
        self.release_environment(scope);
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
        self.work(syntax.members.len())?;
        if syntax.members.iter().any(|member| member.is_static && member.name == "name") {
            return Ok(());
        }
        let Value::Function(function) = value else { return Err(Fault::new("Invalid class name target.")) };
        self.class_function_name(*function, name, object_literals::Kind::Method)
    }

    pub(super) fn infer_class_property_name(&mut self, program: &Program, expression: ExprId, key: &PropertyKey, value: &Value) -> Result<(), Fault> {
        let ExprKind::Class(class) = program.exprs[expression].kind else { return Ok(()) };
        let syntax = &program.classes[class];
        if syntax.name.is_some() { return Ok(()); }
        self.work(syntax.members.len())?;
        if syntax.members.iter().any(|member| member.is_static && member.name == "name") { return Ok(()); }
        let Value::Function(function) = value else { return Err(Fault::new("Invalid class name target.")) };
        self.literal_function_name(*function, key, object_literals::Kind::Method)
    }
}
