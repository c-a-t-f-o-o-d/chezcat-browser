//! Web IDL callback-interface delivery, separate from ordinary function calls.
use super::*;

impl Runtime {
    /// Native event entry uses the same completion/fatal-state accounting as
    /// invoke(). The host owns the job boundary and microtask checkpoint.
    pub fn invoke_event_listener(
        &mut self,
        callback: Value,
        args: &[Value],
        current_target: Value,
        host: &mut dyn Host,
    ) -> Result<Value, String> {
        self.flush_native_microtask_cancellations(host);
        self.pin_exported_value(&callback);
        self.pin_exported_value(&current_target);
        for value in args {
            self.pin_exported_value(value);
        }
        let result = self.call_event_listener(callback, args, current_target, host);
        self.finish_invocation(result, host)
    }

    /// Synchronous dispatchEvent uses this operation within its current job.
    /// IsCallable callbacks never read an author-defined handleEvent property.
    pub(super) fn call_event_listener(
        &mut self,
        callback: Value,
        args: &[Value],
        current_target: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match callback {
            Value::Undefined | Value::Null => Ok(Value::Undefined),
            Value::Function(_) => self.call_value(callback, args, current_target, host),
            Value::Object(_) | Value::Host(_) => {
                self.call_event_listener_object(callback, args, current_target, host)
            }
            _ => Err(Fault::exception(
                "TypeError",
                "An event listener callback must be a function or object.",
            )),
        }
    }

    #[inline(never)]
    fn call_event_listener_object(
        &mut self,
        callback: Value,
        args: &[Value],
        current_target: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // A getter can remove the listener, mutate the registry and cause
        // function collection. Publish all inputs before any such callback.
        let marker = self.publish_call_inputs(&callback, &current_target, args)?;
        let result = (|| {
            self.step()?;
            if args.len() > self.limits.capacity.arguments {
                return Err(self.argument_limit_fault());
            }
            for value in args {
                if value.bytes() > self.limits.capacity.string_bytes {
                    return Err(self.string_limit_fault());
                }
            }
            if current_target.bytes() > self.limits.capacity.string_bytes {
                return Err(self.string_limit_fault());
            }
            let realm = self.callback_interface_object_realm(&callback, host)?;
            self.with_intrinsic_realm(realm, |runtime| {
                // This is an ordinary effective read, including inherited
                // accessors, supported Proxy traps and checked native access.
                let operation = runtime.read_property(&callback, "handleEvent", host)?;
                runtime.call_value(operation, args, callback.clone(), host)
            })
        })();
        self.restore_transient_marker(marker);
        result
    }

    pub(super) fn callback_interface_object_realm(
        &mut self,
        callback: &Value,
        host: &mut dyn Host,
    ) -> Result<Option<usize>, Fault> {
        match callback {
            Value::Object(id) => Ok(self.object(*id)?.realm),
            Value::Host(handle) => {
                // Origin/owner checks precede switching to the callback's
                // realm; changing the active global must never grant access.
                self.validate_native_access(*handle, host)?;
                let owner = host
                    .native_access_owner(*handle)
                    .or_else(|| host.expando_owner(*handle))
                    .or_else(|| host.instance_ancestry(*handle).map(|value| value.window));
                if let Some(owner) = owner {
                    let caller = self
                        .native_access_caller
                        .unwrap_or_else(|| self.browser_global.map(|(window, _)| window));
                    if caller != Some(owner) {
                        host.validate_window_access(caller, owner)
                            .map_err(Fault::new)?;
                    }
                    self.ensure_window_realm(owner, host)
                } else if let Some(id) = self.host_objects.get(handle).copied() {
                    // Explicit embedding-owned bindings already have a real
                    // backing allocation; do not infer realm from its prototype.
                    Ok(self.object(id)?.realm)
                } else {
                    Err(Fault::new(
                        "Native callback object realm is unavailable.",
                    ))
                }
            }
            _ => unreachable!("object callback was classified before realm lookup"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("unexpected native read")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("unexpected native write")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("unexpected native call")
        }
    }

    #[test]
    fn listener_object_realm_missing_operation_error_uses_creation_realm_after_prototype_change() {
        let mut runtime = Runtime::for_test();
        let realm = runtime.allocate_intrinsic_realm().unwrap();
        let values = runtime
            .with_intrinsic_realm(Some(realm), |runtime| {
                runtime
                    .execute("var listener={};", &mut NoHost)
                    .map_err(Fault::new)?;
                let listener = runtime.binding_value(runtime.global_environment, "listener")?;
                let error_prototype = runtime
                    .execute("TypeError.prototype;", &mut NoHost)
                    .map_err(Fault::new)?;
                let array = runtime.execute("Array;", &mut NoHost).map_err(Fault::new)?;
                Ok((listener, error_prototype, array))
            })
            .unwrap();
        runtime.define_global("foreign", values.0.clone()).unwrap();
        runtime
            .execute(
                "Object.setPrototypeOf(foreign,Object.prototype);",
                &mut NoHost,
            )
            .unwrap();
        let fault = runtime
            .call_event_listener(values.0.clone(), &[], Value::Undefined, &mut NoHost)
            .unwrap_err();
        let Some(Exception::Value(Value::Object(error))) = fault.exception.as_deref() else {
            panic!("cross-realm TypeError was not materialized")
        };
        let Value::Object(prototype) = values.1 else {
            panic!("child TypeError prototype")
        };
        assert!(matches!(runtime.objects[*error].prototype,
            Some(Ordinary::Object(found)) if found == prototype));
        runtime
            .with_intrinsic_realm(Some(realm), |runtime| {
                runtime
                    .execute(
                        "listener.handleEvent=function(){if(this!==listener)throw 'object receiver';return Array;};",
                        &mut NoHost,
                    )
                    .map_err(Fault::new)
            })
            .unwrap();
        let result = runtime
            .invoke_event_listener(values.0, &[], Value::Undefined, &mut NoHost)
            .unwrap();
        assert!(matches!((result, values.2), (Value::Function(a), Value::Function(b)) if a == b));
        assert_eq!(
            (runtime.active_realm, runtime.global_environment),
            (None, 0)
        );
        assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
        assert!(runtime.transient_registration_is_empty());
    }
}
