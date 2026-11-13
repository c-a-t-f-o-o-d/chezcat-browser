//! Original Promise resolution and FIFO microtasks over the bounded interpreter.
//! The host performs checkpoints only after the JavaScript stack is empty.
//! Resource/capability faults remain fatal, never ordinary Promise rejections.
//! https://tc39.es/ecma262/#sec-promise-objects
//! https://html.spec.whatwg.org/multipage/webappapis.html#perform-a-microtask-checkpoint

use super::{
    Accessors, Builtin, Declaration, Exception, Fault, Function, Host, Ordinary, Runtime,
    Value, MAX_CALL_DEPTH, MAX_FUNCTIONS, MAX_STRING_BYTES,
};
use std::collections::VecDeque;

const MAX_PROMISES: usize = 1024;
const MAX_REACTIONS: usize = 1024;
const MAX_MICROTASKS: usize = 1024;
const MAX_CHECKPOINT_JOBS: usize = 4096;
const MAX_DIAGNOSTICS: usize = 32;

#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    Resolve,
    Reject,
    Then,
    Catch,
    Finally,
    Species,
    QueueMicrotask,
    All,
    Race,
    AllSettled,
    Any,
    WithResolvers,
    AggregateError,
    Try,
}

impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Then | Self::AggregateError => 2,
            Self::Species | Self::WithResolvers => 0,
            _ => 1,
        }
    }
}

pub(super) struct Data {
    state: State,
    reactions: Vec<Reaction>,
    handled: bool,
    reported: bool,
}

enum State {
    Pending,
    Fulfilled(Value),
    Rejected(Value),
}

#[derive(Clone)]
struct Capability {
    promise: Value,
    resolve: Value,
    reject: Value,
}

struct Reaction {
    capability: Capability,
    fulfill: Option<usize>,
    reject: Option<usize>,
}

struct ResolvingPair {
    promise: usize,
    used: bool,
}

struct CapabilitySlot {
    resolve: Option<Value>,
    reject: Option<Value>,
}

#[derive(Clone, Copy)]
enum GroupKind {
    All,
    AllSettled,
    Any,
}

struct Group {
    capability: Capability,
    kind: GroupKind,
    values: usize,
    remaining: usize,
}
struct Element {
    group: usize,
    index: usize,
    called: bool,
}

#[derive(Clone)]
pub(super) enum Closure {
    Resolve {
        pair: usize,
        reject: bool,
    },
    Capture {
        slot: usize,
    },
    Finally {
        handler: usize,
        constructor: usize,
        reject: bool,
    },
    FinallyValue {
        value: Value,
        reject: bool,
    },
    Element {
        element: usize,
        reject: bool,
    },
}

impl Closure {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::FinallyValue { value, .. } => value.bytes(),
                _ => 0,
            }
    }
    fn arity(&self) -> usize {
        match self {
            Self::Capture { .. } => 2,
            Self::FinallyValue { .. } => 0,
            _ => 1,
        }
    }
}

enum Job {
    Thenable {
        promise: usize,
        receiver: Value,
        then: Value,
    },
    Reaction {
        capability: Capability,
        handler: Option<usize>,
        value: Value,
        reject: bool,
    },
    Callback(usize),
}

impl Job {
    fn allocation_kind(&self) -> usize {
        match self { Self::Callback(_) => 0, Self::Reaction { .. } => 1, Self::Thenable { .. } => 2 }
    }

    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Thenable { receiver, then, .. } => receiver.bytes() + then.bytes(),
                Self::Reaction {
                    capability, value, ..
                } => {
                    capability.promise.bytes()
                        + capability.resolve.bytes()
                        + capability.reject.bytes()
                        + value.bytes()
                }
                Self::Callback(_) => 0,
            }
    }
}

#[derive(Default)]
struct AllocationTrace {
    active: Option<(usize, usize)>,
    started: [usize; 3],
    succeeded: [usize; 3],
    scripts: [usize; 3],
}

#[derive(Default)]
pub(super) struct Arena {
    pub(super) prototype: Option<usize>,
    pub(super) constructor: Option<usize>,
    count: usize,
    reactions: usize,
    pairs: Vec<ResolvingPair>,
    capabilities: Vec<CapabilitySlot>,
    groups: Vec<Group>,
    elements: Vec<Element>,
    jobs: VecDeque<(Option<usize>, Job)>,
    draining: bool,
    allocation_trace: AllocationTrace,
}

impl Capability {
    fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        for value in [&self.promise, &self.resolve, &self.reject] { super::function_reachability::Edge::value(value, add); }
    }
}

impl Data {
    pub(super) fn reachability_cost(&self) -> usize { 1 + self.reactions.len() * 5 }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        match &self.state { State::Fulfilled(value) | State::Rejected(value) => Edge::value(value, add), State::Pending => {} }
        for reaction in &self.reactions {
            reaction.capability.reachability_edges(add);
            for id in [reaction.fulfill, reaction.reject].into_iter().flatten() { add(Edge::Function(id)); }
        }
    }
}

impl Closure {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        match self {
            Self::Finally { handler, constructor, .. } => { add(Edge::Function(*handler)); add(Edge::Function(*constructor)); }
            Self::FinallyValue { value, .. } => Edge::value(value, add),
            // Pair/slot/group metadata are conservatively rooted by Arena.
            _ => {}
        }
    }
}

impl Arena {
    pub(super) fn reachability_cost(&self) -> usize {
        2 + self.pairs.len() + self.capabilities.len() * 2 + self.groups.len() * 4 + self.jobs.len() * 5
    }

    pub(super) fn reachability_is_inactive(&self) -> bool {
        !self.draining && self.allocation_trace.active.is_none()
    }

    pub(super) fn reachability_job_is_registered(&self, registrations: usize) -> bool {
        if self.draining { registrations == 1 && self.allocation_trace.active.is_some() }
        else { registrations == 0 && self.allocation_trace.active.is_none() }
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        if let Some(id) = self.prototype { add(Edge::Object(id)); }
        if let Some(id) = self.constructor { add(Edge::Function(id)); }
        for pair in &self.pairs { add(Edge::Object(pair.promise)); }
        for slot in &self.capabilities {
            for value in [&slot.resolve, &slot.reject].into_iter().flatten() { Edge::value(value, add); }
        }
        for group in &self.groups { group.capability.reachability_edges(add); add(Edge::Object(group.values)); }
        for (_, job) in &self.jobs {
            match job {
                Job::Callback(id) => add(Edge::Function(*id)),
                Job::Thenable { promise, receiver, then } => {
                    add(Edge::Object(*promise)); Edge::value(receiver, add); Edge::value(then, add);
                }
                Job::Reaction { capability, handler, value, .. } => {
                    capability.reachability_edges(add);
                    if let Some(id) = handler { add(Edge::Function(*id)); }
                    Edge::value(value, add);
                }
            }
        }
    }
}

impl Runtime {
    pub(super) fn microtask_allocation_diagnostic(&self) -> String {
        let trace = &self.promises.allocation_trace;
        let mut scripts = trace.scripts;
        if let Some((kind, first)) = trace.active {
            scripts[kind] = scripts[kind].saturating_add(self.function_arena.script_created.saturating_sub(first));
        }
        let active = trace.active.map(|(kind, _)| ["callback", "reaction", "thenable"][kind]);
        format!("microtasks-active={active:?}; callback/reaction/thenable-started={:?}; succeeded={:?}; script-created={scripts:?}; queued={}; resolving-pairs={}; promises={}",
            trace.started, trace.succeeded, self.promises.jobs.len(), self.promises.pairs.len(), self.promises.count)
    }

    fn promise_retain(&mut self, bytes: usize) -> Result<(), Fault> {
        if self.value_bytes.saturating_add(bytes) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work(bytes)?;
        self.value_bytes += bytes;
        Ok(())
    }

