//! Lazy per-realm AsyncFunction identity and dynamic constructor selection.
//! https://tc39.es/ecma262/#sec-async-function-objects
use super::*;

#[derive(Default)]
pub(super) struct Roots {
    pub(super) function_constructor: Option<usize>,
    prototype: Option<usize>,
    constructor: Option<usize>,
    failed: bool,
}

impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(function_reachability::Edge)) {
        if let Some(id) = self.function_constructor {
            add(function_reachability::Edge::Function(id));
        }
        if let Some(id) = self.prototype {
            add(function_reachability::Edge::Object(id));
        }
        if let Some(id) = self.constructor {
            add(function_reachability::Edge::Function(id));
        }
    }
}

impl Runtime {
    pub(super) fn async_function_prototype(&mut self) -> Result<usize, Fault> {
        if self.async_function_roots.failed {
            return Err(Fault::new(
                "AsyncFunction intrinsics are unavailable after a bounded initialization failure.",
            ));
        }
        if let Some(prototype) = self.async_function_roots.prototype {
            return Ok(prototype);
        }
        // No async intrinsic or native callable is created for ordinary code.
        // Partial initialization is unpublished, and a failed attempt cannot be
        // retried to consume another set of bounded arena identities.
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let prototype = self.new_object(None)?;
            self.objects[prototype].prototype = self.function_prototype.map(Ordinary::Function);
            let Value::Function(constructor) =
                self.native_function(Builtin::AsyncFunction, "AsyncFunction")?
            else {
                unreachable!()
            };
            let properties = self.ordinary_id(Ordinary::Function(constructor))?;
            let function = self
                .async_function_roots
                .function_constructor
                .ok_or_else(|| Fault::new("Canonical Function constructor is unavailable."))?;
            self.objects[properties].prototype = Some(Ordinary::Function(function));
            self.store_property(
                properties,
                "prototype".into(),
                Value::Object(prototype),
                Some((false, false, false)),
            )?;
            self.store_property(
                prototype,
                "constructor".into(),
                Value::Function(constructor),
                Some((false, false, true)),
            )?;
            let tag = self
                .symbol_to_string_tag
                .ok_or_else(|| Fault::new("AsyncFunction tag intrinsic is unavailable."))?;
            self.store_symbol_property(
                prototype,
                tag,
                Value::String("AsyncFunction".into()),
                Some((false, false, true)),
            )?;
            self.async_function_roots.prototype = Some(prototype);
            self.async_function_roots.constructor = Some(constructor);
            Ok(prototype)
        })();
        if result.is_err() {
            self.async_function_roots.failed = true;
        }
        self.restore_transient_marker(marker);
        result
    }

    pub(super) fn async_function_constructor(
        &mut self,
        args: &[Value],
        new_target: Option<usize>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.dynamic_function_constructor(args, true, new_target, host)
    }

    pub(super) fn dynamic_async_function_prototype(
        &mut self,
        new_target: Option<usize>,
        host: &mut dyn Host,
    ) -> Result<Ordinary, Fault> {
        let target = new_target
            .or(self.async_function_roots.constructor)
            .ok_or_else(|| Fault::new("AsyncFunction constructor intrinsic is unavailable."))?;
        let prototype =
            self.get_ordinary_property_with_host(Ordinary::Function(target), "prototype", host)?;
        if let Some(prototype) = Ordinary::from_value(&prototype) {
            return Ok(prototype);
        }
        if matches!(prototype, Value::Host(_)) {
            return Err(Fault::new(
                "Native host object prototypes are not implemented.",
            ));
        }
        let realm = self
            .function_realms
            .get(target)
            .copied()
            .ok_or_else(|| Fault::new("Invalid AsyncFunction new target."))?;
        self.with_intrinsic_realm(realm, |runtime| {
            runtime.async_function_prototype().map(Ordinary::Object)
        })
    }
}

#[cfg(test)]
#[path = "async_function_intrinsics_tests.rs"]
mod tests;
