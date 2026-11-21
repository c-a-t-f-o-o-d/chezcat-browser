//! Audited native leaves retain dispatch guards without redundant input edges.
use super::*;
struct Leaf {
    enabled: bool,
    calls: usize,
}
impl Host for Leaf {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("no property read")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("no property write")
    }
    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        assert_eq!(object, 900);
        assert_eq!(method, "\0Node.get.firstChild");
        assert!(args.is_empty());
        self.calls += 1;
        Ok(Value::Host(901))
    }
    fn native_leaf_getter(&self, object: u32, method: &str) -> bool {
        self.enabled && object == 900 && method == "\0Node.get.firstChild"
    }
}
fn fixture() -> (Runtime, Value, Leaf) {
    let mut runtime = Runtime::with_limits(limits::BROWSER);
    let function = runtime.host_method(900, "\0Node.get.firstChild").unwrap();
    runtime.begin_job();
    (
        runtime,
        function,
        Leaf {
            enabled: true,
            calls: 0,
        },
    )
}
#[test]
fn native_leaf_getter_needs_no_new_arena_edge_and_restores_call_registration() {
    let (mut runtime, function, mut host) = fixture();
    let before = runtime.root_storage_bytes();
    runtime.limits.heap.retained_values = 0;
    assert!(matches!(
        runtime
            .call_value(function, &[], Value::Host(900), &mut host)
            .unwrap(),
        Value::Host(901)
    ));
    assert_eq!(host.calls, 1);
    assert_eq!(runtime.root_storage_bytes(), before);
    assert!(runtime.transient_registration_is_empty());
}
#[test]
fn native_leaf_getter_opt_out_extra_arguments_and_non_native_receivers_keep_roots() {
    for mode in ["optout", "arguments", "receiver"] {
        let (mut runtime, function, mut host) = fixture();
        let object = Value::Object(runtime.new_object(None).unwrap());
        if mode == "optout" {
            host.enabled = false;
        }
        runtime.limits.heap.retained_values = 0;
        let args = if mode == "arguments" {
            vec![object.clone()]
        } else {
            Vec::new()
        };
        let receiver = if mode == "receiver" {
            object
        } else {
            Value::Host(900)
        };
        let error = runtime
            .call_value(function, &args, receiver, &mut host)
            .unwrap_err();
        assert!(
            error.message.contains("Transient root-reference"),
            "{mode}: {}",
            error.message
        );
        assert_eq!(host.calls, 0);
        assert!(runtime.transient_registration_is_empty());
    }
}
#[test]
fn native_leaf_getter_keeps_receiver_step_and_deadline_guards_and_recovers() {
    for mode in ["receiver", "steps", "deadline"] {
        let (mut runtime, function, mut host) = fixture();
        if mode == "steps" {
            runtime.steps = 0;
        }
        if mode == "deadline" {
            runtime.steps = 256;
            runtime.job_deadline = std::time::Instant::now() - std::time::Duration::from_secs(1);
        }
        let receiver = if mode == "receiver" { 901 } else { 900 };
        let error = runtime
            .call_value(function.clone(), &[], Value::Host(receiver), &mut host)
            .unwrap_err();
        assert!(
            match mode {
                "receiver" => error.message.contains("receiver"),
                "steps" => error.message.contains("step limit"),
                _ => error.message.contains("time limit"),
            },
            "{mode}: {}",
            error.message
        );
        assert_eq!(host.calls, 0);
        assert!(runtime.transient_registration_is_empty());
        runtime.begin_job();
        assert!(matches!(
            runtime
                .call_value(function, &[], Value::Host(900), &mut host)
                .unwrap(),
            Value::Host(901)
        ));
        assert_eq!(host.calls, 1);
        assert!(runtime.transient_registration_is_empty());
    }
}