    pub(super) fn initialize_promises(&mut self) -> Result<(), Fault> {
        let constructor = self.native_function(Builtin::Promise(Op::Constructor), "Promise")?;
        let Value::Function(index) = constructor else {
            unreachable!()
        };
        self.promises.constructor = Some(index);
        self.put_binding(
            self.global_environment,
            "Promise".into(),
            Some(constructor),
            Declaration::Var,
            false,
        )?;
        let prototype = self.new_object(None)?;
        self.promises.prototype = Some(prototype);
        self.constructor_prototype("Promise", Value::Object(prototype))?;
        for (name, op) in [
            ("then", Op::Then),
            ("catch", Op::Catch),
            ("finally", Op::Finally),
        ] {
            self.prototype_method(prototype, name, Builtin::Promise(op))?;
        }
        let properties = self.ordinary_id(Ordinary::Function(index))?;
        for (name, op) in [
            ("resolve", Op::Resolve),
            ("reject", Op::Reject),
            ("all", Op::All),
            ("race", Op::Race),
            ("allSettled", Op::AllSettled),
            ("any", Op::Any),
            ("withResolvers", Op::WithResolvers),
            ("try", Op::Try),
        ] {
            self.prototype_method(properties, name, Builtin::Promise(op))?;
        }
        let getter = self.native_function(Builtin::Promise(Op::Species), "get [Symbol.species]")?;
        let Value::Function(getter) = getter else {
            unreachable!()
        };
        let key = self.symbol_species.unwrap();
        self.store_symbol_property(
            properties,
            key,
            Value::Undefined,
            Some((false, false, true)),
        )?;
        self.objects[properties]
            .symbol_properties
            .get_mut(&key)
            .unwrap()
            .accessor = Some(Accessors {
            get: Some(getter),
            set: None,
        });
        let array = self.binding_value(self.global_environment, "Array")?;
        let array = self.ordinary_id(self.ordinary_required(&array)?)?;
        let array_getter =
            self.native_function(Builtin::Promise(Op::Species), "get [Symbol.species]")?;
        let Value::Function(array_getter) = array_getter else {
            unreachable!()
        };
        self.store_symbol_property(array, key, Value::Undefined, Some((false, false, true)))?;
        self.objects[array]
            .symbol_properties
            .get_mut(&key)
            .unwrap()
            .accessor = Some(Accessors {
            get: Some(array_getter),
            set: None,
        });
        self.initialize_aggregate_error()?;
        let callback =
            self.native_function(Builtin::Promise(Op::QueueMicrotask), "queueMicrotask")?;
        self.put_binding(
            self.global_environment,
            "queueMicrotask".into(),
            Some(callback),
            Declaration::Var,
            false,
        )
    }

    fn promise_function(&mut self, closure: Closure) -> Result<Value, Fault> {
        self.step()?;
        if self.function_arena.occupied >= MAX_FUNCTIONS {
            return Err(self.function_limit_fault("promise-closure"));
        }
        let length = closure.arity();
        self.promise_retain(closure.retained_bytes())?;
        let allocation = self.insert_function_slot(Function::Promise(closure))?;
        self.initialize_function_properties(allocation, "", length, false)?;
        Ok(Value::Function(allocation.id))
    }

    fn new_promise(&mut self) -> Result<usize, Fault> {
        if self.promises.count >= MAX_PROMISES {
            return Err(Fault::new("Promise identity limit reached (1024)."));
        }
        self.promise_retain(std::mem::size_of::<Data>())?;
        let object = self.new_object(None)?;
        self.objects[object].prototype = self.promises.prototype.map(Ordinary::Object);
        self.objects[object].promise = Some(Data {
            state: State::Pending,
            reactions: Vec::new(),
            handled: false,
            reported: false,
        });
        self.promises.count += 1;
        Ok(object)
    }

    fn resolving_pair(&mut self, promise: usize) -> Result<(Value, Value), Fault> {
        if self.promises.pairs.len() >= MAX_FUNCTIONS {
            return Err(Fault::new("Promise resolving-pair limit reached (1024)."));
        }
        self.promise_retain(std::mem::size_of::<ResolvingPair>())?;
        let pair = self.promises.pairs.len();
        self.promises.pairs.push(ResolvingPair {
            promise,
            used: false,
        });
        let resolve = self.promise_function(Closure::Resolve {
            pair,
            reject: false,
        })?;
        let reject = self.promise_function(Closure::Resolve { pair, reject: true })?;
        Ok((resolve, reject))
    }

    fn promise_exception(&mut self, fault: Fault) -> Result<Value, Fault> {
        match fault.exception {
            Some(exception) => match *exception {
                Exception::Value(value) => Ok(value),
                Exception::Error(name) => self.error_object(name, Some(fault.message)),
            },
            None => Err(fault),
        }
    }

    pub(super) fn construct_promise(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = (|| {
            let executor = args.first().unwrap_or(&Value::Undefined);
            if !matches!(executor, Value::Function(_)) {
                return Err(Fault::exception(
                    "TypeError",
                    "Promise executor must be callable.",
                ));
            }
            let promise = self.new_promise()?;
            let (resolve, reject) = self.resolving_pair(promise)?;
            if let Err(error) = self.call_value(
                executor.clone(),
                &[resolve, reject.clone()],
                Value::Undefined,
                host,
            ) {
                let reason = self.promise_exception(error)?;
                self.call_value(reject, &[reason], Value::Undefined, host)?;
            }
            Ok(Value::Object(promise))
        })();
        self.call_depth -= 1;
        result
    }

    fn new_promise_capability(
        &mut self,
        constructor: Value,
        host: &mut dyn Host,
    ) -> Result<Capability, Fault> {
        if !self.from_constructor(&constructor)? {
            return Err(Fault::exception(
                "TypeError",
                "Promise capability requires a constructor.",
            ));
        }
        if matches!(constructor, Value::Function(index) if Some(index) == self.promises.constructor)
        {
            let promise = self.new_promise()?;
            let (resolve, reject) = self.resolving_pair(promise)?;
            return Ok(Capability {
                promise: Value::Object(promise),
                resolve,
                reject,
            });
        }
        if self.promises.capabilities.len() >= MAX_FUNCTIONS {
            return Err(Fault::new("Promise capability limit reached (1024)."));
        }
        self.promise_retain(std::mem::size_of::<CapabilitySlot>())?;
        let slot = self.promises.capabilities.len();
        self.promises.capabilities.push(CapabilitySlot {
            resolve: None,
            reject: None,
        });
        let executor = self.promise_function(Closure::Capture { slot })?;
        let promise = self.construct_value(constructor, &[executor], host)?;
        let data = &self.promises.capabilities[slot];
        let (Some(Value::Function(resolve)), Some(Value::Function(reject))) =
            (&data.resolve, &data.reject)
        else {
            return Err(Fault::exception(
                "TypeError",
                "Promise capability resolve and reject must be callable.",
            ));
        };
        Ok(Capability {
            promise,
            resolve: Value::Function(*resolve),
            reject: Value::Function(*reject),
        })
    }

    fn promise_enqueue(&mut self, job: Job) -> Result<(), Fault> {
        self.step()?;
        if self.promises.jobs.len() >= MAX_MICROTASKS {
            return Err(Fault::new("Microtask queue limit reached (1024)."));
        }
        self.promise_retain(job.retained_bytes().saturating_add(std::mem::size_of::<Option<usize>>()))?;
        let callback = match &job {
            Job::Thenable { then: Value::Function(id), .. } => Some(*id),
            Job::Reaction { handler: Some(id), .. } | Job::Callback(id) => Some(*id),
            _ => None,
        };
        let realm = callback.map_or(self.active_realm, |id| self.function_realms[id]);
        self.promises.jobs.push_back((realm, job));
        Ok(())
    }

    fn promise_settle(&mut self, promise: usize, value: Value, reject: bool) -> Result<(), Fault> {
        self.step()?;
        if !matches!(
            self.objects[promise].promise.as_ref().unwrap().state,
            State::Pending
        ) {
            return Ok(());
        }
        if value.bytes() > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        self.promise_retain(value.bytes())?;
        let data = self.objects[promise].promise.as_mut().unwrap();
        let reactions = std::mem::take(&mut data.reactions);
        data.state = if reject {
            State::Rejected(value.clone())
        } else {
            State::Fulfilled(value.clone())
        };
        self.promises.reactions -= reactions.len();
        self.value_bytes -= reactions.len() * std::mem::size_of::<Reaction>();
        for reaction in reactions {
            let handler = if reject {
                reaction.reject
            } else {
                reaction.fulfill
            };
            self.promise_enqueue(Job::Reaction {
                capability: reaction.capability,
                handler,
                value: value.clone(),
                reject,
            })?;
        }
        Ok(())
    }

