//! Assignment syntax applies strictness after the internal Proxy set result.
use super::*;

#[cfg(test)]
#[path = "proxy_dispatch_tests.rs"]
mod tests;

impl Runtime {
    pub(super) fn ordinary_descriptor_owner(
        &self,
        owner: Option<usize>,
    ) -> Result<Option<usize>, Fault> {
        if owner.is_some_and(|id| self.objects[id].proxy.is_some()) {
            return Err(Fault::new(
                "This operation cannot inspect a Proxy through ordinary descriptors.",
            ));
        }
        Ok(owner)
    }

    #[inline(never)]
    pub(super) fn assign_via_proxy(
        &mut self,
        proxy: usize,
        key: PropertyKey,
        value: &Value,
        receiver: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let assigned = self.proxy_set(proxy, key, value, receiver, host)?;
        self.proxy_assignment_result(assigned)
    }

    pub(super) fn proxy_assignment_result(&self, assigned: bool) -> Result<(), Fault> {
        if !assigned && self.strict {
            Err(Fault::exception(
                "TypeError",
                "The Proxy property write was refused.",
            ))
        } else {
            Ok(())
        }
    }
}
