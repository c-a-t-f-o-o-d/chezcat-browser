//! Ordinary public slots for explicitly selected host operations.
//!
//! Capability identity lives in the function arena, not in the public name.
//! Replacing or deleting a property never reinstalls a hidden native binding.
//! References: Web IDL operations and ECMA-262 2026 Global Environment Records.
//! Native attributes, scheduler capabilities and host prototypes are not widened.

use super::{Fault, Runtime, Value};

impl Runtime {
    pub(crate) fn define_global_operation(
        &mut self,
        name: &'static str,
        object: u32,
        method: &'static str,
        length: usize,
    ) -> Result<(), String> {
        let Some((handle, properties)) = self.browser_global else {
            return Err("A browser global is required for an operation property.".into());
        };
        if object != handle
            || self.operation_globals.contains(name)
            || self.environment(self.global_environment).bindings.contains_key(name)
            || self.objects[properties].properties.contains_key(name)
        {
            return Err("Operation global conflicts with an existing binding or receiver.".into());
        }
        let Value::Function(index) = self.host_function(object, method)? else {
            unreachable!("host_function always allocates a function");
        };
        self.store_property(
            self.function_properties[index],
            "length".into(),
            Value::Number(length as f64),
            Some((false, false, true)),
        )
        .map_err(Fault::display)?;
        // No environment binding and no host_globals entry. A saved function
        // retains its capability even when the public property is removed.
        self.store_property(
            properties,
            name.into(),
            Value::Function(index),
            Some((true, true, true)),
        )
        .map_err(Fault::display)?;
        self.operation_globals.insert(name);
        Ok(())
    }

    pub(super) fn is_operation_global(&self, name: &str) -> bool {
        self.operation_globals.contains(name)
    }

    pub(super) fn check_operation_lexical(&self, name: &str) -> Result<(), Fault> {
        if !self.is_operation_global(name) {
            return Ok(());
        }
        let (_, properties) = self.browser_global.unwrap();
        // HasRestrictedGlobalProperty checks the current own descriptor, not
        // whether a var previously mentioned an already configurable property.
        if self.objects[properties]
            .properties
            .get(name)
            .is_some_and(|property| !property.configurable)
        {
            return Err(Fault::exception(
                "SyntaxError",
                format!("Lexical binding conflicts with global property {name:?}."),
            ));
        }
        Ok(())
    }

    pub(super) fn check_operation_declaration(
        &self,
        name: &str,
        function: bool,
    ) -> Result<(), Fault> {
        if !self.is_operation_global(name) {
            return Ok(());
        }
        let (_, properties) = self.browser_global.unwrap();
        let allowed = match self.objects[properties].properties.get(name) {
            None => self.objects[properties].extensible,
            Some(property) => {
                !function
                    || property.configurable
                    || property.writable
                        && property.enumerable
                        && property.accessor.is_none()
                        && property.getter.is_none()
            }
        };
        if !allowed {
            return Err(Fault::exception(
                "TypeError",
                format!("Cannot declare global function or variable {name:?}."),
            ));
        }
        Ok(())
    }

    pub(super) fn declare_operation_var(&mut self, name: &str, deletable: bool) -> Result<(), Fault> {
        let (_, properties) = self.browser_global.unwrap();
        // CreateGlobalVarBinding never reads an existing accessor or changes
        // its flags. A later initializer performs ordinary Set instead.
        if !self.objects[properties].properties.contains_key(name) {
            self.store_property(
                properties,
                name.into(),
                Value::Undefined,
                Some((true, true, deletable)),
            )?;
        }
        Ok(())
    }

    pub(super) fn declare_operation_function(
        &mut self,
        name: &str,
        value: Value,
        deletable: bool,
    ) -> Result<(), Fault> {
        let (_, properties) = self.browser_global.unwrap();
        // Complete preflight has checked CanDeclareGlobalFunction. Definition
        // replaces a configurable accessor without calling its getter/setter.
        let configurable = deletable
            && self.objects[properties]
                .properties
                .get(name)
                .is_none_or(|property| property.configurable);
        self.store_property(
            properties,
            name.into(),
            value,
            Some((true, true, configurable)),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Exception, Host};

    // Declaration preflight must not read/write host properties or run a timer.
    struct NoHostEffects;

    impl Host for NoHostEffects {
        fn get(&mut self, _object: u32, _property: &str) -> Result<Value, String> {
            panic!("global declaration preflight invoked a host getter");
        }

        fn set(&mut self, _object: u32, _property: &str, _value: Value) -> Result<(), String> {
            panic!("global declaration preflight invoked a host setter");
        }

        fn call(&mut self, _object: u32, _method: &str, _args: &[Value]) -> Result<Value, String> {
            panic!("global declaration preflight invoked a host function");
        }
    }

    #[test]
    fn operation_function_preflight_keeps_typed_fault_and_atomic_bindings() {
        for (name, length) in [("setTimeout", 1), ("clearTimeout", 0)] {
            let mut runtime = Runtime::for_test();
            runtime.bind_browser_global(1).unwrap();
            runtime.define_global_operation(name, 1, name, length).unwrap();
            let (_, properties) = runtime.browser_global.unwrap();
            let property = runtime.objects[properties].properties.get_mut(name).unwrap();
            property.configurable = false;
            property.writable = false;

            let source = format!("var fresh; function {name}() {{}}");
            let program = runtime.register_program(&source, false).unwrap();
            let bindings = runtime.bindings;
            let functions = runtime.functions.len();
            let mut host = NoHostEffects;
            let error = runtime.prepare(&program, &program.body, 0, true, &mut host).unwrap_err();

            // Inspect the exact top-level preparation path before execute()
            // erases the exception object into its public display string.
            assert!(
                matches!(error.exception.as_deref(), Some(Exception::Error("TypeError"))),
                "{error:?}"
            );
            assert!(!runtime.environment(0).bindings.contains_key("fresh"));
            assert!(!runtime.environment(0).bindings.contains_key(name));
            assert!(!runtime.objects[properties].properties.contains_key("fresh"));
            assert_eq!(runtime.bindings, bindings);
            assert_eq!(runtime.functions.len(), functions);
            assert_eq!(
                error.display(),
                format!("JavaScript error: Cannot declare global function or variable {name:?}.")
            );

            // Restore an eligible descriptor internally to check that rejection
            // left no duplicate binding or partially instantiated function.
            runtime.objects[properties].properties.get_mut(name).unwrap().writable = true;
            runtime.begin_job();
            runtime.prepare(&program, &program.body, 0, true, &mut host).unwrap();
            assert!(runtime.environment(0).bindings.contains_key("fresh"));
            assert!(!runtime.environment(0).bindings.contains_key(name));
            assert!(!runtime.objects[properties].properties[name].configurable);
        }
    }
}