    fn resolve_promise(
        &mut self,
        promise: usize,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if matches!(value, Value::Object(id) if id == promise) {
            let reason = self.error_object(
                "TypeError",
                Some("A Promise cannot resolve to itself.".into()),
            )?;
            return self.promise_settle(promise, reason, true);
        }
        let then = if let Value::Host(handle) = &value {
            if !host.promise_resolution_receiver(*handle) {
                return Err(Fault::new(
                    "Native host Promise assimilation is not enabled for this receiver.",
                ));
            }
            Some(self.read_reference(
                &super::Reference::Property(value.clone(), "then".into()),
                host,
            ))
        } else {
            Ordinary::from_value(&value)
                .map(|object| self.get_ordinary_property_with_host(object, "then", host))
        };
        if let Some(then) = then {
            let then = match then {
                Ok(then) => then,
                Err(fault) => {
                    let reason = self.promise_exception(fault)?;
                    return self.promise_settle(promise, reason, true);
                }
            };
            if matches!(then, Value::Function(_)) {
                return self.promise_enqueue(Job::Thenable {
                    promise,
                    receiver: value,
                    then,
                });
            }
        }
        self.promise_settle(promise, value, false)
    }

    fn promise_resolve(
        &mut self,
        constructor: Value,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if let Value::Object(id) = value {
            if self.object(id)?.promise.is_some() {
                let actual = self.get_property_with_host(id, "constructor", host)?;
                if super::strict_equal(&constructor, &actual) {
                    return Ok(Value::Object(id));
                }
            }
        }
        let capability = self.new_promise_capability(constructor, host)?;
        self.call_value(capability.resolve, &[value], Value::Undefined, host)?;
        Ok(capability.promise)
    }

    fn promise_species(&mut self, receiver: &Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let object = self.ordinary_required(receiver)?;
        let constructor = self.get_ordinary_property_with_host(object, "constructor", host)?;
        let default = Value::Function(self.promises.constructor.unwrap());
        if matches!(constructor, Value::Undefined) {
            return Ok(default);
        }
        if matches!(constructor, Value::Host(_)) {
            return Err(Fault::new(
                "Native host Promise species is not implemented.",
            ));
        }
        if Ordinary::from_value(&constructor).is_none() {
            return Err(Fault::exception(
                "TypeError",
                "Promise constructor property must be an object.",
            ));
        }
        let species = self.get_symbol_property(constructor, self.symbol_species.unwrap(), host)?;
        if matches!(species, Value::Null | Value::Undefined) {
            return Ok(default);
        }
        if !self.from_constructor(&species)? {
            return Err(Fault::exception(
                "TypeError",
                "Promise species must be a constructor.",
            ));
        }
        Ok(species)
    }

    fn promise_then(
        &mut self,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Value::Object(promise) = receiver else {
            return Err(Fault::exception(
                "TypeError",
                "Promise.then requires a Promise receiver.",
            ));
        };
        if self.object(promise)?.promise.is_none() {
            return Err(Fault::exception(
                "TypeError",
                "Promise.then requires a Promise receiver.",
            ));
        }
        let constructor = self.promise_species(&Value::Object(promise), host)?;
        let capability = self.new_promise_capability(constructor, host)?;
        let returned = capability.promise.clone();
        let fulfill = args.first().and_then(|value| match value {
            Value::Function(id) => Some(*id),
            _ => None,
        });
        let reject_handler = args.get(1).and_then(|value| match value {
            Value::Function(id) => Some(*id),
            _ => None,
        });
        match &self.objects[promise].promise.as_ref().unwrap().state {
            State::Pending => {
                if self.promises.reactions >= MAX_REACTIONS {
                    return Err(Fault::new("Promise reaction limit reached (1024)."));
                }
                self.promise_retain(std::mem::size_of::<Reaction>())?;
                self.objects[promise]
                    .promise
                    .as_mut()
                    .unwrap()
                    .reactions
                    .push(Reaction {
                        capability,
                        fulfill,
                        reject: reject_handler,
                    });
                self.promises.reactions += 1;
            }
            State::Fulfilled(value) | State::Rejected(value) => {
                let value = value.clone();
                let reject = matches!(
                    self.objects[promise].promise.as_ref().unwrap().state,
                    State::Rejected(_)
                );
                self.promise_enqueue(Job::Reaction {
                    capability,
                    handler: if reject { reject_handler } else { fulfill },
                    value,
                    reject,
                })?;
            }
        }
        self.objects[promise].promise.as_mut().unwrap().handled = true;
        Ok(returned)
    }

    fn promise_invoke_then(
        &mut self,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let then = self.read_reference(
            &super::Reference::Property(receiver.clone(), "then".into()),
            host,
        )?;
        self.call_value(then, args, receiver, host)
    }

    fn promise_finally(
        &mut self,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.ordinary_required(&receiver)?;
        let constructor = self.promise_species(&receiver, host)?;
        let callback = args.first().unwrap_or(&Value::Undefined);
        if let Value::Function(handler) = callback {
            let Value::Function(constructor) = constructor else {
                unreachable!()
            };
            let fulfill = self.promise_function(Closure::Finally {
                handler: *handler,
                constructor,
                reject: false,
            })?;
            let reject = self.promise_function(Closure::Finally {
                handler: *handler,
                constructor,
                reject: true,
            })?;
            self.promise_invoke_then(receiver, &[fulfill, reject], host)
        } else {
            self.promise_invoke_then(receiver, &[callback.clone(), callback.clone()], host)
        }
    }

    fn initialize_aggregate_error(&mut self) -> Result<(), Fault> {
        let constructor =
            self.native_function(Builtin::Promise(Op::AggregateError), "AggregateError")?;
        let constructor_object = self.ordinary_id(self.ordinary_required(&constructor)?)?;
        let error = self.binding_value(self.global_environment, "Error")?;
        self.objects[constructor_object].prototype = Ordinary::from_value(&error);
        self.put_binding(
            self.global_environment,
            "AggregateError".into(),
            Some(constructor),
            Declaration::Var,
            false,
        )?;
        let prototype = self.new_object(None)?;
        self.objects[prototype].prototype = self
            .error_prototypes
            .get("Error")
            .copied()
            .map(Ordinary::Object);
        self.error_prototypes.insert("AggregateError", prototype);
        self.constructor_prototype("AggregateError", Value::Object(prototype))?;
        self.store_property(
            prototype,
            "name".into(),
            Value::String("AggregateError".into()),
            Some((false, true, true)),
        )?;
        self.store_property(
            prototype,
            "message".into(),
            Value::String(String::new()),
            Some((false, true, true)),
        )
    }

    fn aggregate_error_constructor(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let message = args.get(1).unwrap_or(&Value::Undefined);
        let message = if matches!(message, Value::Undefined) {
            None
        } else {
            Some(
                self.primitive_with_hint(message.clone(), true, host)?
                    .script_text()?,
            )
        };
        let error = self.error_object("AggregateError", message)?;
        let Value::Object(error_id) = error else {
            unreachable!()
        };
        if let Some(options) = args.get(2) {
            if matches!(options, Value::Host(_)) {
                return Err(Fault::new("Native host error options are not implemented."));
            }
            if let Some(options) = Ordinary::from_value(options) {
                if self.property_owner(options, "cause")?.is_some() {
                    let cause = self.get_ordinary_property_with_host(options, "cause", host)?;
                    self.store_property(
                        error_id,
                        "cause".into(),
                        cause,
                        Some((false, true, true)),
                    )?;
                }
            }
        }
        let source = args.first().unwrap_or(&Value::Undefined);
        let method = self.iterator_method(source, host)?.ok_or_else(|| {
            Fault::exception("TypeError", "AggregateError errors must be iterable.")
        })?;
        let mut iterator = self.iterator_from_method(source.clone(), method, host)?;
        let values = self.new_object(Some(0))?;
        let mut index = 0;
        while let Some(value) = self.iterator_next(&mut iterator, host)? {
            self.store_property(values, index.to_string(), value, Some((true, true, true)))?;
            index += 1;
        }
        self.store_property(
            error_id,
            "errors".into(),
            Value::Object(values),
            Some((false, true, true)),
        )?;
        Ok(error)
    }

