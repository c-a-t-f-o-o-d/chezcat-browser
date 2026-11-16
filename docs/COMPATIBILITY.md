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
| Unmodified jQuery1.12.4 with Bootstrap3.4.1 dropdown/form | Rechecks advance past child lists, native array-like apply and main-Document Node identity to missing native Comment/createComment. Later missing-library errors are cascades. Initialization and visible improvement are not established |
| Foundation6.9.0 generated CSS with original controls | Narrow stacking passes; the wide two-column layout fails. These observations assess selected CSS output, not optional Foundation JavaScript |
| Vue2.7.16 runtime with an original reactive component | First observed blocker is unsupported object-prototype configuration. Modern Vue3 has a separate missing Proxy prerequisite; legacy and modern tiers are not interchangeable |
| Independent browser reference | Initial DOM and pixels were captured; interactions were not run and do not supply a workflow-pass denominator |

CP1 provides genuine SameObject live child lists over actual retained Element/Text
nodes, ordinary static copies through existing native-host slice, ordered native
array-like Function.apply capture, and correct
fresh/empty Element textContent replacement. Saved old nodes retain identities
and data; owner, resource, quota, rollback and recovery controls remain. Comment,
DocumentType and template-content trees, child-Document collections and full DOM
coverage are not added. Normal/executable integration and the unmodified-library
recheck remain required acceptance. Genuine main-Document Node facts and real
Node.prototype nodeType/nodeName getters now pass focused original controls. CP1
remains a prerequisite pack; native Comment/createComment is the next connected
dependency. Selected Test262 slice checks have 18 passes and two ordinary Array
constructor-poisoning failures across 20 mode runs; the failing assertions are
retained, with no full-suite conformance inference. See the
[CP1 contract](FEATURE_CONTRACTS.md#cp1--real-collections-calls-and-node-identity).

Generated server output is assessed as delivered HTML/CSS/JS, not by the server
brand. TypeScript/JSX build tools are not browser engines; their compiled output
still needs the corresponding browser features. External test runners require an
honest adapter and are not browser-library failures. Unresolved product identities
remain unassessed. Wider modules, async networking, strings, storage, worker,
accessibility, layout/font and frame APIs remain partial or missing as recorded in
the [capability matrix](CAPABILITIES.md).

The next pack follows the next actual first blocker and its shared dependency.
Finite tabs/windows follow the first measured workflow improvement rather than
universal compatibility. Installer/update work stays planned; further media and
service integration remains deferred. Routine CI stays offline.
