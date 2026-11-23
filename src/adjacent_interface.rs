//! DOM insert-adjacent operations use a checked destination in the current tree.
use super::*;

pub(crate) const TEXT: &str = "\0Element.insertAdjacentText";
pub(crate) const ELEMENT: &str = "\0Element.insertAdjacentElement";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Position {
    BeforeBegin,
    AfterBegin,
    BeforeEnd,
    AfterEnd,
}

impl Position {
    fn parse(value: &Value) -> Option<Self> {
        let view = match value {
            Value::String(text) => crate::dom_string::DomStringRef::Scalar(text),
            Value::StringUnits(units) => crate::dom_string::DomStringRef::Raw(units),
            _ => return None,
        };
        [
            ("beforebegin", Self::BeforeBegin),
            ("afterbegin", Self::AfterBegin),
            ("beforeend", Self::BeforeEnd),
            ("afterend", Self::AfterEnd),
        ]
        .into_iter()
        .find_map(|(name, pos)| {
            view.code_units()
                .map(|unit| {
                    if (65..=90).contains(&unit) {
                        unit + 32
                    } else {
                        unit
                    }
                })
                .eq(name.bytes().map(u16::from))
                .then_some(pos)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Destination {
    pub parent: u32,
    pub gap: usize,
    pub previous: Option<u32>,
    pub next: Option<u32>,
    pub future_text: Option<u32>,
}

impl Runtime {
    pub(super) fn insert_adjacent(
        &mut self,
        target: u32,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let text = method == TEXT;
        let pos = Position::parse(&args[0]).ok_or_else(|| {
            self.native_dom_exception("SyntaxError", "Invalid adjacent insertion position.")
                .map(Fault::thrown)
                .unwrap_or_else(|error| error)
        })?;
        let Some(dest) = host
            .adjacent_destination(target, pos, text)
            .map_err(|(k, m)| self.adjacent_fault(k, &m))?
        else {
            return Ok(if text { Value::Undefined } else { Value::Null });
        };
        if !text {
            let reference = dest.next.map_or(Value::Null, Value::Host);
            return self.call_host(
                dest.parent,
                "insertBefore",
                &[args[1].clone(), reference],
                host,
            );
        }
        let child = dest
            .future_text
            .ok_or_else(|| Fault::new("Missing prospective Text identity."))?;
        let ticket = self.prepare_host_microtask(host, target, method, args, false)?;
        let mut observation = None;
        let result = (|| {
            if let Some(generation) = host.mutation_tree_generation() {
                let change = NativeChildListChange {
                    target: dest.parent,
                    added: vec![child],
                    removed: Vec::new(),
                    previous: dest.previous,
                    next: dest.next,
                };
                observation =
                    self.prepare_native_child_changes(dest.parent, generation, vec![change], host)?;
            }
            host.commit_adjacent_text(target, pos, dest, &args[1])
                .map_err(|(k, m)| self.adjacent_fault(k, &m))?;
            Ok(Value::Undefined)
        })();
        let record = self.finish_tree_observation(observation, host);
        let finish = self.finish_host_microtask(host, ticket);
        record?;
        finish?;
        result
    }

    pub(super) fn adjacent_fault(&mut self, kind: &'static str, message: &str) -> Fault {
        if matches!(
            kind,
            "HierarchyRequestError" | "NotSupportedError" | "SyntaxError" | "NotFoundError"
        ) {
            self.native_dom_exception(kind, message)
                .map(Fault::thrown)
                .unwrap_or_else(|e| e)
        } else if kind == "TypeError" {
            Fault::exception(kind, message)
        } else {
            Fault::new(message)
        }
    }
}
