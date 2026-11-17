# Browser compatibility scope

Compatibility is partial and measured by observable browser stages. A source
inventory, a parser success, HTTP200, zero script errors or a library name alone
does not establish a working page. The [machine-readable catalogue](compatibility.json)
classifies 44 named families and formats by their actual browser, build, server
or test-host role. Unassessed entries are not assumed to work or fail.
This document and the catalogue own measured version/workflow results; development
order belongs to the [roadmap](ROADMAP.md), implemented limits to the
[capability matrix](CAPABILITIES.md), and lasting acceptance to the
[feature contracts](FEATURE_CONTRACTS.md).

The semantic reference for the active language checks is ECMAScript 2025, edition
16; DOM references use the living DOM specification. Original model tests, pinned
upstream cases, actual App/input/render journeys and independent browser reference
captures have distinct scopes. A missing prerequisite or unavailable test host is
reported separately from a language assertion failure. No conformance percentage
is inferred from the small selected case sets.

| Selected local workflow | Observed boundary |
| --- | --- |
| Original asynchronous form/search/navigation | Async ready state, Unicode form submission, readable results, destination activation, exactly one Back and a different query pass the bounded App/input workflow |
| Unmodified jQuery1.12.4 with Bootstrap3.4.1 dropdown/form | The unchanged hidden-input recheck at1280 advances from setting INPUT type=hidden at1265:4 to `div.innerHTML = '<input/>'` at2684:2. Load and initial rendering pass; initialization, input and async stages fail. Later missing-library errors are cascades; no visible library workflow improvement is established |
| Foundation6.9.0 generated CSS with original controls | The unchanged JS-enabled fixture now forms two columns at1280 and stacks at640, with its original independent image pixels. This is a responsive rendering improvement; Vue initialization and optional Foundation JavaScript are separate |
| Vue2.7.16 runtime with an original reactive component | The unchanged concat recheck at1280/640 advances from undefined concat at11:3613 to object-prototype access at11:3808. Load/initial render/Foundation CSS pass; Vue mount/input/async fail. Modern Vue3 retains its separate Proxy prerequisite |
| Separate unmodified jQuery4.0.0 input app | Load and initial paired rendering pass; the first failure is unsupported getRootNode at4321:7. Initialization, trusted input and timer update fail. This is independent of the legacy hidden-input blocker |
| Separate unmodified Bootstrap5.3.9 app with bundled Popper2.11.8 | Load/initial rendering pass at1280/640, with two columns widely and stacking narrowly. Unsupported class syntax at599:3 prevents initialization and dropdown input. This measures the original cards' positions, not complete component styling |
| Independent browser reference | Initial DOM and pixels were captured; interactions were not run and do not supply a workflow-pass denominator |

Delivered CP1 provides the real collection/call/replacement/Node prerequisites.
CP2 adds genuine native-created Comment leaves and standard input ToString for
existing native RegExp exec/test paths, with focused original Page/App/language
controls. Comment data stays out of Text, rendering, selectors and resources;
the tokenizer still omits parsed comments. Full Comment/CharacterData interfaces,
generic RegExpExec/custom exec and object-valued lastIndex remain outside scope.
Neither pack establishes library initialization or a visible workflow improvement.
Normal/build/executable integration and unchanged-library rechecks remain required.
See the [CP1](FEATURE_CONTRACTS.md#cp1--real-collections-calls-and-node-identity) and
[CP2 contracts](FEATURE_CONTRACTS.md#cp2--genuine-comment-and-native-regexp-input-conversion).

Selected CP2 Test262 observations have 18 passes and two ordinary Array
constructor-poisoning failures across 20 mode runs; all 30 runner controls pass
and inputs remain unchanged. Failing assertions are retained, with no full-suite
conformance inference. Original workflow and reference entries retain their stated
measured scopes rather than becoming a new all-workflow run.

Generated server output is assessed as delivered HTML/CSS/JS, not by the server
brand. TypeScript/JSX build tools are not browser engines; their compiled output
still needs the corresponding browser features. External test runners require an
honest adapter and are not browser-library failures. Unresolved product identities
remain unassessed. Wider modules, async networking, strings, storage, worker,
accessibility, layout/font and frame APIs remain partial or missing as recorded in
the [capability matrix](CAPABILITIES.md).

CP3 connects initial-font em media lengths, recognized terminal before/after
stylesheet targets, explicit box-sizing inheritance and fractional flex line
fitting. At the same1280 viewport, the unchanged Foundation panels move from
stacked positions(38,163)/(38,266) to two columns(38,163)/(648,163); at640 the
original stacking remains. Zoom100, normal JavaScript, original fixtures,
unmodified distributions and independent control pixels are retained. Original
App tests separately cover1023/1024 thresholds,125% zoom, odd-width fitting,
padding pixels and owned link hits. This finite rendering result does not imply
that the separately blocked Vue component initializes or responds to input.
Full normal/build/executable verification remains required for integration.

The hidden-input prerequisite passes nine focused and twelve adjacent select
controls. Actual App software clicks create a genuine hidden control, change an
isolated text crop, preserve a neighboring edited value/cursor and source/history,
and retain native form request serialization. Hidden inputs gain no box or focus,
even under authored display rules. The unchanged legacy workload next refuses an
input-containing innerHTML fragment at2684:2; initialization remains failed.
The separate finite literal-prototype prerequisite passes ten language/App
controls, including real input and isolated text pixels. Original concat controls
now pass nine VM/realm/App groups and four adjacent substring groups. Its unchanged
Vue recheck reaches the separate object-prototype boundary; no mount or reactive
interaction is established. The workload observations use explicitly dirty
development builds with retained source maps and executable identities, not clean
release results; the catalogue preserves the distinct concat and hidden-input
build records. The hidden-input build contains an unregistered Node draft and
establishes no getRootNode result.
Finite tabs/windows and guarded native link/context actions are delivered in
[0.81.0-alpha.2](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2).
Their source-scoped normal/optimized, hosted and desktop acceptance is separate
from this library scorecard. The concat correction is in development; its focused
language/App result does not establish that the unchanged Vue workload initializes.
Legacy and modern library measurements remain separate. Installer/update work,
further media and service integration stay deferred. Routine CI stays offline.

The anchor engine uses genuine trusted click/auxclick and
source-bound targets. Demonstrated shared fixes admit checked HTML href/download
mutation and keep painted targets/link styling synchronized with live attributes.
Original App/DOM/event and native lifetime controls cover explicit link/context
routing. Changed native behavior requires affected acceptance; normal regression
and hosted CI remain integration gates. This does not change the measured library
scorecard or establish broader framework support.

The modern cohort uses exact unmodified distributions and retained MIT notices
under the source-only `tools/fixtures/compatibility-modern/inputs.json` manifest.
Its two workflows cover three configurations; the existing legacy fixtures remain
unchanged. Parsing is reported conservatively when no dedicated parser-phase
diagnostic exists: the actual first error is retained, while initialization and
interaction remain failed. No reference browser was run for these new samples.