    fn promise_combinator(
        &mut self,
        op: Op,
        constructor: Value,
        source: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let capability = self.new_promise_capability(constructor.clone(), host)?;
        let result = (|| {
            let ordinary = self.ordinary_required(&constructor)?;
            let resolve = self.get_ordinary_property_with_host(ordinary, "resolve", host)?;
            if !matches!(resolve, Value::Function(_)) {
                return Err(Fault::exception(
                    "TypeError",
                    "Promise constructor resolve property must be callable.",
                ));
            }
            let method = self.iterator_method(source, host)?.ok_or_else(|| {
                Fault::exception("TypeError", "Promise combinator requires an iterable.")
            })?;
            let mut iterator = self.iterator_from_method(source.clone(), method, host)?;
            let consumed = self.perform_promise_combinator(
                op,
                &constructor,
                &resolve,
                &capability,
                &mut iterator,
                host,
            );
            self.iterator_close(&mut iterator, consumed, host)
        })();
        if let Err(error) = result {
            let reason = self.promise_exception(error)?;
            self.call_value(capability.reject.clone(), &[reason], Value::Undefined, host)?;
        }
        Ok(capability.promise)
    }

    fn perform_promise_combinator(
        &mut self,
        op: Op,
        constructor: &Value,
        resolve: &Value,
        capability: &Capability,
        iterator: &mut super::iterator_protocol::Record,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let group = if matches!(op, Op::Race) {
            None
        } else {
            if self.promises.groups.len() >= MAX_PROMISES {
                return Err(Fault::new("Promise combinator state limit reached (1024)."));
            }
            self.promise_retain(std::mem::size_of::<Group>())?;
            let values = self.new_object(Some(0))?;
            let group = self.promises.groups.len();
            let kind = match op {
                Op::All => GroupKind::All,
                Op::AllSettled => GroupKind::AllSettled,
                Op::Any => GroupKind::Any,
                _ => unreachable!(),
            };
            self.promises.groups.push(Group {
                capability: capability.clone(),
                kind,
                values,
                remaining: 1,
            });
            Some(group)
        };
        let mut index = 0;
        while let Some(value) = self.iterator_next(iterator, host)? {
            self.step()?;
            if let Some(group) = group {
                let values = self.promises.groups[group].values;
                self.store_property(
                    values,
                    index.to_string(),
                    Value::Undefined,
                    Some((true, true, true)),
                )?;
            }
            let next = self.call_value(resolve.clone(), &[value], constructor.clone(), host)?;
            let (fulfill, reject) = if let Some(group) = group {
                if self.promises.elements.len() >= MAX_REACTIONS {
                    return Err(Fault::new(
                        "Promise combinator element limit reached (1024).",
                    ));
                }
                self.promise_retain(std::mem::size_of::<Element>())?;
                let element = self.promises.elements.len();
                self.promises.elements.push(Element {
                    group,
                    index,
                    called: false,
                });
                let callbacks = match self.promises.groups[group].kind {
                    GroupKind::All => (
                        self.promise_function(Closure::Element {
                            element,
                            reject: false,
                        })?,
                        capability.reject.clone(),
                    ),
                    GroupKind::Any => (
                        capability.resolve.clone(),
                        self.promise_function(Closure::Element {
                            element,
                            reject: true,
                        })?,
                    ),
                    GroupKind::AllSettled => (
                        self.promise_function(Closure::Element {
                            element,
                            reject: false,
                        })?,
                        self.promise_function(Closure::Element {
                            element,
                            reject: true,
                        })?,
                    ),
                };
                self.promises.groups[group].remaining += 1;
                callbacks
            } else {
                (capability.resolve.clone(), capability.reject.clone())
            };
            self.promise_invoke_then(next, &[fulfill, reject], host)?;
            index += 1;
        }
        if let Some(group) = group {
            self.promise_group_complete_one(group, host)?;
        }
        Ok(())
    }

    fn promise_element(
        &mut self,
        element: usize,
        reject: bool,
        mut value: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.promises.elements[element].called {
            return Ok(Value::Undefined);
        }
        self.promises.elements[element].called = true;
        let element = &self.promises.elements[element];
        let (group, index) = (element.group, element.index);
        if matches!(self.promises.groups[group].kind, GroupKind::AllSettled) {
            let object = self.new_object(None)?;
            self.store_property(
                object,
                "status".into(),
                Value::String(if reject { "rejected" } else { "fulfilled" }.into()),
                Some((true, true, true)),
            )?;
            self.store_property(
                object,
                if reject { "reason" } else { "value" }.into(),
                value,
                Some((true, true, true)),
            )?;
            value = Value::Object(object);
        }
        let values = self.promises.groups[group].values;
        self.store_property(values, index.to_string(), value, Some((true, true, true)))?;
        self.promise_group_complete_one(group, host)
    }

    fn promise_group_complete_one(
        &mut self,
        group: usize,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let group = &mut self.promises.groups[group];
        group.remaining -= 1;
        if group.remaining != 0 {
            return Ok(Value::Undefined);
        }
        let value = Value::Object(group.values);
        let reject = matches!(group.kind, GroupKind::Any);
        let callback = if reject {
            group.capability.reject.clone()
        } else {
            group.capability.resolve.clone()
        };
        let value = if reject {
            let error =
                self.error_object("AggregateError", Some("All promises were rejected".into()))?;
            let Value::Object(id) = error else {
                unreachable!()
            };
            self.store_property(id, "errors".into(), value, Some((false, true, true)))?;
            error
        } else {
            value
        };
        self.call_value(callback, &[value], Value::Undefined, host)
    }

    pub(super) fn promise_builtin(
        &mut self,
        op: Op,
        args: &[Value],
        receiver: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = (|| {
            let value = args.first().unwrap_or(&Value::Undefined);
            match op {
                Op::Constructor => Err(Fault::exception(
                    "TypeError",
                    "Promise constructor requires new.",
                )),
                Op::Try => {
                    if Ordinary::from_value(&receiver).is_none() {
                        return Err(Fault::exception(
                            "TypeError",
                            "Promise.try receiver must be an object.",
                        ));
                    }
                    // Current ECMA262 calls the callback before NewPromiseCapability
                    // and uses PromiseResolve for its normal completion. Node24's
                    // older ordering/identity behavior is recorded separately.
                    match self.call_value(
                        value.clone(),
                        args.get(1..).unwrap_or(&[]),
                        Value::Undefined,
                        host,
                    ) {
                        Ok(value) => self.promise_resolve(receiver, value, host),
                        Err(error) => {
                            let reason = self.promise_exception(error)?;
                            let capability = self.new_promise_capability(receiver, host)?;
                            self.call_value(capability.reject, &[reason], Value::Undefined, host)?;
                            Ok(capability.promise)
                        }
                    }
                }
                Op::Resolve => {
                    if Ordinary::from_value(&receiver).is_none() {
                        return Err(Fault::exception(
                            "TypeError",
                            "Promise.resolve receiver must be an object.",
                        ));
                    }
                    self.promise_resolve(receiver, value.clone(), host)
                }
                Op::Reject => {
                    let capability = self.new_promise_capability(receiver, host)?;
                    self.call_value(capability.reject, &[value.clone()], Value::Undefined, host)?;
                    Ok(capability.promise)
                }
                Op::Then => self.promise_then(receiver, args, host),
                Op::Catch => {
                    self.promise_invoke_then(receiver, &[Value::Undefined, value.clone()], host)
                }
                Op::Finally => self.promise_finally(receiver, args, host),
                Op::Species => Ok(receiver),
                Op::All | Op::Race | Op::AllSettled | Op::Any => {
                    self.promise_combinator(op, receiver, value, host)
                }
                Op::WithResolvers => {
                    let capability = self.new_promise_capability(receiver, host)?;
                    let result = self.new_object(None)?;
                    for (name, value) in [
                        ("promise", capability.promise),
                        ("resolve", capability.resolve),
                        ("reject", capability.reject),
                    ] {
                        self.store_property(result, name.into(), value, Some((true, true, true)))?;
                    }
                    Ok(Value::Object(result))
                }
                Op::AggregateError => self.aggregate_error_constructor(args, host),
                Op::QueueMicrotask => {
                    let Value::Function(callback) = value else {
                        return Err(Fault::exception(
                            "TypeError",
                            "queueMicrotask callback must be callable.",
                        ));
                    };
                    self.promise_enqueue(Job::Callback(*callback))?;
                    Ok(Value::Undefined)
                }
            }
        })();
        self.call_depth -= 1;
        result
    }

