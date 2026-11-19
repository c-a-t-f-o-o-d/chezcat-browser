//! Private ordinary-object Proxy identity. Public algorithms live separately;
//! no author property or visible prototype supplies these checked record edges.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProxyData {
    pub(super) target: usize,
    pub(super) handler: usize,
}

impl Runtime {
    fn ordinary_proxy_endpoint(&self, value: &Value) -> Result<usize, Fault> {
        if matches!(value, Value::Function(_) | Value::Host(_)) {
            return Err(Fault::new("Callable and native Proxy targets or handlers are not implemented."));
        }
        let Value::Object(id) = value else {
            return Err(Fault::exception(
                "TypeError",
                "Proxy requires an object target and handler.",
            ));
        };
        let object = self.object(*id)?;
        // Class instances and prototypes retain ordinary storage. Native data,
        // indexed/exotic objects and nested proxies need separate algorithms.
        if object.proxy.is_some()
            || object.length.is_some()
            || object.date.is_some()
            || object.url.is_some()
            || object.rect.is_some()
            || object.dom_exception.is_some()
            || object.primitive.is_some()
            || object.promise.is_some()
            || object.arguments.is_some()
            || object.binary.is_some()
            || object.collection.is_some()
            || object.iterator.is_some()
            || object.generator.is_some()
            || object.text.is_some()
            || object.regexp.is_some()
            || object.error_name.is_some()
        {
            return Err(Fault::new("Proxy targets and handlers with non-ordinary internal storage are not implemented."));
        }
        Ok(*id)
    }

    #[inline(never)]
    pub(super) fn new_ordinary_proxy(
        &mut self,
        target: &Value,
        handler: &Value,
    ) -> Result<Value, Fault> {
        self.step()?;
        let target = self.ordinary_proxy_endpoint(target)?;
        let handler = self.ordinary_proxy_endpoint(handler)?;
        let bytes = std::mem::size_of::<ProxyData>();
        let retained = self
            .value_bytes
            .checked_add(bytes)
            .filter(|&retained| retained <= self.limits.heap.retained_values)
            .ok_or_else(|| {
                Fault::new(format!(
                    "Retained value limit reached ({} bytes).",
                    self.limits.heap.retained_values
                ))
            })?;
        self.work(bytes)?;
        // Reserve before the object ID or retained record becomes observable.
        self.objects
            .try_reserve(1)
            .map_err(|_| Fault::new("Proxy object identity reservation failed."))?;
        let id = self.new_object(None)?;
        // No callbacks or fallible operation follow publication of this record.
        self.objects[id].prototype = None;
        self.objects[id].proxy = Some(ProxyData { target, handler });
        self.value_bytes = retained;
        Ok(Value::Object(id))
    }
}
