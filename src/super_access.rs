//! Literal super references retain their base snapshot and actual receiver.
//! Construction keeps the super constructor/newTarget snapshot across args.
use super::*;

impl Runtime {
    pub(super) fn read_super_property(&mut self, base: Option<Ordinary>, receiver: &Value, name: &str, host: &mut dyn Host) -> Result<Value, Fault> {
        self.reject_foreign_window(receiver, host)?;
        let base = base.ok_or_else(|| Fault::exception("TypeError", "A null super base has no properties."))?;
        self.get_ordinary_property_with_receiver(base, name, receiver, host)
    }

    pub(super) fn write_super_property(&mut self, base: Option<Ordinary>, receiver: Value, name: Box<str>, value: Value, host: &mut dyn Host) -> Result<(), Fault> {
        self.reject_foreign_window(&receiver, host)?;
        let base = base.ok_or_else(|| Fault::exception("TypeError", "A null super base has no properties."))?;
        self.assign_super_property(base, name.into_string(), value, receiver, host)
    }

    #[inline(never)]
    #[cfg(test)]
    pub(super) fn eval_super_call(&mut self, program: &Rc<Program>, arguments: &[Argument], environment: usize, this: &Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let target = self.prepare_super_call()?;
        let marker = self.root_marker();
        let result = (|| {
            self.push_transient_value(&target.constructor)?;
            self.push_transient_root(function_reachability::Edge::Function(target.new_target))?;
            self.push_transient_root(function_reachability::Edge::Environment(target.environment))?;
            let args = self.eval_argument_list(program, arguments, environment, this, host)?;
            self.finish_super_call(target, &args, host)
        })();
        self.restore_root_marker(marker);
        result
    }

    #[cfg(test)]
    pub(super) fn eval_super_member_call(&mut self, program: &Rc<Program>, name: &str, arguments: &[Argument], environment: usize, this: &Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let reference = self.super_reference(name)?;
        let marker = self.root_marker();
        let result = (|| {
            let Reference::SuperProperty { base, receiver, .. } = &reference else { unreachable!() };
            if let Some(base) = base { self.push_transient_value(&base.value())?; }
            self.push_transient_value(receiver)?;
            let function = self.read_reference(&reference, host)?;
            self.push_transient_value(&function)?;
            let Reference::SuperProperty { receiver, .. } = reference else { unreachable!() };
            let args = self.eval_argument_list(program, arguments, environment, this, host)?;
            self.call_value(function, &args, receiver, host)
        })();
        self.restore_root_marker(marker);
        result
    }
}
