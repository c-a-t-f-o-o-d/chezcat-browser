//! Additional original array operations. Reads recheck inherited indices and
//! holes; writes use Set/Delete with Throw=true. Callback traversal captures
//! length but observes later element changes, with the existing job limits.

#[cfg(test)]
use super::MAX_PROPERTY_BYTES;
use super::{
    primitive_conversion, property_index, strict_equal, Builtin, Fault, Function, Host,
    Ordinary, PropertyKey, Runtime, Value,
};
use std::cmp::Ordering;

struct FlattenFrame {
    id: usize,
    length: usize,
    next: usize,
    depth: f64,
    map: bool,
    root: super::transient_roots::RootMarker,
}

impl Runtime {
    /// A native collection is not an Array: ArraySpeciesCreate returns a fresh
    /// ordinary Array without reading constructor/species. Length is captured
    /// once; coercion and indexed reads continue to observe the live source.
    pub(super) fn array_slice_host(&mut self, handle:u32, args:&[Value], host:&mut dyn Host) -> Result<Value,Fault> {
        self.reject_foreign_window(&Value::Host(handle),host)?;
        // Host/prototype getters may enter Script while the result is partial.
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let length_value = self.host_read(handle,"length",host)?;
            let number = self.binary_number(&length_value,host)?;
            let number = if number.is_nan() || number<=0.0 {0.0} else {number.floor().min(9_007_199_254_740_991.0)};
            if number > usize::MAX as f64 { return Err(Fault::new("Native slice length exceeds the index representation.")); }
            let length = number as usize;
            let start = self.binary_relative(args.first(),length,0,host)?;
            let end = self.binary_relative(args.get(1),length,length,host)?;
            let output = Value::Object(self.new_object(Some(end.saturating_sub(start)))?);
            for index in start..end {
                self.step()?;
                let name = index.to_string();
                if matches!(self.has_property_value(Value::String(name.clone()),Value::Host(handle),host)?,Value::Bool(true)) {
                    let value = self.host_read(handle,&name,host)?;
                    self.create_data_property(&output,PropertyKey::String((index-start).to_string()),value,host)?;
                }
            }
            Ok(output)
        })();
        self.restore_transient_marker(marker);
        result
    }

    /// Join uses the length captured before separator conversion. Active array
    /// identities span custom conversion callbacks as well as native nesting;
    /// cleanup on every completion lets later independent joins visit again.
    pub(super) fn array_join(
        &mut self,
        id: usize,
        length: usize,
        separator: &str,
        host: &mut dyn Host,
    ) -> Result<String, Fault> {
        self.step()?;
        if self.active_array_joins.contains(&id) {
            return Ok(String::new());
        }
        #[cfg(test)]
        if self.limits.test_call_depth.is_some_and(|limit| self.active_array_joins.len() >= limit) {
            return Err(Fault::new("Array string conversion nesting limit reached in explicit regression profile."));
        }
        self.check_native_call_stack("Array string conversion nesting")?;
        self.active_array_joins.try_reserve(1)
            .map_err(|_| Fault::new("Unable to reserve an active Array join identity."))?;
        self.active_array_joins.push(id);
        let result = (|| {
            #[cfg(test)]
            let zero_shortcut = !self.legacy_array_join_preflight;
            #[cfg(not(test))]
            let zero_shortcut = true;
            // Separator conversion has already completed. With captured zero
            // length there is no index to inspect, including on prototypes.
            if zero_shortcut && length == 0 {
                self.work(0)?;
                return Ok(String::new());
            }
            // Preserve the original hole-only fast path. Prototype indices
            // count too, since even a hole can expose an inherited getter.
            let mut parent = Some(Ordinary::Object(id));
            let mut has_indices = false;
            while let Some(object) = parent {
                self.step()?;
                let object = self.ordinary_id(object)?;
                // A proxy may synthesize indexed values even when every
                // preceding ordinary object has holes. Use live Get below.
                if self.objects[object].proxy.is_some() {
                    has_indices = true;
                    break;
                }
                // A first stored key "0" proves the old scan's first match.
                // No value/getter is read; captured zero length stays on the
                // original path. Prepay the bounded key/node inspection.
                if zero_shortcut && length > 0 && self.objects[object].properties
                    .scalar_iter().next().is_some_and(|(key, _)| key == "0")
                {
                    self.work_as(std::mem::size_of::<usize>() + 1, super::WorkKind::ArrayJoinPreflight)?;
                    has_indices = true;
                    break;
                }
                self.work_as(
                    self.objects[object]
                        .properties
                        .len()
                        .saturating_mul(self.limits.capacity.property_bytes),
                    super::WorkKind::ArrayJoinPreflight,
                )?;
                if self.objects[object]
                    .properties
                    .scalar_keys()
                    .any(|key| property_index(key).is_some_and(|index| index < length))
                {
                    has_indices = true;
                    break;
                }
                parent = self.objects[object].prototype;
            }
            if !has_indices {
                let bytes = length.saturating_sub(1).saturating_mul(separator.len());
                if bytes > self.limits.capacity.string_bytes {
                    return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
                }
                self.work(bytes)?;
                return Ok(separator.repeat(length.saturating_sub(1)));
            }
            let mut output = String::new();
            for index in 0..length {
                if index != 0 {
                    self.append_string(&mut output, separator)?;
                }
                let value = self.get_property_with_host(id, &index.to_string(), host)?;
                let text = self.array_join_element(value, host)?;
                self.append_string(&mut output, &text)?;
            }
            Ok(output)
        })();
        self.active_array_joins.pop();
        result
    }

    fn array_join_element(&mut self, value: Value, host: &mut dyn Host) -> Result<String, Fault> {
        if matches!(value, Value::Undefined | Value::Null) {
            return Ok(String::new());
        }
        if matches!(value, Value::Host(_)) {
            // Actual-array join has never granted native conversion. Ordinary
            // callback support does not expand the document's host permission.
            return Err(Fault::new(
                "Array join cannot implicitly convert host objects.",
            ));
        }
        let array = match value {
            Value::Object(id) if self.object(id)?.length.is_some() => Some(id),
            _ => None,
        };
        let Some(id) = array else {
            return self.primitive_with_hint(value, true, host)?.script_text();
        };
        if let Some(primitive) =
            self.primitive_exotic(value.clone(), primitive_conversion::Hint::String, host)?
        {
            return primitive.script_text();
        }
        // OrdinaryToPrimitive's string-hint order, retaining the captured
        // callable and reading valueOf only if toString did not yield a
        // primitive. The genuine native Array toString/join pair recurses
        // directly, checking the actual stack at each array without spending
        // several script-call frames for each plain nested array.
        for name in ["toString", "valueOf"] {
            let method = self.get_property_with_host(id, name, host)?;
            let Value::Function(function) = method else {
                continue;
            };
            let primitive = if matches!(
                self.functions.get(function),
                Some(Function::Native(Builtin::ArrayMethod("toString")))
            ) {
                let join = self.get_property_with_host(id, "join", host)?;
                if matches!(join, Value::Function(join) if matches!(self.functions.get(join),
                    Some(Function::Native(Builtin::ArrayMethod("join")))))
                {
                    let length = self.array_like_length(id, host)?;
                    Value::String(self.array_join(id, length, ",", host)?)
                } else if matches!(join, Value::Function(_)) {
                    self.call_value(join, &[], value.clone(), host)?
                } else {
                    self.object_method("toString", value.clone(), &[], host)?
                }
            } else {
                self.call_value(Value::Function(function), &[], value.clone(), host)?
            };
            if !matches!(
                primitive,
                Value::Object(_) | Value::Function(_) | Value::Host(_)
            ) {
                return self.checked_value(primitive)?.script_text();
            }
        }
        Err(Fault::exception(
            "TypeError",
            "Cannot convert the object to a primitive value.",
        ))
    }

    fn flatten_target(&mut self, source: usize, host: &mut dyn Host) -> Result<Value, Fault> {
        if self.object(source)?.length.is_some() {
            let mut constructor = self.get_property_with_host(source, "constructor", host)?;
            if matches!(constructor, Value::Host(_)) {
                return Err(Fault::new(
                    "Native array species constructors are not supported.",
                ));
            }
            if matches!(constructor, Value::Object(_) | Value::Function(_)) {
                constructor =
                    self.get_symbol_property(constructor, self.symbol_species.unwrap(), host)?;
                if matches!(constructor, Value::Null) {
                    constructor = Value::Undefined;
                }
            }
            if !matches!(constructor, Value::Undefined) {
                if !self.from_constructor(&constructor)? {
                    return Err(Fault::exception(
                        "TypeError",
                        "Array species must be a constructor.",
                    ));
                }
                let target = self.construct_value(constructor, &[Value::Number(0.0)], host)?;
                self.ordinary_required(&target)?;
                return Ok(target);
            }
        }
        Ok(Value::Object(self.new_object(Some(0))?))
    }

    // Root registrations use the existing shared root storage. Pay its actual
    // backing growth, movement and new identity writes before author reentry.
    fn flat_root(&mut self, value: &Value) -> Result<(), Fault> {
        let before = self.root_storage_bytes();
        let length = self.transient_roots.roots.len();
        self.push_transient_value(value)?;
        let added = self
            .root_storage_bytes()
            .checked_sub(before)
            .ok_or_else(|| Fault::new("Array flatten root storage arithmetic failed."))?;
        let moved = if added != 0 {
            length.checked_mul(std::mem::size_of::<super::function_reachability::Edge>())
        } else {
            Some(0)
        }
        .ok_or_else(|| Fault::new("Array flatten root movement overflow."))?;
        let writes = self
            .transient_roots
            .roots
            .len()
            .checked_sub(length)
            .and_then(|n| n.checked_mul(std::mem::size_of::<super::function_reachability::Edge>()))
            .ok_or_else(|| Fault::new("Array flatten root write overflow."))?;
        self.work(
            added
                .checked_add(moved)
                .and_then(|n| n.checked_add(writes))
                .ok_or_else(|| Fault::new("Array flatten root work overflow."))?,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn push_flat_frame(
        &mut self,
        frames: &mut Vec<FlattenFrame>,
        owned: &mut usize,
        id: usize,
        length: usize,
        depth: f64,
        map: bool,
    ) -> Result<(), Fault> {
        let root = self.root_marker();
        self.flat_root(&Value::Object(id))?;
        let available = self
            .limits
            .heap
            .retained_values
            .checked_sub(self.value_bytes)
            .ok_or_else(|| Fault::new("Array flatten retained-value limit reached."))?;
        let growth = super::limits::prepare_retained_growth(frames, 1, available)?;
        let movement = if growth.added != 0 {
            frames
                .len()
                .checked_mul(std::mem::size_of::<FlattenFrame>())
        } else {
            Some(0)
        }
        .ok_or_else(|| Fault::new("Array flatten frame movement overflow."))?;
        self.work(
            growth
                .added
                .checked_add(movement)
                .and_then(|n| n.checked_add(std::mem::size_of::<FlattenFrame>()))
                .ok_or_else(|| Fault::new("Array flatten frame work overflow."))?,
        )?;
        let next = self
            .value_bytes
            .checked_add(growth.added)
            .ok_or_else(|| Fault::new("Array flatten retained storage overflow."))?;
        let active = self
            .array_flat_workspace_bytes
            .checked_add(growth.added)
            .ok_or_else(|| Fault::new("Array flatten workspace overflow."))?;
        let owned_next = owned
            .checked_add(growth.added)
            .ok_or_else(|| Fault::new("Array flatten owned storage overflow."))?;
        growth.commit(frames);
        self.value_bytes = next;
        self.array_flat_workspace_bytes = active;
        *owned = owned_next;
        frames.push(FlattenFrame {
            id,
            length,
            next: 0,
            depth,
            map,
            root,
        });
        Ok(())
    }
    pub(super) fn array_flat(
        &mut self,
        source: Value,
        map: bool,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let marker = self.root_marker();
        let mut frames = Vec::new();
        let mut owned = 0usize;
        let result = (|| {
            // These handles remain live through length/depth/species getters,
            // mapper callbacks and property creation, including nested flat.
            self.flat_root(&source)?;
            for value in args.iter().take(if map { 2 } else { 1 }) {
                self.flat_root(value)?;
            }
            let ordinary = self.ordinary_required(&source)?;
            let id = self.ordinary_id(ordinary)?;
            let length = self.array_like_length(id, host)?;
            let callback = args.first().unwrap_or(&Value::Undefined);
            if map && !matches!(callback, Value::Function(_)) {
                return Err(Fault::exception(
                    "TypeError",
                    "Array flatMap callback must be callable.",
                ));
            }
            let depth = if map || matches!(args.first(), None | Some(Value::Undefined)) {
                1.
            } else {
                self.binary_integer(args.first(), 0., host)?.max(0.)
            };
            let target = self.flatten_target(id, host)?;
            if length == 0 {
                return Ok(target);
            }
            self.flat_root(&target)?;
            let receiver = args.get(1).unwrap_or(&Value::Undefined);
            self.push_flat_frame(&mut frames, &mut owned, id, length, depth, map)?;
            let mut output = 0usize;
            while let Some(frame) = frames.last_mut() {
                self.step()?;
                if frame.next == frame.length {
                    let root = frame.root;
                    frames.pop();
                    self.restore_root_marker(root);
                    continue;
                }
                let (id, index, depth, map) = (frame.id, frame.next, frame.depth, frame.map);
                frame.next += 1;
                if !self.array_has_index(id, index)? {
                    continue;
                }
                let mut value = self.array_get_index(id, index, host)?;
                if map {
                    let receiver = self.copy_value(receiver)?;
                    value = self.call_value(
                        callback.clone(),
                        &[value, Value::Number(index as f64), source.clone()],
                        receiver,
                        host,
                    )?;
                }
                if let Value::Object(child) = value {
                    if depth > 0. {
                        if let Some(length) = self.object(child)?.length {
                            // No index can be visited in an empty child. In particular,
                            // this needs neither a depth guard nor an extra work frame.
                            if length != 0 {
                                self.push_flat_frame(
                                    &mut frames,
                                    &mut owned,
                                    child,
                                    length,
                                    depth - 1.,
                                    false,
                                )?;
                            }
                            continue;
                        }
                    }
                }
                if output == self.limits.heap.array_length {
                    return Err(Fault::new(format!(
                        "Array flatten length limit reached ({}).",
                        self.limits.heap.array_length
                    )));
                }
                let value_marker = self.root_marker();
                self.flat_root(&value)?;
                let created = self.create_data_property(
                    &target,
                    PropertyKey::String(output.to_string()),
                    value,
                    host,
                );
                self.restore_root_marker(value_marker);
                created?;
                output = output
                    .checked_add(1)
                    .ok_or_else(|| Fault::new("Array flatten output index overflow."))?;
            }
            Ok(target)
        })();
        // Only this invocation's actual frame backing is refunded. Callback
        // allocations and observable partial species writes remain intact.
        drop(frames);
        let refund = self
            .value_bytes
            .checked_sub(owned)
            .zip(self.array_flat_workspace_bytes.checked_sub(owned));
        self.restore_root_marker(marker);
        if let Some((values, active)) = refund {
            self.value_bytes = values;
            self.array_flat_workspace_bytes = active;
        } else {
            return Err(Fault::new(
                "Array flatten workspace refund accounting failed.",
            ));
        }
        result
    }

    pub(super) fn array_like_length(
        &mut self,
        id: usize,
        host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        if let Some(length) = self.object(id)?.length {
            return Ok(length);
        }
        let length = self.binary_source_length(&Value::Object(id), host)?;
        if length > self.limits.heap.array_length {
            return Err(Fault::new(format!("Array-like length limit reached ({}).", self.limits.heap.array_length)));
        }
        Ok(length)
    }
    pub(super) fn array_like_join(
        &mut self,
        id: usize,
        length: usize,
        separator: &str,
        host: &mut dyn Host,
    ) -> Result<String, Fault> {
        let mut output = String::new();
        for index in 0..length {
            self.step()?;
            if index != 0 {
                self.append_string(&mut output, separator)?;
            }
            let value = self.get_property_with_host(id, &index.to_string(), host)?;
            if !matches!(value, Value::Null | Value::Undefined) {
                let text = self.primitive_with_hint(value, true, host)?.script_text()?;
                self.append_string(&mut output, &text)?;
            }
        }
        Ok(output)
    }
    fn array_has_index(&mut self, id: usize, index: usize) -> Result<bool, Fault> {
        Ok(self
            .property_owner(Ordinary::Object(id), &index.to_string())?
            .is_some())
    }
    fn array_get_index(
        &mut self,
        id: usize,
        index: usize,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.get_property_with_host(id, &index.to_string(), host)
    }
    fn array_put_index(
        &mut self,
        id: usize,
        index: usize,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let previous = self.strict;
        self.strict = true;
        let result =
            self.assign_ordinary_property(Ordinary::Object(id), index.to_string(), value, host);
        self.strict = previous;
        result
    }
    pub(super) fn array_delete_index(&mut self, id: usize, index: usize) -> Result<(), Fault> {
        self.reject_unsupported_proxy_operation(id, "deleteProperty")?;
        let key = index.to_string();
        if self.typed_view(id).is_some_and(|view| index < view.length) {
            return Err(Fault::exception(
                "TypeError",
                "Cannot delete a typed-array element.",
            ));
        }
        if self.objects[id]
            .properties
            .get(&key)
            .is_some_and(|property| !property.configurable)
        {
            return Err(Fault::exception(
                "TypeError",
                "Cannot delete a nonconfigurable array element.",
            ));
        }
        self.remove_property(id, &key)
    }
    pub(super) fn array_length_throw(
        &mut self,
        id: usize,
        length: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let previous = self.strict;
        self.strict = true;
        let result = self.assign_ordinary_property(
            Ordinary::Object(id),
            "length".into(),
            Value::Number(length as f64),
            host,
        );
        self.strict = previous;
        result
    }
    fn array_move_index(
        &mut self,
        id: usize,
        source: usize,
        target: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if self.array_has_index(id, source)? {
            let value = self.array_get_index(id, source, host)?;
            self.array_put_index(id, target, value, host)
        } else {
            self.array_delete_index(id, target)
        }
    }
    pub(super) fn extra_array_method(
        &mut self,
        id: usize,
        length: usize,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match method {
            "shift" => {
                let first = if length == 0 {
                    Value::Undefined
                } else {
                    self.array_get_index(id, 0, host)?
                };
                for index in 1..length {
                    self.array_move_index(id, index, index - 1, host)?;
                }
                if length > 0 {
                    self.array_delete_index(id, length - 1)?;
                }
                self.array_length_throw(id, length.saturating_sub(1), host)?;
                Ok(first)
            }
            "unshift" => {
                if length.saturating_add(args.len()) > self.limits.heap.array_length {
                    return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                }
                if !args.is_empty() {
                    for index in (0..length).rev() {
                        self.array_move_index(id, index, index + args.len(), host)?;
                    }
                    for (index, value) in args.iter().enumerate() {
                        let value = self.copy_value(value)?;
                        self.array_put_index(id, index, value, host)?;
                    }
                }
                self.array_length_throw(id, length + args.len(), host)?;
                Ok(Value::Number((length + args.len()) as f64))
            }
            "splice" | "toSpliced" => {
                self.array_splice(id, length, method == "toSpliced", args, host)
            }
            "at" => {
                let index = self.binary_integer(args.first(), 0.0, host)?;
                let index = if index < 0.0 {
                    length as f64 + index
                } else {
                    index
                };
                if index < 0.0 || index >= length as f64 {
                    Ok(Value::Undefined)
                } else {
                    self.array_get_index(id, index as usize, host)
                }
            }
            "lastIndexOf" => {
                if length == 0 {
                    return Ok(Value::Number(-1.0));
                }
                let needle = args.first().unwrap_or(&Value::Undefined);
                let index = if args.len() < 2 {
                    length as f64 - 1.0
                } else {
                    self.binary_integer(args.get(1), 0.0, host)?
                };
                let index = if index < 0.0 {
                    length as f64 + index
                } else {
                    index
                };
                if index >= 0.0 {
                    for index in (0..=index.min(length as f64 - 1.0) as usize).rev() {
                        if self.array_has_index(id, index)? {
                            let value = self.array_get_index(id, index, host)?;
                            self.work(value.bytes().saturating_add(needle.bytes()))?;
                            if strict_equal(needle, &value) {
                                return Ok(Value::Number(index as f64));
                            }
                        }
                    }
                }
                Ok(Value::Number(-1.0))
            }
            "fill" => {
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                let end = self.binary_relative(args.get(2), length, length, host)?;
                for index in start..end {
                    let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                    self.array_put_index(id, index, value, host)?;
                }
                Ok(Value::Object(id))
            }
            "copyWithin" => {
                let target = self.binary_relative(args.first(), length, 0, host)?;
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                let end = self.binary_relative(args.get(2), length, length, host)?;
                let count = end.saturating_sub(start).min(length - target);
                for next in 0..count {
                    let step = if start < target && target < start + count {
                        count - 1 - next
                    } else {
                        next
                    };
                    self.array_move_index(id, start + step, target + step, host)?;
                }
                Ok(Value::Object(id))
            }
            "reverse" => {
                for left in 0..length / 2 {
                    let right = length - 1 - left;
                    let has_left = self.array_has_index(id, left)?;
                    let left_value = if has_left {
                        Some(self.array_get_index(id, left, host)?)
                    } else {
                        None
                    };
                    let has_right = self.array_has_index(id, right)?;
                    let right_value = if has_right {
                        Some(self.array_get_index(id, right, host)?)
                    } else {
                        None
                    };
                    match (left_value, right_value) {
                        (Some(a), Some(b)) => {
                            self.array_put_index(id, left, b, host)?;
                            self.array_put_index(id, right, a, host)?;
                        }
                        (None, Some(b)) => {
                            self.array_put_index(id, left, b, host)?;
                            self.array_delete_index(id, right)?;
                        }
                        (Some(a), None) => {
                            self.array_delete_index(id, left)?;
                            self.array_put_index(id, right, a, host)?;
                        }
                        (None, None) => {}
                    }
                }
                Ok(Value::Object(id))
            }
            "toReversed" | "with" => {
                let replacement = if method == "with" {
                    let index = self.binary_integer(args.first(), 0.0, host)?;
                    let index = if index < 0.0 {
                        length as f64 + index
                    } else {
                        index
                    };
                    if index < 0.0 || index >= length as f64 {
                        return Err(Fault::exception(
                            "RangeError",
                            "Array with index is out of range.",
                        ));
                    }
                    Some(index as usize)
                } else {
                    None
                };
                let result = self.new_object(Some(length))?;
                for index in 0..length {
                    let value = if replacement == Some(index) {
                        self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?
                    } else {
                        self.array_get_index(
                            id,
                            if method == "toReversed" {
                                length - 1 - index
                            } else {
                                index
                            },
                            host,
                        )?
                    };
                    self.set_property(result, index.to_string(), value)?;
                }
                Ok(Value::Object(result))
            }
            "concat" => {
                let result = self.new_object(Some(0))?;
                let mut at = 0;
                for source in std::iter::once(&Value::Object(id)).chain(args) {
                    if let Value::Object(source) = source {
                        if let Some(length) = self.object(*source)?.length {
                            if at + length > self.limits.heap.array_length {
                                return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                            }
                            for index in 0..length {
                                if self.array_has_index(*source, index)? {
                                    let value = self.array_get_index(*source, index, host)?;
                                    self.set_property(result, (at + index).to_string(), value)?;
                                }
                            }
                            at += length;
                            continue;
                        }
                    }
                    if at == self.limits.heap.array_length {
                        return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                    }
                    let value = self.copy_value(source)?;
                    self.set_property(result, at.to_string(), value)?;
                    at += 1;
                }
                self.set_array_length(result, at)?;
                Ok(Value::Object(result))
            }
            "sort" | "toSorted" => {
                let comparator = args
                    .first()
                    .filter(|value| !matches!(value, Value::Undefined));
                if comparator.is_some_and(|value| !matches!(value, Value::Function(_))) {
                    return Err(Fault::exception(
                        "TypeError",
                        "Array comparator must be callable.",
                    ));
                }
                let mut values = Vec::new();
                for index in 0..length {
                    if method == "toSorted" || self.array_has_index(id, index)? {
                        let value = self.array_get_index(id, index, host)?;
                        self.work(8usize.saturating_add(value.bytes()))?;
                        values.push(value);
                    }
                }
                self.sort_native_values(&mut values, comparator, false, host)?;
                let target = if method == "toSorted" {
                    self.new_object(Some(length))?
                } else {
                    id
                };
                let count = values.len();
                for (index, value) in values.into_iter().enumerate() {
                    self.array_put_index(target, index, value, host)?;
                }
                for index in count..length {
                    self.array_delete_index(target, index)?;
                }
                Ok(Value::Object(target))
            }
            "map" | "filter" | "forEach" | "some" | "every" | "find" | "findIndex" | "findLast"
            | "findLastIndex" | "reduce" | "reduceRight" => {
                self.array_callback(id, length, method, args, host)
            }
            _ => Err(Fault::new("Unsupported array method.")),
        }
    }
    fn array_splice(
        &mut self,
        id: usize,
        length: usize,
        copy: bool,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let start = self.binary_relative(args.first(), length, 0, host)?;
        let deleted = if args.is_empty() {
            0
        } else if args.len() == 1 {
            length - start
        } else {
            self.binary_integer(args.get(1), 0.0, host)?
                .max(0.0)
                .min((length - start) as f64) as usize
        };
        let items = args.get(2..).unwrap_or(&[]);
        let added = items.len();
        let new_length = length - deleted + added;
        if new_length > self.limits.heap.array_length {
            return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
        }
        if copy {
            let result = self.new_object(Some(new_length))?;
            for index in 0..new_length {
                let value = if index >= start && index < start + added {
                    self.copy_value(&items[index - start])?
                } else {
                    self.array_get_index(
                        id,
                        if index < start {
                            index
                        } else {
                            index + deleted - added
                        },
                        host,
                    )?
                };
                self.set_property(result, index.to_string(), value)?;
            }
            return Ok(Value::Object(result));
        }
        let result = self.new_object(Some(deleted))?;
        for index in 0..deleted {
            if self.array_has_index(id, start + index)? {
                let value = self.array_get_index(id, start + index, host)?;
                self.set_property(result, index.to_string(), value)?;
            }
        }
        if added < deleted {
            for index in start..length - deleted {
                self.array_move_index(id, index + deleted, index + added, host)?;
            }
            for index in (new_length..length).rev() {
                self.array_delete_index(id, index)?;
            }
        } else if added > deleted {
            for index in (start..length - deleted).rev() {
                self.array_move_index(id, index + deleted, index + added, host)?;
            }
        }
        for (index, value) in items.iter().enumerate() {
            let value = self.copy_value(value)?;
            self.array_put_index(id, start + index, value, host)?;
        }
        self.array_length_throw(id, new_length, host)?;
        Ok(Value::Object(result))
    }
    fn array_callback(
        &mut self,
        id: usize,
        length: usize,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let callback = args.first().unwrap_or(&Value::Undefined);
        if !matches!(callback, Value::Function(_)) {
            return Err(Fault::exception(
                "TypeError",
                "Array callback must be callable.",
            ));
        }
        let reduce = matches!(method, "reduce" | "reduceRight");
        let reverse = matches!(method, "findLast" | "findLastIndex" | "reduceRight");
        let visit_holes = matches!(method, "find" | "findIndex" | "findLast" | "findLastIndex");
        let result = if method == "map" {
            Some(self.new_object(Some(length))?)
        } else if method == "filter" {
            Some(self.new_object(Some(0))?)
        } else {
            None
        };
        let receiver = if reduce {
            Value::Undefined
        } else {
            self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?
        };
        let mut accumulator = if reduce { args.get(1).cloned() } else { None };
        let mut kept = 0;
        for next in 0..length {
            let index = if reverse { length - 1 - next } else { next };
            if !visit_holes && !self.array_has_index(id, index)? {
                continue;
            }
            let value = self.array_get_index(id, index, host)?;
            if reduce && accumulator.is_none() {
                accumulator = Some(value);
                continue;
            }
            let values = if reduce {
                vec![
                    self.copy_value(accumulator.as_ref().unwrap())?,
                    self.copy_value(&value)?,
                    Value::Number(index as f64),
                    Value::Object(id),
                ]
            } else {
                vec![
                    self.copy_value(&value)?,
                    Value::Number(index as f64),
                    Value::Object(id),
                ]
            };
            let output = self.call_value(callback.clone(), &values, receiver.clone(), host)?;
            match method {
                "map" => self.set_property(result.unwrap(), index.to_string(), output)?,
                "filter" if output.truthy() => {
                    self.set_property(result.unwrap(), kept.to_string(), value)?;
                    kept += 1;
                }
                "some" if output.truthy() => return Ok(Value::Bool(true)),
                "every" if !output.truthy() => return Ok(Value::Bool(false)),
                "find" | "findLast" if output.truthy() => return Ok(value),
                "findIndex" | "findLastIndex" if output.truthy() => {
                    return Ok(Value::Number(index as f64))
                }
                "reduce" | "reduceRight" => accumulator = Some(output),
                _ => {}
            }
        }
        Ok(match method {
            "map" | "filter" => Value::Object(result.unwrap()),
            "reduce" | "reduceRight" => accumulator.ok_or_else(|| {
                Fault::exception(
                    "TypeError",
                    "Cannot reduce an empty array without an initial value.",
                )
            })?,
            "every" => Value::Bool(true),
            "some" => Value::Bool(false),
            "findIndex" | "findLastIndex" => Value::Number(-1.0),
            _ => Value::Undefined,
        })
    }
    pub(super) fn sort_native_values(
        &mut self,
        values: &mut [Value],
        comparator: Option<&Value>,
        numeric: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        // Iterative stable merge sort permits fallible script comparators and
        // keeps native stack depth constant. Every temporary copy is charged.
        let mut width = 1usize;
        while width < values.len() {
            self.work(values.len().saturating_mul(std::mem::size_of::<Value>()))?;
            let mut merged = Vec::with_capacity(values.len());
            for start in (0..values.len()).step_by(width * 2) {
                let middle = (start + width).min(values.len());
                let end = (middle + width).min(values.len());
                let (mut left, mut right) = (start, middle);
                while left < middle || right < end {
                    let take_left = right == end
                        || left < middle
                            && self.compare_array_values(
                                &values[left],
                                &values[right],
                                comparator,
                                numeric,
                                host,
                            )? != Ordering::Greater;
                    let index = if take_left {
                        let index = left;
                        left += 1;
                        index
                    } else {
                        let index = right;
                        right += 1;
                        index
                    };
                    merged.push(self.copy_value(&values[index])?);
                }
            }
            for (destination, value) in values.iter_mut().zip(merged) {
                *destination = value;
            }
            width *= 2;
        }
        Ok(())
    }
    fn compare_array_values(
        &mut self,
        a: &Value,
        b: &Value,
        comparator: Option<&Value>,
        numeric: bool,
        host: &mut dyn Host,
    ) -> Result<Ordering, Fault> {
        self.step()?;
        if !numeric {
            if matches!(a, Value::Undefined) {
                return Ok(if matches!(b, Value::Undefined) {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                });
            }
            if matches!(b, Value::Undefined) {
                return Ok(Ordering::Less);
            }
        }
        if let Some(comparator) = comparator {
            let a = self.copy_value(a)?;
            let b = self.copy_value(b)?;
            let value = self.call_value(comparator.clone(), &[a, b], Value::Undefined, host)?;
            let number = self.binary_number(&value, host)?;
            return Ok(if number.is_nan() || number == 0.0 {
                Ordering::Equal
            } else if number < 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        if numeric {
            let (Value::Number(a), Value::Number(b)) = (a, b) else {
                return Err(Fault::new("Invalid numeric sort values."));
            };
            return Ok(if a.is_nan() {
                if b.is_nan() {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            } else if b.is_nan() {
                Ordering::Less
            } else if *a == 0.0 && *b == 0.0 {
                b.is_sign_negative().cmp(&a.is_sign_negative())
            } else {
                a.partial_cmp(b).unwrap()
            });
        }
        let a = self.copy_value(a)?;
        let a = self.primitive_with_hint(a, true, host)?;
        self.work(a.bytes())?;
        // Complete ToString on the left before invoking conversion callbacks
        // on the right. A Symbol result is an abrupt completion here.
        let a = a.script_text()?;
        let b = self.copy_value(b)?;
        let b = self.primitive_with_hint(b, true, host)?;
        self.work(b.bytes())?;
        let b = b.script_text()?;
        self.work(a.len().saturating_add(b.len()).saturating_mul(2))?;
        Ok(a.encode_utf16().cmp(b.encode_utf16()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SliceHost {
        length:Value,
        fail_has:bool,
        fail_get:bool,
        log:Vec<String>,
    }
    impl Default for SliceHost {
        fn default()->Self {Self {length:Value::Number(3.0),fail_has:false,fail_get:false,log:Vec::new()}}
    }
    impl Host for SliceHost {
        fn get(&mut self,_:u32,name:&str)->Result<Value,String> {
            self.log.push(match name {"length"=>"L".into(),"start"=>"S".into(),"end"=>"E".into(),_=>format!("G{name}")});
            match name {
                "length"=>Ok(self.length.clone()),"start"=>Ok(Value::Number(0.0)),"end"=>Ok(Value::Number(3.0)),
                "0"=>Ok(Value::Undefined),"2" if !self.fail_get=>Ok(Value::String("third".into())),
                _=>Err("Host indexed read denied".into()),
            }
        }
        fn has(&mut self,_:u32,name:&str)->Result<bool,String> {
            self.log.push(format!("H{name}"));
            if name=="1" && self.fail_has {Err("Host indexed presence denied".into())} else {Ok(matches!(name,"0"|"2"))}
        }
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String> {Err("No host write".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String> {Err("No host call".into())}
    }

    #[test]
    fn host_array_slice_observes_order_holes_and_own_creation_without_species() {
        let mut runtime=Runtime::for_test();
        runtime.define_global("native",Value::Host(1234)).unwrap();
        let mut host=SliceHost::default();
        let value=runtime.execute(r#"
Object.defineProperty(Array.prototype,'0',{get:function(){throw 'inherited read';},set:function(){throw 'inherited setter';},configurable:true});
var result=Array.prototype.slice.call(native,{valueOf:function(){return native.start;}},{valueOf:function(){return native.end;}});
if(result.length!==3||!Object.prototype.hasOwnProperty.call(result,'0')||result[0]!==undefined||1 in result||result[2]!=='third'||Object.getPrototypeOf(result)!==Array.prototype)throw 'shape';
var desc=Object.getOwnPropertyDescriptor(result,'0');if(!desc.writable||!desc.enumerable||!desc.configurable)throw 'own data flags';delete Array.prototype[0];'ok';
"#,&mut host).unwrap();
        assert_eq!(value.to_text(),"ok");
        assert_eq!(host.log,["L","S","E","H0","G0","H1","H2","G2"]);
    }

    #[test]
    fn host_array_slice_keeps_host_denials_array_capacity_and_recovery() {
        for fail_get in [false,true] {
            let mut runtime=Runtime::for_test();
            runtime.define_global("native",Value::Host(1234)).unwrap();
            let mut host=SliceHost {fail_has:!fail_get,fail_get,..SliceHost::default()};
            let error=runtime.execute("Array.prototype.slice.call(native);",&mut host).unwrap_err();
            assert!(error.contains("Host indexed"),"{error}");
            runtime.begin_job();host.fail_has=false;host.fail_get=false;
            assert_eq!(runtime.execute("Array.prototype.slice.call(native).length;",&mut host).unwrap().to_number(),3.0);
        }
        let mut runtime=Runtime::for_test();
        runtime.define_global("native",Value::Host(1234)).unwrap();
        let mut host=SliceHost {length:Value::Number(1_000_000_000.0),..SliceHost::default()};
        let error=runtime.execute("Array.prototype.slice.call(native);",&mut host).unwrap_err();
        assert!(error.contains("limit"),"{error}");
        assert_eq!(host.log,["L"]);
        runtime.begin_job();host.log.clear();host.length=Value::Number(f64::INFINITY);
        assert_eq!(runtime.execute("Array.prototype.slice.call(native,Infinity).length;",&mut host).unwrap().to_number(),0.0);
        assert_eq!(host.log,["L"]);
    }

    fn join_succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn actual_array_join_observes_lazy_conversion_and_callable_values() {
        join_succeeds(
            r#"
var order='',o={get toString(){order+='g';return function(){order+='s';return {};};},get valueOf(){order+='v';return function(){order+='V';return 12;};}};
if([o].join()!=='12'||order!=='gsvV')throw 'ordinary conversion order';
var first={toString:function(){Object.defineProperty(this,'valueOf',{get:function(){throw 'unneeded';}});return 'first';}};
if([first].join()!=='first')throw 'lazy fallback';
var child=Object.create({toString:function(){return this.name;}});child.name='inherited';
function f(){}f.toString=function(){return this===f?'function':'bad receiver';};
var bound=f.bind(null);bound[Symbol.toPrimitive]=function(h){return h;};
if([child,f,bound,new Number(3),new Boolean(false)].join('|')!=='inherited|function|string|3|false')throw 'ordinary callable values';
var caught=0;for(var bad of [{toString:function(){return {};},valueOf:function(){return {}; }},{toString:function(){return Symbol();}}]){
try{[bad].join();}catch(e){if(e.name==='TypeError')caught++;}}
if(caught!==2)throw 'catchable conversion';
'ok';
"#,
        );
    }

    #[test]
    fn zero_index_join_original_dense_reducer_preserves_output_and_reduces_preflight() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut observations = Vec::new();
            for legacy in [true, false] {
                let mut runtime = Runtime::for_capacity_test();
                runtime.legacy_array_join_preflight = legacy;
                let values: Vec<_> = (0..32).map(|i| Value::Number(i as f64)).collect();
                let Value::Object(array) = runtime.array_from_values(&values).unwrap() else { unreachable!() };
                runtime.begin_job();
                let before = (runtime.steps, runtime.copy_bytes, runtime.job_deadline);
                let expected = (0..32).map(|i| i.to_string()).collect::<String>();
                for _ in 0..1000 { assert_eq!(runtime.array_join(array, 32, "", &mut NoHost).unwrap(), expected); }
                assert!(runtime.active_array_joins.is_empty());
                assert_eq!(runtime.job_deadline, before.2);
                observations.push((before.0 - runtime.steps, before.1 - runtime.copy_bytes));
            }
            assert_eq!(observations[0].0, observations[1].0);
            assert_eq!(observations[0].1 - observations[1].1, (32 * MAX_PROPERTY_BYTES - 9) * 1000);
            println!("Original32element/1000join reducer: legacy={:?}; zero-key={:?}; saved={}bytes; steps/deadline unchanged.", observations[0], observations[1], observations[0].1 - observations[1].1);
        }).unwrap().join().unwrap();
    }

    #[test]
    fn zero_index_join_keeps_live_getters_prototypes_captured_length_and_fallback_keys() {
        join_succeeds(r#"
var log='',a=[1,2,3];Object.defineProperty(a,'0',{get:function(){log+='g';delete a[1];a[2]=9;return {toString:function(){log+='c';return 'first';}};}});
if(a.join('|')!=='first||9'||log!=='gc')throw 'live getter/conversion';
var parent=Object.create(Array.prototype),reads=0;Object.defineProperty(parent,'0',{get:function(){reads++;return 7;}});
var holes=Array(2);Object.setPrototypeOf(holes,parent);if(holes.join('|')!=='7|'||reads!==1)throw 'inherited zero';
var empty=[],touched=0,separator={toString:function(){Object.defineProperty(empty,'0',{get:function(){touched++;return 9;}});return '|';}};
if(empty.join(separator)!==''||touched!==0)throw 'captured zero length';
for(var key of ['', '00', '0.0']){var fallback=[1,2];fallback[key]=3;if(fallback.join('|')!=='1|2')throw 'fallback key';}
var shrinking=[1,2];if(shrinking.join({toString:function(){shrinking.length=1;return '|';}})!=='1|')throw 'captured length';
'ok';
"#);
        for source in ["var a=[];a['']=3;a[0]=1;a[1]=2;", "var a=Array(2);a['00']=3;"] {
            let mut observed = Vec::new();
            for legacy in [true, false] {
                let mut runtime = Runtime::for_test();
                runtime.legacy_array_join_preflight = legacy;
                runtime.execute(source, &mut NoHost).unwrap();
                runtime.begin_job();
                let value = runtime.execute("a.join('|');", &mut NoHost).unwrap().to_text();
                observed.push((value, runtime.job_usage()));
            }
            assert_eq!(observed[0], observed[1], "{source}");
        }
    }

    #[test]
    fn zero_index_join_preflight_failures_clear_cycle_state_and_preserve_arenas() {
        for failure in ["copy", "steps", "deadline"] {
            let mut runtime = Runtime::for_test();
            let Value::Object(array) = runtime.array_from_values(&[Value::Number(1.0)]).unwrap() else { unreachable!() };
            runtime.begin_job();
            match failure {
                "copy" => runtime.copy_bytes = 8,
                "steps" => runtime.steps = 1,
                _ => { runtime.steps = 256; runtime.job_deadline = std::time::Instant::now() - std::time::Duration::from_secs(1); }
            }
            let before = (runtime.functions.len(), runtime.objects.len(), runtime.environments.len(), runtime.value_bytes, runtime.bindings, runtime.job_deadline);
            let error = runtime.array_join(array, 1, "", &mut NoHost).unwrap_err();
            assert!(error.exception.is_none() && error.message.contains("limit"), "{error:?}");
            assert!(runtime.active_array_joins.is_empty());
            assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.environments.len(), runtime.value_bytes, runtime.bindings, runtime.job_deadline), before);
            if failure == "copy" { assert!(runtime.string_work_failure_report().unwrap().contains("category=array-join-preflight; requested=9; remaining-before=8")); }
            runtime.begin_job();
            assert_eq!(runtime.array_join(array, 1, "", &mut NoHost).unwrap(), "1");
        }
    }

    #[test]
    fn zero_length_join_eliminates_prototype_walk_after_separator_conversion() {
        let mut observations = Vec::new();
        for legacy in [true, false] {
            let mut runtime = Runtime::with_limits(super::super::limits::BROWSER);
            runtime.legacy_array_join_preflight = legacy;
            let Value::Object(array) = runtime.array_from_values(&[]).unwrap() else { unreachable!() };
            runtime.begin_job();
            let before = (runtime.steps, runtime.copy_bytes, runtime.job_deadline);
            for _ in 0..1000 { assert_eq!(runtime.array_join(array, 0, "", &mut NoHost).unwrap(), ""); }
            observations.push((before.0 - runtime.steps, before.1 - runtime.copy_bytes));
            assert_eq!(runtime.job_deadline, before.2);
            assert!(runtime.active_array_joins.is_empty());
        }
        assert!(observations[1].0 < observations[0].0);
        assert_eq!(observations[1].1, 0);
        println!("Original1000emptyjoin reducer: legacy={:?}; capturedzero={:?}.", observations[0], observations[1]);
        join_succeeds(r#"
var log='',a=[],parent=Object.create(Array.prototype);Object.defineProperty(parent,'0',{get:function(){throw 'unread';}});Object.setPrototypeOf(a,parent);
var separator={toString:function(){log+='s';return '|';}};
if(a.join(separator)!==''||log!=='s')throw 'separator order';
var marker={};try{a.join({toString:function(){throw marker;}});throw 'missing';}catch(e){if(e!==marker)throw e;}
'ok';
"#);
    }

    #[test]
    fn actual_array_join_captures_length_before_separator_and_reads_live_indices() {
        join_succeeds(
            r#"
var a=[1,2,3],separator={toString:function(){a.length=0;return '|';}};
if(a.join(separator)!=='||')throw 'captured shrinking length';
var b=[1],separator2={toString:function(){b.push(2,3);return ':';}};
if(b.join(separator2)!=='1')throw 'captured growing length';
var values=[{toString:function(){delete values[1];values[2]=9;values.push(10);return 'first';}},2,3];
if(values.join('|')!=='first||9'||values.length!==4)throw 'live conversion mutation';
var parent=Object.create(Array.prototype),reads=0;Object.defineProperty(parent,'1',{get:function(){reads++;return {toString:function(){return 'inherited';}};}});
var holes=Array(3);Object.setPrototypeOf(holes,parent);if(holes.join('|')!=='|inherited|'||reads!==1)throw 'inherited index';
var marker={},called=0,s={toString:function(){throw marker;}};
try{[{toString:function(){called++;return 'wrong';}}].join(s);}catch(e){if(e!==marker)throw e;}
if(called!==0)throw 'separator error order';
'ok';
"#,
        );
    }

    #[test]
    fn actual_array_join_nested_overrides_keep_lookup_and_fallback_order() {
        join_succeeds(
            r#"
var log='',child=[1,2];
Object.defineProperty(child,'toString',{get:function(){log+='t';return Array.prototype.toString;}});
Object.defineProperty(child,'join',{get:function(){log+='j';return function(){log+='c';return {};};},configurable:true});
Object.defineProperty(child,'valueOf',{get:function(){log+='v';return function(){log+='V';return 'fallback';};},configurable:true});
if([child].join()!=='fallback'||log!=='tjcvV')throw 'nested method order';
Object.defineProperty(child,'join',{value:7});log='';
if([child].join()!=='[object Array]'||log!=='t')throw 'noncallable join fallback';
var custom=[3];custom.toString=function(){return 'override';};Object.defineProperty(custom,'join',{get:function(){throw 'unneeded join';}});
if([custom].join()!=='override')throw 'toString override';
'ok';
"#,
        );
    }

    #[test]
    fn actual_array_join_cycles_reentrancy_and_throw_cleanup_keep_native_depth() {
        let mut runtime = Runtime::for_test();
        assert_eq!(runtime.execute(r#"
var a=[];a[0]={toString:function(){return a.join(':');}};a[1]=2;
if(a.join('|')!=='|2'||a.join('|')!=='|2')throw 'reentrant cycle';
var b=[],c=[b];b[0]=c;if([b,b].join('|')!=='|')throw 'mutual cycle';
var depth=['end'];for(var i=0;i<24;i++)depth=[depth];if(depth.join()!=='end')throw 'native depth regression';
var marker={},later=0,broken=[{toString:function(){throw marker;}},{toString:function(){later++;return 'second';}}];
try{broken.join();}catch(e){if(e!==marker)throw e;}if(later!==0)throw 'abrupt order';
broken[0]=1;if(broken.join('|')!=='1|second'||later!==1)throw 'cycle guard cleanup';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
        assert!(runtime.active_array_joins.is_empty());
        assert_eq!(runtime.call_depth, 0);
    }

    #[test]
    fn browser_array_join_exceeds_regression_nesting_with_real_conversion() {
        std::thread::Builder::new().stack_size(8 * 1024 * 1024).spawn(|| {
            let mut runtime = Runtime::new();
            assert_eq!(runtime.execute("var a=['leaf'];for(var i=0;i<48;i++)a=[a];a.join();", &mut NoHost).unwrap().to_text(), "leaf");
            assert!(runtime.active_array_joins.is_empty());
        }).unwrap().join().unwrap();
    }

    #[test]
    fn actual_array_join_fatal_limits_clear_cycle_state_without_cleanup_callbacks() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            for source in [
                "var a=[{toString:function(){while(true){}}}];try{a.join();}catch(e){caught=true;}finally{finalized=true;}",
                "var a=Array(4096);try{a.join('0123456789012345678901234567890123456789');}catch(e){caught=true;}finally{finalized=true;}",
                "var a=[];for(var i=0;i<40;i++)a=[a];try{a.join();}catch(e){caught=true;}finally{finalized=true;}",
                "var o={toString:function(){return [o].join();}};try{[o].join();}catch(e){caught=true;}finally{finalized=true;}",
            ] {
                let mut runtime=Runtime::for_test();
                runtime.execute("var caught=false,finalized=false;",&mut NoHost).unwrap();
                let error=runtime.execute(source,&mut NoHost).unwrap_err();
                assert!(error.contains("limit"),"{error}");
                assert!(runtime.active_array_joins.is_empty());
                assert_eq!(runtime.call_depth,0);
                assert_eq!(runtime.eval_depth,0);
                runtime.begin_job();
                assert_eq!(runtime.execute("!caught&&!finalized&&[1,2].join('|')==='1|2';",&mut NoHost).unwrap().to_text(),"true");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn actual_array_join_keeps_native_conversion_denied_before_host_reads() {
        struct Native {
            reads: usize,
        }
        impl Host for Native {
            fn primitive_conversion(&self, _: u32) -> bool {
                true
            }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                self.reads += 1;
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                Err("Unexpected native write".into())
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                Err("Unexpected native call".into())
            }
        }
        let mut runtime = Runtime::for_test();
        let mut host = Native { reads: 0 };
        runtime.define_global("native", Value::Host(17)).unwrap();
        let error=runtime.execute("var caught=false,finalized=false;try{[native].join();}catch(e){caught=true;}finally{finalized=true;}",&mut host).unwrap_err();
        assert!(
            error.contains("cannot implicitly convert host objects"),
            "{error}"
        );
        assert_eq!(host.reads, 0);
        assert!(runtime.active_array_joins.is_empty());
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!caught&&!finalized;", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    fn flatten_succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn flatten_arrays_skip_holes_and_preserve_depth_and_nonarrays() {
        flatten_succeeds(
            r#"
var a=[,1,[,2,[3,,4]],undefined],zero=a.flat(0),one=a.flat(),deep=a.flat(Infinity);
if(zero.length!==3||!Object.hasOwn(zero,0)||zero[1]!==a[2])throw 'zero depth';
if(one.length!==4||one[2]!==a[2][2]||deep.join('|')!=='1|2|3|4|')throw 'depth';
if(a.flat(-3).length!==3||a.flat(NaN).length!==3||a.flat(1.9).length!==4)throw 'depth conversion';
var o={0:'x',length:1},typed=new Uint8Array([2,3]),kept=[o,typed].flat();
if(kept.length!==2||kept[0]!==o||kept[1]!==typed)throw 'only arrays flatten';
if(Array.prototype.flat.length!==0||Array.prototype.flatMap.length!==1)throw 'metadata';
'ok';
"#,
        );
    }

    #[test]
    fn flatten_arrays_observe_live_getters_and_inherited_indices() {
        flatten_succeeds(
            r#"
var log='',inner=[2],a=[1,inner,3],depth={valueOf(){log+='d';a[0]=8;return 2;}};
Object.defineProperty(a,'1',{get:function(){log+='g';inner.push(4);delete a[2];return inner;}});
if(a.flat(depth).join()!=='8,2,4'||log!=='dg')throw 'live ordering';
var parent={1:[7]},o=Object.create(parent);o[0]=[5];o.length=3;
if(Array.prototype.flat.call(o).join()!=='5,7'||Object.hasOwn(o,1))throw 'inherited holes';
'ok';
"#,
        );
    }

    #[test]
    fn flatten_map_preserves_receiver_source_and_one_level_expansion() {
        flatten_succeeds(
            r#"
var a=[,1,2,3],seen=[],receiver={n:10};
var result=a.flatMap(function(value,index,original){'use strict';
if(this!==receiver||original!==a||arguments.length!==3)throw 'callback arguments';
seen.push(index+':'+value);if(index===1){a[2]=8;a.push(9);delete a[3];}return [,value,this.n];},receiver);
if(result.join()!=='1,10,8,10'||seen.join('|')!=='1:1|2:8')throw 'map traversal';
if(!Array.isArray([1].flatMap(function(){return [[2]];})[0]))throw 'one level';
var count=0,caught=false;try{Array.prototype.flatMap.call({get length(){count++;return 0;}},42);}catch(e){caught=e.name==='TypeError';}
if(count!==1||!caught)throw 'length before callable validation';
'ok';
"#,
        );
    }

    #[test]
    fn flatten_species_observes_order_and_creates_own_properties() {
        flatten_succeeds(
            r#"
var log='',target={},a=[1,[2]],holder={};
Object.defineProperty(a,'constructor',{get:function(){log+='c';return holder;}});
Object.defineProperty(holder,Symbol.species,{get:function(){log+='s';return function(n){log+='n'+n;return target;};}});
Object.setPrototypeOf(target,{set 0(v){throw 'inherited setter';}});
var result=a.flat({valueOf(){log+='d';return 1;}}),d=Object.getOwnPropertyDescriptor(result,'0');
if(result!==target||result[0]!==1||result[1]!==2||result.length!==undefined||log!=='dcsn0')throw 'species';
if(!d.writable||!d.enumerable||!d.configurable)throw 'own data';
if(Object.getOwnPropertyDescriptor(Array,Symbol.species).get.call({x:1}).x!==1)throw 'species getter';
var bad={};Object.preventExtensions(bad);a=[1];a.constructor={};a.constructor[Symbol.species]=function(){return bad;};
var failed=false;try{a.flat();}catch(e){failed=e.name==='TypeError';}if(!failed)throw 'target rejection';
'ok';
"#,
        );
    }

    #[test]
    fn flatten_limits_are_fatal_and_recover_on_the_next_job() {
        for source in [
            "var a=[];a[0]=a;try{a.flat(Infinity);}catch(e){caught=true;}finally{finished=true;}",
            "var a=Array(2050).fill(1);try{[a,a].flat();}catch(e){caught=true;}finally{finished=true;}",
        ] {
            let mut runtime=Runtime::for_test();
            runtime.execute("var caught=false,finished=false;",&mut NoHost).unwrap();
            let error=runtime.execute(source,&mut NoHost).unwrap_err();
            assert!(error.contains("limit"),"{error}");
            assert_eq!(runtime.call_depth,0);
            assert_eq!(runtime.eval_depth,0);
            runtime.begin_job();
            assert_eq!(runtime.execute("!caught&&!finished;",&mut NoHost).unwrap().to_text(),"true");
        }
    }

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No host object is exposed in this test.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No host object is exposed in this test.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No host object is exposed in this test.".into())
        }
    }

    #[test]
    fn default_comparison_finishes_left_string_conversion_before_right_callback() {
        for object_left in [false, true] {
            let mut runtime = Runtime::for_test();
            let mut host = NoHost;
            runtime
                .execute(
                    "var leftCalls=0,rightCalls=0;var key=Symbol('left');",
                    &mut host,
                )
                .unwrap();
            let left = runtime
                .execute(
                    if object_left {
                        "({toString(){leftCalls++;return key;}});"
                    } else {
                        "key;"
                    },
                    &mut host,
                )
                .unwrap();
            let right = runtime
                .execute("({toString(){rightCalls++;return 'right';}});", &mut host)
                .unwrap();
            let fault = runtime
                .compare_array_values(&left, &right, None, false, &mut host)
                .unwrap_err();
            assert!(matches!(
                fault.exception.as_deref(),
                Some(super::super::Exception::Error("TypeError"))
            ));
            assert_eq!(
                runtime
                    .execute("leftCalls+':'+rightCalls;", &mut host)
                    .unwrap()
                    .to_text(),
                if object_left { "1:0" } else { "0:0" }
            );

            // A supplied comparator consumes the values directly. The default
            // string-conversion rule must not reject its Symbol arguments.
            let comparator = runtime
                .execute("(function(){return 0;});", &mut host)
                .unwrap();
            assert_eq!(
                runtime
                    .compare_array_values(&left, &right, Some(&comparator), false, &mut host)
                    .unwrap(),
                Ordering::Equal
            );
            assert_eq!(
                runtime.execute("rightCalls;", &mut host).unwrap().to_text(),
                "0"
            );
        }
    }
}

#[cfg(test)]
#[path="array_flat_workspace_tests.rs"]
mod flat_workspace_tests;
