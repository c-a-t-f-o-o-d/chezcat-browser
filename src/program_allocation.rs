//! Bounded numeric attribution of existing retained-program guards.
//! No sharing/reclamation, source output, callbacks or quota changes.
use super::*;

impl Runtime {
    pub(super) fn program_limit_fault(
        &self,
        source: &str,
        nodes: Option<usize>,
        kind: &'static str,
        input_strict: Option<bool>,
        ast: bool,
    ) -> Fault {
        let mut groups = BTreeMap::<&str, (usize, usize)>::new();
        let mut registry_only = 0usize;
        let mut requested_matches = 0usize;
        let mut dynamic = 0usize;
        for (index, program) in self.programs.iter().enumerate().take(MAX_PROGRAMS) {
            let group = groups.entry(program.source.as_str()).or_insert((index, 0));
            group.1 += 1;
            if program.source == source {
                requested_matches += 1;
            }
            if Rc::strong_count(program) == 1 {
                registry_only += 1;
            }
            if program
                .templates
                .iter()
                .any(|template| template.dynamic_source)
            {
                dynamic += 1;
            }
        }
        let unique = groups.len();
        let mut repeated: Vec<_> = groups
            .into_iter()
            .filter(|(_, (_, count))| *count > 1)
            .map(|(source, (first, count))| (first, count, source.len()))
            .collect();
        repeated.sort_unstable_by(|left, right| {
            right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
        });
        repeated.truncate(3);
        let count_cap = self.programs.len() >= MAX_PROGRAMS;
        let source_cap = self.retained_source.saturating_add(source.len()) > MAX_RETAINED_SOURCE;
        let ast_cap =
            nodes.is_some_and(|nodes| self.retained_ast.saturating_add(nodes) > MAX_RETAINED_AST);
        let prefix = if ast {
            "Retained AST limit reached (262144 nodes)."
        } else {
            "Retained script limit reached (128 scripts / 2 MiB source)."
        };
        Fault::new(format!("{prefix} Count/source/AST-cap={count_cap}/{source_cap}/{ast_cap}; programs={}; source-bytes={}; AST-nodes={}; regexp-accounting-bytes={}; requested-kind={kind}; requested-input-strict={input_strict:?}; requested-bytes={}; requested-nodes={nodes:?}; active-native={}; exact-source-groups={unique}; requested-exact-source-occurrences={requested_matches}; registry-only={registry_only}; dynamic-template-programs={dynamic}; top-duplicate-first-index/count/bytes={repeated:?}. Exact source equality is attribution, not proof of a reusable compilation context.",
            self.programs.len(), self.retained_source, self.retained_ast, self.retained_regexp, source.len(), self.eval_depth))
    }
}

#[test]
fn retained_program_attribution_is_numeric_and_preserves_existing_guards_and_work() {
    let source = "var PrivateSourcePayload = 1;";
    let mut runtime = Runtime::new();
    for _ in 0..MAX_PROGRAMS {
        runtime.register_program(source, false).unwrap();
    }
    let before = (
        runtime.programs.len(),
        runtime.retained_source,
        runtime.retained_ast,
        runtime.retained_regexp,
        runtime.copy_bytes,
        runtime.steps,
        runtime.job_deadline,
    );
    let fault = runtime.register_program(source, false).err().unwrap();
    assert!(
        fault
            .message
            .contains("Count/source/AST-cap=true/false/false"),
        "{}",
        fault.message
    );
    assert!(
        fault.message.contains("exact-source-groups=1")
            && fault
                .message
                .contains("requested-exact-source-occurrences=128")
    );
    assert!(
        fault.message.contains("registry-only=128")
            && fault.message.contains("requested-input-strict=Some(false)")
    );
    assert!(!fault.message.contains("PrivateSourcePayload"));
    assert_eq!(
        (
            runtime.programs.len(),
            runtime.retained_source,
            runtime.retained_ast,
            runtime.retained_regexp,
            runtime.copy_bytes,
            runtime.steps,
            runtime.job_deadline
        ),
        (
            before.0,
            before.1,
            before.2,
            before.3,
            before.4,
            before.5 - 1,
            before.6
        )
    );
    for ast in [false, true] {
        let mut runtime = Runtime::new();
        if ast {
            runtime.retained_ast = MAX_RETAINED_AST;
        } else {
            runtime.retained_source = MAX_RETAINED_SOURCE;
        }
        let fault = runtime.register_program("42;", false).err().unwrap();
        assert!(
            fault.message.contains(if ast {
                "Count/source/AST-cap=false/false/true"
            } else {
                "Count/source/AST-cap=false/true/false"
            }),
            "{}",
            fault.message
        );
    }
}
