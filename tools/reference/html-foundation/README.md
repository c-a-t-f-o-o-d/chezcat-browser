# Native foundation cohort

This is the development-only F00 input/selection lock for the
[document foundation plan](../../../docs/HTML_FOUNDATION_PLAN.md). Production
algorithms remain original Rust/std-only. Upstream corpus bytes stay in an
explicit external cache and are never downloaded by the runner or shipped in a
runtime package.

The inputs.json lock records repository revisions, paths, Git blobs, byte lengths
and SHA-256 digests, including licenses, data format documents and the selected
F01 browser fixtures' static dependencies. WPT uses its 3-clause BSD license;
html5lib-tests uses its MIT license. The full pinned HTML-build license covers
the separately attributed official entity-data input. Retain these upstream
licenses with any distributed copy of the cached data.

The selection.json lock freezes enumeration before the first baseline. Every
WPT tree case expands an unspecified scripting flag to both values, retaining
fragment namespace/context. Tokenizer cases expand every declared initial state
and preserve double-escaped UTF-16 code units. Pending-spec and XML-infoset
coercion data are separately supplementary; required scripted cases remain
required. Encoding and serializer inputs are pinned for later F02/F07 selection,
not counted as executed parser cases.

The F01 browser selection includes all unchanged assertions in the locked files.
Its subtests are not yet enumerated/executed: the real browser harness and loaded
entry dependencies belong to F06/F10 and remain required closure blockers.
Native parser output is not browser WPT, Windows App evidence or complete F01.

The original witnesses are requirement examples, not an exhaustive denominator.
The tree adapter emits actual native records without inventing implicit roots,
doctypes or namespace identities. Retained native template children use WPT's
content notation. Missing token-stream/state entries, parse diagnostics, foreign
fragment entries and parser-script execution remain explicit blocks. A matching
tree with unavailable required diagnostics is never a case pass.

Run original classifier/fixture controls with:

    python tools/check_foundation_corpus.py

After explicit exact-byte preparation and a clean committed test build:

    python tools/fetch_foundation_inputs.py --cache <external-cache>
    python tools/foundation_corpus.py run --cache <external-cache> --exe <test-executable>

Preparation validates both Git-blob and SHA-256 identities. Its --offline mode
verifies an already prepared cache without network access. Neither preparation
nor execution changes the frozen denominator.

The test adapter is ignored during ordinary Cargo tests. Results are confined to
the existing verification root and bind source/dirty state, binary/adapter/input/
expected-output hashes, target, effective guards and every selected outcome.
Required failures or harness blocks keep foundation closure open.