    pub(super) fn promise_closure_call(
        &mut self,
        closure: Closure,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = (|| {
            let value = args.first().unwrap_or(&Value::Undefined);
            match closure {
                Closure::Resolve { pair, reject } => {
                    if self.promises.pairs[pair].used {
                        return Ok(Value::Undefined);
                    }
                    self.promises.pairs[pair].used = true;
                    let promise = self.promises.pairs[pair].promise;
                    if reject {
                        self.promise_settle(promise, value.clone(), true)?;
                    } else {
                        self.resolve_promise(promise, value.clone(), host)?;
                    }
                    Ok(Value::Undefined)
                }
                Closure::Capture { slot } => {
                    let data = &self.promises.capabilities[slot];
                    if data
                        .resolve
                        .as_ref()
                        .is_some_and(|value| !matches!(value, Value::Undefined))
                        || data
                            .reject
                            .as_ref()
                            .is_some_and(|value| !matches!(value, Value::Undefined))
                    {
                        return Err(Fault::exception(
                            "TypeError",
                            "Promise capability executor called more than once.",
                        ));
                    }
                    let resolve = value.clone();
                    let reject = args.get(1).cloned().unwrap_or(Value::Undefined);
                    self.promise_retain(resolve.bytes() + reject.bytes())?;
                    self.promises.capabilities[slot] = CapabilitySlot {
                        resolve: Some(resolve),
                        reject: Some(reject),
                    };
                    Ok(Value::Undefined)
                }
                Closure::Finally {
                    handler,
                    constructor,
                    reject,
                } => {
                    let result =
                        self.call_value(Value::Function(handler), &[], Value::Undefined, host)?;
                    let promise =
                        self.promise_resolve(Value::Function(constructor), result, host)?;
                    let callback = self.promise_function(Closure::FinallyValue {
                        value: value.clone(),
                        reject,
                    })?;
                    self.promise_invoke_then(promise, &[callback], host)
                }
                Closure::Element { element, reject } => {
                    self.promise_element(element, reject, value.clone(), host)
                }
                Closure::FinallyValue { value, reject } => {
                    self.work(value.bytes())?;
                    if reject {
                        Err(Fault::thrown(value))
                    } else {
                        Ok(value)
                    }
                }
            }
        })();
        self.call_depth -= 1;
        result
    }

    fn run_promise_job(&mut self, realm: Option<usize>, job: Job, host: &mut dyn Host) -> Result<(), Fault> {
        // The queue no longer owns this job. Publish its fields before any
        // handler runs; permanent Promise roots alone do not cover these Values.
        let marker = self.transient_marker();
        let result = (|| {
            use super::function_reachability::Edge;
            match &job {
                Job::Callback(function) => self.push_transient_root(Edge::Function(*function))?,
                Job::Thenable { promise, receiver, then } => {
                    self.push_transient_root(Edge::Object(*promise))?;
                    self.push_transient_value(receiver)?;
                    self.push_transient_value(then)?;
                }
                Job::Reaction { capability, handler, value, .. } => {
                    self.push_transient_value(&capability.promise)?;
                    self.push_transient_value(&capability.resolve)?;
                    self.push_transient_value(&capability.reject)?;
                    if let Some(function) = handler { self.push_transient_root(Edge::Function(*function))?; }
                    self.push_transient_value(value)?;
                }
            }
            self.transient_roots.active_job_registrations += 1;
            let result = self.with_intrinsic_realm(realm, |runtime| runtime.run_promise_job_registered(job, host));
            self.transient_roots.active_job_registrations -= 1;
            result
        })();
        self.restore_transient_marker(marker);
        result
    }

