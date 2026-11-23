//! Original resource and native-slot checks for protocol parser no-ops.
use super::*;

const ORIGINAL: &str = "https://u:p@example.test:81/p?x=a%20b&slash=%2f&tilde=~&x=two#kept";
const CALLBACK_HREF: &str = "https://u:p@example.test:81/p?live=a%20b&slash=%2f#kept";
const PUBLISHED_HREF: &str = "https://side.test/a?k=%7e";

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("Unexpected native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("Unexpected native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("Unexpected native call") }
}

fn initialized_runtime() -> Runtime {
    let mut runtime = Runtime::for_test();
    runtime.binding_value(runtime.global_environment, "URL").unwrap();
    runtime.binding_value(runtime.global_environment, "URLSearchParams").unwrap();
    runtime
}

fn object_id(value: Value) -> usize {
    let Value::Object(id) = value else { panic!("Expected a genuine object") };
    id
}

fn attached_runtime(href: &str) -> (Runtime, usize, usize) {
    let mut runtime = initialized_runtime();
    let url = object_id(runtime.url_construct(
        Kind::Url, &[Value::String(href.into())], &mut NoHost,
    ).unwrap());
    let params = runtime.url_record(url).unwrap().1;
    runtime.begin_job();
    (runtime, url, params)
}

#[derive(Debug, PartialEq, Eq)]
struct SlotState {
    url: usize,
    href: String,
    query: Option<String>,
    pairs: Vec<Pair>,
    pair_capacity: usize,
    params: usize,
    owner: Option<usize>,
    url_bytes: usize,
    params_bytes: usize,
    retained: usize,
    objects: usize,
    properties: usize,
    native_slots: usize,
}

fn slot_state(runtime: &Runtime, url: usize, params: usize) -> SlotState {
    let (value, attached) = runtime.url_record(url).unwrap();
    let (pairs, owner) = runtime.params_data(params).unwrap();
    SlotState {
        url,
        href: value.href(),
        query: value.query.clone(),
        pairs: pairs.clone(),
        pair_capacity: pairs.capacity(),
        params: attached,
        owner,
        url_bytes: runtime.url_data(url).unwrap().bytes(),
        params_bytes: runtime.url_data(params).unwrap().bytes(),
        retained: runtime.value_bytes,
        objects: runtime.objects.len(),
        properties: runtime.properties,
        native_slots: runtime.objects.iter().filter(|object| object.url.is_some()).count(),
    }
}

