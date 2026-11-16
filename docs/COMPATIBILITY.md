# Browser compatibility scope

Compatibility is partial and measured by observable browser stages. A source
inventory, a parser success, HTTP200, zero script errors or a library name alone
does not establish a working page. The [machine-readable catalogue](compatibility.json)
classifies 44 named families and formats by their actual browser, build, server
or test-host role. Unassessed entries are not assumed to work or fail.

The semantic reference for the active language checks is ECMAScript 2025, edition
16; DOM references use the living DOM specification. Original model tests, pinned
upstream cases, actual App/input/render journeys and independent browser reference
captures have distinct scopes. A missing prerequisite or unavailable test host is
reported separately from a language assertion failure. No conformance percentage
is inferred from the small selected case sets.

| Selected local workflow | Observed boundary |
| --- | --- |
| Original asynchronous form/search/navigation | Async ready state, Unicode form submission, readable results, destination activation, exactly one Back and a different query pass the bounded App/input workflow |
| Unmodified jQuery1.12.4 with Bootstrap3.4.1 dropdown/form | One unchanged CP4 App recheck advances past conventional select fragments to jQuery line1265,column4 setting INPUT type=hidden. Load and initial rendering pass; initialization, input and async stages fail. Later missing-library errors are cascades; no visible workflow improvement is established |
| Foundation6.9.0 generated CSS with original controls | The unchanged JS-enabled fixture now forms two columns at1280 and stacks at640, with its original independent image pixels. This is a responsive rendering improvement; Vue initialization and optional Foundation JavaScript are separate |
| Vue2.7.16 runtime with an original reactive component | One unchanged CP5 invocation at1280/640 admits the prior literal parse boundary, then first fails during execution on undefined concat at11:3613. Load/initial render/Foundation CSS pass; Vue mount/input/async fail. Modern Vue3 retains its separate Proxy prerequisite |
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

The select-containing fragment prerequisite passes its reduced controls; the
single unchanged legacy workflow recheck now refuses INPUT type=hidden mutation.
The separate finite literal-prototype prerequisite passes ten language/App
controls, including real input and isolated text pixels. The unchanged Vue file
parses, but its concat failure occurs before the later literal is known to run;
ordinary prototype membership/access remains an unobserved future boundary.
Finite tabs/windows are the next discrete usability milestone after this measured
rendering improvement. Their shared-profile prerequisite now has original
two-App revision/gesture and retained-page controls. Session ownership, shared
downloads/history and terminal retirement now support a finite native tab/window
shell with original pure UI and hidden-HWND controls. Full normal/optimized
integration remains required; native link/context dispositions now have original
pure and hidden-HWND controls, with physical desktop interaction as separate
acceptance. Installer/update work stays planned; further media and
service integration remains deferred. Routine CI stays offline.

The anchor engine uses genuine trusted click/auxclick and
source-bound targets. Demonstrated shared fixes admit checked HTML href/download
mutation and keep painted targets/link styling synchronized with live attributes.
Original App/DOM/event and native lifetime controls cover explicit link/context
routing. Complete normal/optimized and desktop acceptance is still required per
checkpoint. This does not change the measured library
scorecard or establish broader framework support.