    fn run_promise_job_registered(&mut self, job: Job, host: &mut dyn Host) -> Result<(), Fault> {
        self.step()?;
        match job {
            Job::Callback(function) => {
                self.call_value(Value::Function(function), &[], Value::Undefined, host)?;
            }
            Job::Thenable {
                promise,
                receiver,
                then,
            } => {
                let (resolve, reject) = self.resolving_pair(promise)?;
                if let Err(error) =
                    self.call_value(then, &[resolve, reject.clone()], receiver, host)
                {
                    let reason = self.promise_exception(error)?;
                    self.call_value(reject, &[reason], Value::Undefined, host)?;
                }
            }
            Job::Reaction {
                capability,
                handler,
                value,
                reject,
            } => {
                let completion = if let Some(handler) = handler {
                    self.call_value(Value::Function(handler), &[value], Value::Undefined, host)
                } else if reject {
                    Err(Fault::thrown(value))
                } else {
                    Ok(value)
                };
                match completion {
                    Ok(value) => {
                        self.call_value(capability.resolve, &[value], Value::Undefined, host)?;
                    }
                    Err(error) => {
                        let reason = self.promise_exception(error)?;
                        self.call_value(capability.reject, &[reason], Value::Undefined, host)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Drain FIFO Promise/queueMicrotask jobs without resetting the host's job
    /// allowance. Ordinary callback errors and unhandled rejections are bounded
    /// diagnostics; resource/capability failures cancel pending work and return.
    pub fn microtask_checkpoint(&mut self, host: &mut dyn Host) -> Result<Vec<String>, String> {
        if self.promises.draining || self.call_depth != 0 || self.eval_depth != 0 {
            return Ok(Vec::new());
        }
        self.promises.draining = true;
        self.promises.allocation_trace = AllocationTrace::default();
        let mut diagnostics = Vec::new();
        let result = (|| {
            let mut executed = 0;
            while let Some((realm, job)) = self.promises.jobs.pop_front() {
                self.value_bytes -= job.retained_bytes() + std::mem::size_of::<Option<usize>>();
                if executed == MAX_CHECKPOINT_JOBS {
                    return Err(Fault::new(
                        "Microtask checkpoint limit reached (4096 jobs).",
                    ));
                }
                executed += 1;
                let kind = job.allocation_kind();
                let first = self.function_arena.script_created;
                self.promises.allocation_trace.active = Some((kind, first));
                self.promises.allocation_trace.started[kind] += 1;
                let result = self.run_promise_job(realm, job, host);
                self.promises.allocation_trace.active = None;
                self.promises.allocation_trace.scripts[kind] += self.function_arena.script_created.saturating_sub(first);
                if result.is_ok() { self.promises.allocation_trace.succeeded[kind] += 1; }
                if let Err(error) = result {
                    if error.exception.is_none() {
                        return Err(error);
                    }
                    if diagnostics.len() < MAX_DIAGNOSTICS {
                        diagnostics.push(format!(
                            "Microtask callback error: {}",
                            bounded_diagnostic(&error.message)
                        ));
                    }
                }
            }
            for object in &mut self.objects {
                if let Some(data) = &mut object.promise {
                    if let State::Rejected(value) = &data.state {
                        if !data.handled && !data.reported {
                            data.reported = true;
                            if diagnostics.len() < MAX_DIAGNOSTICS {
                                diagnostics.push(format!(
                                    "Unhandled Promise rejection: {}",
                                    bounded_diagnostic(&value.to_text())
                                ));
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        self.promises.draining = false;
        if result.is_err() {
            self.cancel_microtasks();
        }
        self.collect_environments();
        result.map(|()| diagnostics).map_err(Fault::display)
    }

    /// Cancel existing queued callbacks and pending observers. Promise states
    /// remain ordinary document-owned values, but removed reactions never resume.
    pub fn cancel_microtasks(&mut self) {
        for (_, job) in std::mem::take(&mut self.promises.jobs) {
            self.value_bytes -= job.retained_bytes() + std::mem::size_of::<Option<usize>>();
        }
        for object in &mut self.objects {
            if let Some(data) = &mut object.promise {
                self.value_bytes -= data.reactions.len() * std::mem::size_of::<Reaction>();
                data.reactions = Vec::new();
                data.reported = true;
            }
        }
        self.promises.reactions = 0;
    }

    #[cfg(test)]
    pub fn has_microtasks(&self) -> bool {
        !self.promises.jobs.is_empty()
            || self.objects.iter().any(|object| {
                object.promise.as_ref().is_some_and(|data| {
                    matches!(data.state, State::Rejected(_)) && !data.handled && !data.reported
                })
            })
    }
}

fn bounded_diagnostic(text: &str) -> String {
    text.chars()
        .take(512)
        .flat_map(char::escape_default)
        .take(2048)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_promise_closure_slot_dispatches_its_new_value_and_drops_old_candidate_tag() {
        let mut runtime = Runtime::new();
        runtime.execute("var holder={};holder.fn=function gone(){};holder.fn=null;0;", &mut NoHost).unwrap();
        let id = *runtime.object_assigned_closures.iter().next().unwrap();
        runtime.retire_proven_inactive_candidate(id).unwrap();
        let before = (runtime.functions.len(), runtime.function_arena.script_created);
        let closure = runtime.promise_function(Closure::FinallyValue { value: Value::Number(17.0), reject: false }).unwrap();
        assert!(matches!(closure, Value::Function(found) if found == id));
        assert!(!runtime.object_assigned_closures.contains(&id));
        assert_eq!((runtime.functions.len(), runtime.function_arena.script_created), before);
        assert_eq!(runtime.invoke(closure, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 17.0);
    }

    #[test]
    fn first_promise_closure_backing_failure_keeps_legacy_capture_charge_for_append_and_reuse() {
        for reused in [false, true] {
            let mut runtime = Runtime::new();
            if reused {
                runtime.execute("var holder={};holder.fn=function gone(){};holder.fn=null;0;", &mut NoHost).unwrap();
                let id = *runtime.object_assigned_closures.iter().next().unwrap();
                runtime.retire_proven_inactive_candidate(id).unwrap();
            }
            let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.value_bytes, runtime.function_arena.script_created);
            let closure = Closure::FinallyValue { value: Value::String("retained".into()), reject: false };
            let charge = closure.retained_bytes();
            runtime.limits.heap.objects = runtime.objects.len();
            assert!(runtime.promise_function(closure).is_err());
            assert_eq!((runtime.functions.len(), runtime.function_arena.occupied, runtime.function_arena.script_created), (before.0, before.1, before.3));
            assert_eq!(runtime.value_bytes, before.2 + charge);
            assert_eq!(runtime.functions.len(), runtime.function_properties.len());
            assert_eq!(runtime.functions.len(), runtime.function_realms.len());
            assert_eq!(runtime.functions.len(), runtime.function_initial_names.len());
        }
    }

    #[test]
    fn reachability_report_rejects_popped_or_draining_jobs_before_traversal() {
        let mut runtime = Runtime::new();
        runtime.promises.draining = true;
        assert!(runtime.persistent_allocation_report().contains("unavailable"));
        runtime.promises.draining = false;
        runtime.promises.allocation_trace.active = Some((0, 0));
        assert!(runtime.persistent_allocation_report().contains("unavailable"));
        runtime.promises.allocation_trace.active = None;
        assert!(runtime.persistent_allocation_report().contains("No collection or slot reuse"));
    }

    #[test]
    fn microtask_allocation_trace_distinguishes_jobs_and_resets_without_changing_work() {
        let mut runtime = Runtime::new();
        runtime.execute("var a,b,c;queueMicrotask(function(){a=function(){};});Promise.resolve(1).then(function(){b=function(){};});Promise.resolve({then:function(resolve){c=function(){};resolve(1);}});", &mut NoHost).unwrap();
        runtime.microtask_checkpoint(&mut NoHost).unwrap();
        let trace = &runtime.promises.allocation_trace;
        assert_eq!(trace.active, None);
        assert_eq!(trace.started, [1, 1, 1]);
        assert_eq!(trace.succeeded, [1, 1, 1]);
        assert_eq!(trace.scripts, [1, 1, 1]);
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.job_usage());
        let report = runtime.microtask_allocation_diagnostic();
        assert!(report.contains("script-created=[1, 1, 1]") && report.contains("resolving-pairs="), "{report}");
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.job_usage()), before);
        runtime.microtask_checkpoint(&mut NoHost).unwrap();
        assert_eq!(runtime.promises.allocation_trace.started, [0; 3]);
        assert_eq!(runtime.promises.allocation_trace.scripts, [0; 3]);
    }

    #[test]
    fn microtask_allocation_trace_identifies_active_fatal_job_and_restores_context() {
        for (kind, name, source) in [
            (0, "callback", "queueMicrotask(function(){for(var i=0;i<1100;i++)(function(){})();});"),
            (1, "reaction", "Promise.resolve(1).then(function(){for(var i=0;i<1100;i++)(function(){})();});"),
            (2, "thenable", "Promise.resolve({then:function(){for(var i=0;i<1100;i++)(function(){})();}});"),
        ] {
            let mut runtime = Runtime::new();
            runtime.execute(source, &mut NoHost).unwrap();
            let error = runtime.microtask_checkpoint(&mut NoHost).unwrap_err();
            assert!(error.contains("Function limit reached (1024)") && error.contains(&format!("microtasks-active=Some(\"{name}\")")), "{error}");
            let trace = &runtime.promises.allocation_trace;
            assert_eq!(trace.active, None);
            assert_eq!(trace.started.iter().sum::<usize>(), 1);
            assert_eq!(trace.started[kind], 1);
            assert_eq!(trace.succeeded, [0; 3]);
            assert!(trace.scripts[kind] > 0);
            assert!(runtime.promises.jobs.is_empty());
            assert!(!runtime.promises.draining);
            assert_eq!((runtime.active_realm, runtime.call_depth, runtime.eval_depth), (None, 0, 0));
        }
    }

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No native property reads.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native property writes.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native calls.".into())
        }
    }

    fn completed(source: &str, check: &str) {
        let mut runtime = Runtime::for_test();
        runtime.execute(source, &mut NoHost).unwrap();
        assert!(runtime
            .microtask_checkpoint(&mut NoHost)
            .unwrap()
            .is_empty());
        assert_eq!(
            runtime.execute(check, &mut NoHost).unwrap().to_text(),
            "true"
        );
        assert_eq!(runtime.call_depth, 0);
        assert_eq!(runtime.eval_depth, 0);
    }

    #[test]
    fn promise_constructor_and_resolution_have_real_identity_and_deferred_reactions() {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
var log='',saved;
var p=new Promise(function(resolve,reject){log+='executor;';saved=resolve;resolve(3);reject(8);throw 9;});
var q=p.then(function(value){log+=value+';';return value+1;});
q.then(function(value){log+=value+';';});
if(log!=='executor;'||p===q||Promise.resolve(p)!==p||!(p instanceof Promise))throw 'identity';
if(typeof p.then!=='function'||Promise.length!==1||Promise.prototype.then.length!==2)throw 'metadata';
if(Object.keys(p).length!==0||Object.prototype.toString.call(p)!=='[object Promise]')throw 'object';
"#, &mut NoHost).unwrap();
        assert!(runtime.has_microtasks());
        assert!(runtime
            .microtask_checkpoint(&mut NoHost)
            .unwrap()
            .is_empty());
        assert_eq!(
            runtime
                .execute("log==='executor;3;4;';", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn promise_thenable_lookup_is_sync_call_is_queued_and_resolution_is_once() {
        completed(
            r#"
var log='',receiver,reads=0;
var value={get then(){log+='get;';reads++;return function(resolve,reject){if(this!==value)throw 'receiver';log+='call;';resolve(7);reject(8);resolve(9);throw 10;};}};
Promise.resolve(value).then(function(v){log+=v+';';});
log+='sync;';
var original={then:3};Promise.resolve(original).then(function(v){receiver=v===original;});
"#,
            "log==='get;sync;call;7;'&&reads===1&&receiver;",
        );
    }

    #[test]
    fn promise_rejections_preserve_values_and_getter_exceptions() {
        completed(
            r#"
var first={},second={},log='',correct=0;
new Promise(function(){throw first;}).catch(function(reason){if(reason===first)correct++;});
Promise.resolve({get then(){throw second;}}).catch(function(reason){if(reason===second)correct++;});
Promise.reject(4).then().then(undefined,function(reason){log+=reason;return 5;}).then(function(value){log+=value;});
Promise.resolve(1).then(function(){throw first;}).catch(function(reason){if(reason===first)correct++;});
"#,
            "correct===3&&log==='45';",
        );
    }

    #[test]
    fn promise_self_resolution_and_chaining_cycles_reject() {
        completed(
            r#"
var resolve,seen=0;
var p=new Promise(function(r){resolve=r;});resolve(p);
p.catch(function(error){if(error.name==='TypeError')seen++;});
var q=Promise.resolve(1).then(function(){return q;});q.catch(function(error){if(error.name==='TypeError')seen++;});
"#,
            "seen===2;",
        );
    }

    #[test]
    fn promise_adoption_preserves_async_order_and_resolution_lock() {
        completed(
            r#"
var settle,log='';var source=new Promise(function(resolve){settle=resolve;});
var outer=new Promise(function(resolve,reject){resolve(source);reject('wrong');});
outer.then(function(value){log+='outer'+value+';';});
source.then(function(value){log+='source'+value+';';});
queueMicrotask(function(){log+='micro;';});settle(6);log+='sync;';
"#,
            "log==='sync;micro;source6;outer6;';",
        );
    }

    #[test]
    fn promise_finally_waits_and_preserves_or_replaces_completion() {
        completed(
            r#"
var log='',seen=0;
Promise.resolve(5).finally(function(){log+='cleanup;';return {then:function(resolve){log+='wait;';resolve(99);}};}).then(function(value){if(value===5)seen++;});
Promise.reject(6).finally(function(){return 8;}).catch(function(value){if(value===6)seen++;});
Promise.resolve(7).finally(function(){throw 9;}).catch(function(value){if(value===9)seen++;});
Promise.resolve(8).finally(4).then(function(value){if(value===8)seen++;});
Promise.reject(9).finally(function(){return Promise.reject(10);}).catch(function(value){if(value===10)seen++;});
"#,
            "seen===5&&log==='cleanup;wait;';",
        );
    }

    #[test]
    fn promise_generic_constructors_and_species_preserve_order_and_receiver() {
        completed(
            r#"
var log='',seen,returned;
function C(executor){log+='construct;';executor(function(value){log+='resolve'+value+';';},function(reason){log+='reject'+reason+';';});this.custom=true;}
var result=Promise.resolve.call(C,3);if(!result.custom)throw 'generic resolve';
var p=Promise.resolve(4);p.constructor={get [Symbol.species](){log+='species;';return C;}};
returned=p.then(function(value){log+='handler;';return value+1;});
if(!returned.custom||Promise[Symbol.species]!==Promise)throw 'species';
var getter=Object.getOwnPropertyDescriptor(Promise,Symbol.species).get;if(getter.call(C)!==C)throw 'getter receiver';
Number.prototype.then=function(a,b){seen=this.valueOf();return b;};
if(Promise.prototype.catch.call(3,9)!==9)throw 'generic catch';
"#,
            "seen===3&&log==='construct;resolve3;species;construct;handler;resolve5;';",
        );
    }

    #[test]
    fn promise_constructor_and_method_errors_are_catchable_and_ordered() {
        completed(
            r#"
var seen=0;
try{Promise(function(){});}catch(e){if(e.name==='TypeError')seen++;}
try{new Promise(3);}catch(e){if(e.name==='TypeError')seen++;}
try{Promise.prototype.then.call({});}catch(e){if(e.name==='TypeError')seen++;}
try{queueMicrotask(3);}catch(e){if(e.name==='TypeError')seen++;}
try{Promise.resolve.call(3,4);}catch(e){if(e.name==='TypeError')seen++;}
try{Promise.resolve.call(()=>0,4);}catch(e){if(e.name==='TypeError')seen++;}
var p=Promise.resolve(2);p.constructor=3;try{p.then();}catch(e){if(e.name==='TypeError')seen++;}
function C(executor){executor(undefined,undefined);executor(function(){},function(){});}
if(!(Promise.resolve.call(C,2) instanceof C))throw 'undefined capability values';
"#,
            "seen===7;",
        );
    }

    #[test]
    fn promise_queue_order_nested_jobs_and_unhandled_reporting_are_bounded() {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
var log='';
queueMicrotask(function(){log+='a';queueMicrotask(function(){log+='d';});throw 'oops';});
Promise.resolve(1).then(function(){log+='b';});queueMicrotask(function(){log+='c';});
Promise.reject('unhandled');var handled=Promise.reject('handled');queueMicrotask(function(){handled.catch(function(){});});
"#,&mut NoHost).unwrap();
        let messages = runtime.microtask_checkpoint(&mut NoHost).unwrap();
        assert_eq!(messages.len(), 2, "{messages:?}");
        assert!(messages[0].contains("oops"));
        assert!(messages[1].contains("unhandled"));
        assert!(runtime
            .microtask_checkpoint(&mut NoHost)
            .unwrap()
            .is_empty());
        assert_eq!(
            runtime
                .execute("log==='abcd';", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn promise_cancellation_removes_queued_and_pending_reactions() {
        let mut runtime = Runtime::for_test();
        runtime.execute("var log='',resolve;var p=new Promise(function(r){resolve=r;});p.then(function(){log+='old';});queueMicrotask(function(){log+='queued';});",&mut NoHost).unwrap();
        runtime.cancel_microtasks();
        runtime.begin_job();
        runtime
            .execute(
                "resolve(3);Promise.resolve(1).then(function(){log+='new';});",
                &mut NoHost,
            )
            .unwrap();
        assert!(runtime
            .microtask_checkpoint(&mut NoHost)
            .unwrap()
            .is_empty());
        assert_eq!(
            runtime
                .execute("log==='new';", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn promise_fatal_faults_cancel_callbacks_without_becoming_rejections() {
        let mut runtime = Runtime::for_test();
        runtime.execute("var caught=false,finalized=false,called=false;Promise.resolve(1).then(function(){try{while(true){}}catch(e){caught=true;}finally{finalized=true;}}).catch(function(){caught=true;});queueMicrotask(function(){called=true;});",&mut NoHost).unwrap();
        assert!(runtime
            .microtask_checkpoint(&mut NoHost)
            .unwrap_err()
            .contains("limit"));
        assert!(!runtime.has_microtasks());
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!caught&&!finalized&&!called;", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
        runtime
            .execute("queueMicrotask(function(){called=true;});", &mut NoHost)
            .unwrap();
        assert!(runtime
            .execute("while(true){};", &mut NoHost)
            .unwrap_err()
            .contains("limit"));
        assert!(!runtime.has_microtasks());
    }

    #[test]
    fn promise_microtask_queue_and_checkpoint_limits_remain_fatal() {
        for source in [
            "var called=false;function f(){called=true;}try{for(var i=0;i<1025;i++)queueMicrotask(f);}catch(e){called=true;}finally{called=true;}",
            "var called=false;function f(){queueMicrotask(f);}queueMicrotask(f);"
        ] {
            let mut runtime=Runtime::for_test();
            let error=match runtime.execute(source,&mut NoHost) { Err(error)=>error,Ok(_)=>runtime.microtask_checkpoint(&mut NoHost).unwrap_err() };
            assert!(error.contains("limit"),"{error}");
            assert!(!runtime.has_microtasks());
            runtime.begin_job();
            assert_eq!(runtime.execute("!called;",&mut NoHost).unwrap().to_text(),"true");
        }
    }

    #[test]
    fn promise_host_assimilation_gate_runs_before_native_lookup() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("opaque", Value::Host(7)).unwrap();
        let error=runtime.execute("var called=false;try{Promise.resolve(opaque);}catch(e){called=true;}finally{called=true;}",&mut NoHost).unwrap_err();
        assert!(
            error.contains("Native host Promise assimilation"),
            "{error}"
        );
        runtime.begin_job();
        assert_eq!(
            runtime.execute("!called;", &mut NoHost).unwrap().to_text(),
            "true"
        );
    }

    #[test]
    fn promise_retained_strings_are_accounted_and_reaction_storage_is_released() {
        let mut runtime = Runtime::for_test();
        let before = runtime.value_bytes;
        runtime
            .execute(
                "var p=Promise.resolve('hello');p.then(function(value){return value+'!';});",
                &mut NoHost,
            )
            .unwrap();
        assert!(runtime.value_bytes > before);
        let queued = runtime.value_bytes;
        assert!(runtime
            .microtask_checkpoint(&mut NoHost)
            .unwrap()
            .is_empty());
        assert!(runtime.value_bytes < queued);
        assert_eq!(runtime.promises.reactions, 0);
        assert!(runtime.promises.jobs.is_empty());
    }
    #[test]
    fn promise_all_preserves_input_order_and_dense_holes() {
        completed(
            r#"
var first=Promise.withResolvers(),second=Promise.withResolvers(),out,holes;
Promise.all([first.promise,2,second.promise]).then(function(values){out=values.join(',');});
Promise.all([,4]).then(function(values){holes=values.length===2&&Object.hasOwn(values,'0')&&values[0]===undefined;});
second.resolve(3);first.resolve(1);
"#,
            "out==='1,2,3'&&holes;",
        );
    }

    #[test]
    fn promise_race_and_empty_combinators_keep_their_settlement_rules() {
        completed(
            r#"
var log='',empty=false;
Promise.race([]).then(function(){empty=true;},function(){empty=true;});
Promise.all([]).then(function(values){log+='all'+values.length+';';});
Promise.allSettled([]).then(function(values){log+='settled'+values.length+';';});
Promise.any([]).catch(function(error){if(!(error instanceof AggregateError)||!(error instanceof Error))throw 'error prototype';log+='any'+error.errors.length+';';});
Promise.race([Promise.reject(3),4]).catch(function(reason){log+='race'+reason+';';});
"#,
            "!empty&&log==='all0;settled0;any0;race3;';",
        );
    }

    #[test]
    fn promise_all_settled_and_any_keep_values_errors_and_input_order() {
        completed(
            r#"
var a=Promise.withResolvers(),b=Promise.withResolvers(),reason={},settled,errors,winner;
Promise.allSettled([a.promise,b.promise,3]).then(function(values){settled=values[0].status==='rejected'&&values[0].reason===reason&&values[1].status==='fulfilled'&&values[1].value===2&&values[2].value===3;});
Promise.any([Promise.reject(1),Promise.reject(2)]).catch(function(error){errors=error.name==='AggregateError'&&error.errors.join(',')==='1,2'&&!Object.getOwnPropertyDescriptor(error,'errors').enumerable;});
Promise.any([Promise.reject(4),5]).then(function(value){winner=value;});b.resolve(2);a.reject(reason);
"#,
            "settled&&errors&&winner===5;",
        );
    }

    #[test]
    fn promise_combinators_cache_resolve_and_guard_misbehaving_callbacks() {
        completed(
            r#"
var gets=0,receivers=0;
function C(executor){var self=this;executor(function(value){self.value=value;},function(reason){self.reason=reason;});}
Object.defineProperty(C,'resolve',{configurable:true,get:function(){gets++;return function(value){if(this===C)receivers++;return {then:function(resolve,reject){resolve(value);resolve(99);reject(98);}};};}});
var all=Promise.all.call(C,[1,2]);
if(all.value.join(',')!=='1,2')throw 'all duplicate';
var settled=Promise.allSettled.call(C,[3]);
if(settled.value[0].status!=='fulfilled'||settled.value[0].value!==3)throw 'shared duplicate';
Object.defineProperty(C,'resolve',{value:function(value){return {then:function(resolve,reject){reject(value);reject(88);}};}});
var any=Promise.any.call(C,[4,5]);if(any.reason.errors.join(',')!=='4,5')throw 'any duplicate';
"#,
            "gets===2&&receivers===3;",
        );
    }

    #[test]
    fn promise_combinator_abrupt_consumers_close_active_iterators_only() {
        completed(
            r#"
var closed=0,seen=0,reason={};
function C(executor){return new Promise(executor);}C.resolve=function(){throw reason;};
var iterable={};iterable[Symbol.iterator]=function(){return {next:function(){return {value:1,done:false};},return:function(){closed++;return {};}};};
Promise.all.call(C,iterable).catch(function(value){if(value===reason)seen++;});
var failing={};failing[Symbol.iterator]=function(){return {next:function(){throw reason;},return:function(){closed+=100;return {};}};};
Promise.race(failing).catch(function(value){if(value===reason)seen++;});
Promise.all(3).catch(function(error){if(error.name==='TypeError')seen++;});
"#,
            "closed===1&&seen===3;",
        );
    }

    #[test]
    fn promise_with_resolvers_and_aggregate_error_have_real_properties() {
        completed(
            r#"
var pair=Promise.withResolvers(),seen;
if(Object.keys(pair).join(',')!=='promise,resolve,reject'||Promise.withResolvers.length!==0)throw 'record';
pair.promise.then(function(value){seen=value;});pair.resolve(8);pair.reject(9);
var log='',cause={};var errors={};errors[Symbol.iterator]=function(){log+='iterate;';return [1,2].values();};
var message={toString:function(){log+='message;';return 'bad';}};
var options={get cause(){log+='cause;';return cause;}};
var error=new AggregateError(errors,message,options);
if(error.errors.join(',')!=='1,2'||error.message!=='bad'||error.cause!==cause||!(error instanceof AggregateError)||!(error instanceof Error))throw 'fields';
if(AggregateError.length!==2||Object.getPrototypeOf(AggregateError)!==Error||Object.keys(error).length!==0)throw 'metadata';
"#,
            "seen===8&&log==='message;cause;iterate;';",
        );
    }

    #[test]
    fn promise_try_follows_current_callback_order_and_resolve_identity() {
        completed(
            r#"
var log='',seen=0;
function C(executor){log+='constructor;';executor(function(value){log+='resolve'+value+';';},function(reason){log+='reject'+reason+';';});}
Promise.try.call(C,function(a,b){'use strict';if(this!==undefined)throw 'receiver';log+='callback;';return a+b;},2,3);
var p=Promise.resolve(8);if(Promise.try(function(){return p;})!==p)throw 'identity';
Promise.try(function(){throw 6;}).catch(function(reason){if(reason===6)seen++;});
Promise.try(3).catch(function(error){if(error.name==='TypeError')seen++;});
var order='';try{Promise.try.call({},function(){order+='callback';return 3;});}catch(e){if(e.name==='TypeError')order+='error';}
if(order!=='callbackerror')throw 'invalid constructor order';
"#,
            "seen===2&&log==='callback;constructor;resolve5;';",
        );
    }

    #[test]
    fn promise_combinators_and_try_do_not_catch_resource_or_host_failures() {
        for source in [
            "var called=false;try{Promise.all(opaque).catch(function(){called=true;});}catch(e){called=true;}finally{called=true;}",
            "var called=false;try{Promise.try(function(){while(true){}}).catch(function(){called=true;});}catch(e){called=true;}finally{called=true;}",
            "var called=false;function C(executor){executor(function(){called=true;},function(){called=true;});}C.resolve=function(value){return {then:function(){}};};try{Promise.all.call(C,Array(4097));}catch(e){called=true;}finally{called=true;}"
        ] {
            let mut runtime=Runtime::for_test();runtime.define_global("opaque",Value::Host(4)).unwrap();
            let error=runtime.execute(source,&mut NoHost).unwrap_err();
            assert!(error.contains("limit")||error.contains("Host-object iteration"),"{error}");
            runtime.begin_job();assert_eq!(runtime.execute("!called;",&mut NoHost).unwrap().to_text(),"true");
        }
    }
}