fn idle(runtime: &Runtime) {
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

fn fatal(error: Fault, expected: &str) {
    assert!(error.exception.is_none(), "Expected resource/capability refusal: {error:?}");
    assert!(error.message.contains(expected), "{error:?}");
}

fn set_protocol(runtime: &mut Runtime, url: usize, source: &Value) -> Result<Value, Fault> {
    runtime.url_builtin(Op::Set("protocol"), std::slice::from_ref(source), Value::Object(url), &mut NoHost)
}

fn protocol_success(runtime: &mut Runtime, url: usize, source: &Value) {
    assert!(matches!(set_protocol(runtime, url, source).unwrap(), Value::Undefined));
}

#[test]
fn url_protocol_noop_keeps_exact_slots_and_accounting() {
    let (mut runtime, url, params) = attached_runtime(ORIGINAL);
    let before = slot_state(&runtime, url, params);
    for source in ["custom:ignored", "1http:ignored", "file:ignored"] {
        for _ in 0..2 {
            protocol_success(&mut runtime, url, &Value::String(source.into()));
            assert_eq!(slot_state(&runtime, url, params), before);
            idle(&runtime);
        }
    }
}

#[test]
fn url_protocol_noop_full_scalar_input_limit_precedes_cleanup_and_prefix() {
    let cases: [(&str, &str); 3] = [("custom:", "x"), ("file:", "\n"), ("1http:", "\t")];
    for (prefix, fill) in cases {
        let exact = format!("{prefix}{}", fill.repeat(MAX_URL_BYTES - prefix.len()));
        assert_eq!(exact.len(), MAX_URL_BYTES);
        let (mut runtime, url, params) = attached_runtime(ORIGINAL);
        let before = slot_state(&runtime, url, params);
        let oversized = Value::String(format!("{exact}{fill}"));
        fatal(set_protocol(&mut runtime, url, &oversized).unwrap_err(), "URL length limit");
        assert_eq!(slot_state(&runtime, url, params), before);
        idle(&runtime);
        runtime.begin_job();
        protocol_success(&mut runtime, url, &Value::String(exact));
        assert_eq!(slot_state(&runtime, url, params), before);
        idle(&runtime);
    }
}

#[test]
fn url_protocol_noop_full_usv_input_limit_includes_ignored_suffix() {
    let prefix = "custom:";
    let replacements = (MAX_URL_BYTES - prefix.len()) / 3;
    let padding = (MAX_URL_BYTES - prefix.len()) % 3;
    let mut units: Vec<u16> = prefix.encode_utf16().collect();
    units.extend(std::iter::repeat(0xd800u16).take(replacements));
    units.extend(std::iter::repeat(b'a' as u16).take(padding));
    assert_eq!(prefix.len() + replacements * 3 + padding, MAX_URL_BYTES);
    assert!(units.len() * 2 < MAX_URL_BYTES);
    let exact = Value::StringUnits(units.clone());
    units.push(b'a' as u16);
    let (mut runtime, url, params) = attached_runtime(ORIGINAL);
    let before = slot_state(&runtime, url, params);
    fatal(set_protocol(&mut runtime, url, &Value::StringUnits(units)).unwrap_err(), "URL length limit");
    assert_eq!(slot_state(&runtime, url, params), before);
    idle(&runtime);
    runtime.begin_job();
    protocol_success(&mut runtime, url, &exact);
    assert_eq!(slot_state(&runtime, url, params), before);
    idle(&runtime);
}

fn measured_noop(source: &Value) -> (usize, usize) {
    let (mut runtime, url, params) = attached_runtime(ORIGINAL);
    let before = slot_state(&runtime, url, params);
    let budget = (runtime.copy_bytes, runtime.steps);
    protocol_success(&mut runtime, url, source);
    assert_eq!(slot_state(&runtime, url, params), before);
    idle(&runtime);
    (budget.0 - runtime.copy_bytes, budget.1 - runtime.steps)
}

#[test]
fn url_protocol_noop_measured_work_and_step_refusals_recover() {
    let source = Value::String(format!("custom:{}", "ignored".repeat(96)));
    let (copies, steps) = measured_noop(&source);
    let (short_copies, _) = measured_noop(&Value::String("custom:".into()));
    assert!(copies > short_copies, "An ignored suffix still consumes input work");
    assert!(steps > 0);
    for copy_limit in [true, false] {
        let (mut runtime, url, params) = attached_runtime(ORIGINAL);
        let before = slot_state(&runtime, url, params);
        let deadline = runtime.job_deadline;
        if copy_limit { runtime.copy_bytes = copies - 1; } else { runtime.steps = steps - 1; }
        fatal(set_protocol(&mut runtime, url, &source).unwrap_err(),
            if copy_limit { "String/copy work limit" } else { "Execution step limit" });
        assert_eq!(if copy_limit { runtime.copy_bytes } else { runtime.steps }, 0);
        assert_eq!(runtime.job_deadline, deadline);
        assert_eq!(slot_state(&runtime, url, params), before);
        idle(&runtime);
        runtime.begin_job();
        if copy_limit { runtime.copy_bytes = copies; } else { runtime.steps = steps; }
        protocol_success(&mut runtime, url, &source);
        assert_eq!(if copy_limit { runtime.copy_bytes } else { runtime.steps }, 0);
        assert_eq!(slot_state(&runtime, url, params), before);
        idle(&runtime);
    }
}

#[test]
fn url_protocol_noop_needs_no_retained_or_identity_headroom() {
    let (mut runtime, url, params) = attached_runtime(ORIGINAL);
    let before = slot_state(&runtime, url, params);
    let limits = runtime.limits.heap;
    runtime.limits.heap.retained_values = before.retained;
    runtime.limits.heap.objects = before.objects;
    runtime.limits.heap.properties = before.properties;
    for source in ["custom:ignored", "file:ignored"] {
        protocol_success(&mut runtime, url, &Value::String(source.into()));
        assert_eq!(slot_state(&runtime, url, params), before);
    }
    // Exhausted headroom does not turn an actual special transition into a no-op.
    for source in ["ftp:", "ws:", "wss:"] {
        fatal(set_protocol(&mut runtime, url, &Value::String(source.into())).unwrap_err(), "only HTTP(S)");
        assert_eq!(slot_state(&runtime, url, params), before);
    }
    let grown = "x".repeat(before.url_bytes + before.params_bytes);
    let search = Value::String(format!("?grown={grown}"));
    fatal(runtime.url_builtin(Op::Set("search"), std::slice::from_ref(&search),
        Value::Object(url), &mut NoHost).unwrap_err(), "Retained value limit");
    assert_eq!(slot_state(&runtime, url, params), before);
    idle(&runtime);
    runtime.limits.heap = limits;
    runtime.begin_job();
    runtime.url_builtin(Op::Set("search"), &[search], Value::Object(url), &mut NoHost).unwrap();
    let after = slot_state(&runtime, url, params);
    assert_eq!(after.pairs, vec![("grown".into(), grown)]);
    assert_eq!((after.params, after.owner), (params, Some(url)));
    assert_eq!((after.objects, after.properties, after.native_slots),
        (before.objects, before.properties, before.native_slots));
    assert!(after.url_bytes + after.params_bytes > before.url_bytes + before.params_bytes);
    assert_eq!(after.retained, before.retained - before.url_bytes - before.params_bytes
        + after.url_bytes + after.params_bytes);
    idle(&runtime);

    let (mut unguarded, url, params) = attached_runtime("https://example.test/p?x=%2f#kept");
    let before = slot_state(&unguarded, url, params);
    unguarded.limits.heap.retained_values = before.retained;
    unguarded.limits.heap.objects = before.objects;
    unguarded.limits.heap.properties = before.properties;
    fatal(set_protocol(&mut unguarded, url, &Value::String("file:".into())).unwrap_err(), "only HTTP(S)");
    assert_eq!(slot_state(&unguarded, url, params), before);
    idle(&unguarded);
}

struct CallbackFixture {
    runtime: Runtime,
    url: usize,
    params: usize,
    source: Value,
}

fn binding(runtime: &mut Runtime, name: &str) -> Value {
    runtime.binding_value(runtime.global_environment, name).unwrap()
}

fn callback_fixture(output: &str) -> CallbackFixture {
    // These fixtures insert only an ASCII scheme prefix and an ASCII suffix.
    assert!(output.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b':'));
    let mut runtime = initialized_runtime();
    runtime.execute(&format!(r#"
var protocolURL=new URL('{ORIGINAL}'),protocolQuery=protocolURL.searchParams;
var protocolCalls=0,protocolPublished=null,protocolText='{output}';
var protocolSource={{toString:function(){{
 protocolCalls++;protocolURL.search='?live=a%20b&slash=%2f';
 protocolPublished=new URL('{PUBLISHED_HREF}');return protocolText;
}}}};
"#), &mut NoHost).unwrap();
    let url = object_id(binding(&mut runtime, "protocolURL"));
    let params = runtime.url_record(url).unwrap().1;
    let source = binding(&mut runtime, "protocolSource");
    runtime.begin_job();
    CallbackFixture { runtime, url, params, source }
}

fn control_conversion(fixture: &mut CallbackFixture) -> String {
    // url_builtin consumes one entry step and raises call_depth once around
    // url_string and url_set. url_set itself does not change callback depth.
    fixture.runtime.step().unwrap();
    fixture.runtime.call_depth += 1;
    let converted = fixture.runtime.url_string(&fixture.source, &mut NoHost);
    fixture.runtime.call_depth -= 1;
    converted.unwrap()
}

fn publication_state(fixture: &mut CallbackFixture, native_slots: usize) -> SlotState {
    let runtime = &mut fixture.runtime;
    assert_eq!(binding(runtime, "protocolCalls").to_number(), 1.0);
    assert_eq!(object_id(binding(runtime, "protocolQuery")), fixture.params);
    let state = slot_state(runtime, fixture.url, fixture.params);
    assert_eq!(state.href, CALLBACK_HREF);
    assert_eq!(state.query.as_deref(), Some("live=a%20b&slash=%2f"));
    assert_eq!(state.pairs, vec![("live".into(), "a b".into()), ("slash".into(), "/".into())]);
    assert_eq!((state.params, state.owner), (fixture.params, Some(fixture.url)));
    assert_eq!(state.native_slots, native_slots + 2);
    let published = object_id(binding(runtime, "protocolPublished"));
    let params = runtime.url_record(published).unwrap().1;
    let published = slot_state(runtime, published, params);
    assert_eq!(published.href, PUBLISHED_HREF);
    assert_eq!(published.pairs, vec![("k".into(), "~".into())]);
    assert_eq!(published.owner, Some(published.url));
    idle(runtime);
    published
}

fn assert_matched_publications(candidate: &mut CallbackFixture, control: &mut CallbackFixture, native_slots: usize) {
    assert_eq!(publication_state(candidate, native_slots), publication_state(control, native_slots));
    assert_eq!(slot_state(&candidate.runtime, candidate.url, candidate.params),
        slot_state(&control.runtime, control.url, control.params),
        "A failed outer setter must retain exactly the callback's URL/query publications and metadata");
}

#[test]
fn url_protocol_noop_late_work_refusal_keeps_callback_publications() {
    let output = format!("custom:{}", "suffix".repeat(64));
    let mut measured = callback_fixture(&output);
    let before = slot_state(&measured.runtime, measured.url, measured.params);
    let available = measured.runtime.copy_bytes;
    protocol_success(&mut measured.runtime, measured.url, &measured.source);
    let required = available - measured.runtime.copy_bytes;

    let mut control = callback_fixture(&output);
    assert_eq!(slot_state(&control.runtime, control.url, control.params), before);
    let available = control.runtime.copy_bytes;
    assert_eq!(control_conversion(&mut control), output);
    let conversion_work = available - control.runtime.copy_bytes;
    assert!(required > conversion_work, "The full setter must charge work after author conversion");

    let mut candidate = callback_fixture(&output);
    assert_eq!(slot_state(&candidate.runtime, candidate.url, candidate.params), before);
    candidate.runtime.copy_bytes = required - 1;
    let deadline = candidate.runtime.job_deadline;
    fatal(set_protocol(&mut candidate.runtime, candidate.url, &candidate.source).unwrap_err(), "String/copy work limit");
    assert_eq!(candidate.runtime.copy_bytes, 0);
    assert_eq!(candidate.runtime.job_deadline, deadline);
    assert_matched_publications(&mut candidate, &mut control, before.native_slots);
    let after = slot_state(&candidate.runtime, candidate.url, candidate.params);
    candidate.runtime.begin_job();
    protocol_success(&mut candidate.runtime, candidate.url, &Value::String("custom:recovered".into()));
    assert_eq!(slot_state(&candidate.runtime, candidate.url, candidate.params), after);
    idle(&candidate.runtime);
}

#[test]
fn url_protocol_noop_late_length_refusal_keeps_callback_publications() {
    let prefix = "custom:";
    let output = format!("{prefix}{}", "x".repeat(MAX_URL_BYTES + 1 - prefix.len()));
    assert_eq!(output.len(), MAX_URL_BYTES + 1);
    let mut candidate = callback_fixture(&output);
    let before = slot_state(&candidate.runtime, candidate.url, candidate.params);
    let mut control = callback_fixture(&output);
    assert_eq!(slot_state(&control.runtime, control.url, control.params), before);
    assert_eq!(control_conversion(&mut control), output);
    fatal(set_protocol(&mut candidate.runtime, candidate.url, &candidate.source).unwrap_err(), "URL length limit");
    assert_matched_publications(&mut candidate, &mut control, before.native_slots);
    let after = slot_state(&candidate.runtime, candidate.url, candidate.params);
    candidate.runtime.begin_job();
    protocol_success(&mut candidate.runtime, candidate.url, &Value::String("file:recovered".into()));
    assert_eq!(slot_state(&candidate.runtime, candidate.url, candidate.params), after);
    idle(&candidate.runtime);
}
