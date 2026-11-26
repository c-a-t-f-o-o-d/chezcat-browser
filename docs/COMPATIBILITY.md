# Measured compatibility

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Capabilities](CAPABILITIES.md) · [Documentation](README.md)

## How to read this page

**Loading, startup, rendering, interaction and asynchronous updates are separate
outcomes.** HTTP 200, successful parsing or zero startup errors alone does not
establish a working website or every version of a framework.

The current verified release is
[0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4),
from tagged source `122d0b7`. Its delivery evidence is in the
[release record](RELEASING.md#current-verified-release). The results below retain
the source revisions actually measured. They are not fresh runs on that release
or current development source. Later implementation changes do not revise an
earlier observation.

[Homepage checkpoint](#latest-reviewed-signed-out-homepage-checkpoint) · [Workflow results](#current-scorecard) · [Integration fixtures](#recent-shared-increments) · [Measurement method](#measurement-method)

### Latest reviewed signed-out homepage checkpoint

The latest recorded signed-out YouTube homepage check used
[`cad5f7c`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/cad5f7c14a8fe8ebe2637e4c5269e84eed9697da).
The page still showed a skeleton, and the DOM walker reached its 30-second
deadline. A usable header, guide and search field were not established.
Search submission, results, watch navigation and playback remain unproved.

A subsequent integration at
[`1de1f24`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/1de1f24e4052fab14157c1419cb6a6e1fdbccf3c)
has no later live acceptance result in this record. Original component fixtures
and newly implemented interfaces do not establish that the live site works.

<a id="current-scorecard"></a>

## Latest recorded workflow results

The combined framework run used
[`067ac5e`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/067ac5edb64e26f39c2a3d816ecbb1cf0a023f3c);
its results belong to the historical `6089546` integration baseline. Other
measurements identify their own sources below. The
[machine-readable catalogue](compatibility.json) retains full source identities,
available executable/capture hashes, viewport details and measurement limits.

| Workload | Recorded outcome | Scope and remaining boundary |
| :--- | :--- | :--- |
| Web Animations 2.3.2 | On `a965b2b`, the standard build's trusted 500 ms seek paints all 576 tested pixels with the expected midpoint color. The next-lite build paints its 3,456-pixel opacity tile white, gray and near-black at 0, 500 and 1,000 ms. The edited input survives. | Public source `e2dabcf` has the same tracked tree. These are offline workflows, with no new live-site result. |
| Bootstrap 5.3.9 progress meter and action stack | On `5298918`, the 240×16 meter matches the literal 25%/75% fills and has a single-line caption at 640 and 1280 px. On `067ac5e` at 640 px, two 160×40 action links form one column; a trusted click changes the gap from 8 to 16 px and the second link receives its click. | A separate `e11acce` fixture retained start alignment for center/end. The later `b4f35e4` focused fixture verifies painted and clickable start/center/end positions. Authored grid tracks and RTL alignment remain open; these are distinct observations. |
| jQuery 1.12.4 + Bootstrap 3.4.1 | At 1280×800 on `7608f4c`, the pinned application loads, paints its initial control and passes the deep radio-clone step. | Initialization stops at jQuery 4743:4 assigning raw onfocusin on a DIV. The same first failure is recorded on `f2e1823`. Dropdown/form input and async remain blocked. |
| jQuery 4.0.0 input application | At 1280×800 on `067ac5e`, startup, trusted input and the asynchronous timer update pass. | This result covers the selected input application. It does not establish AJAX or Deferred support. |
| Bootstrap 5.3.9 + Popper 2.11.8 dropdown | On `067ac5e`, cards render responsively at 1280×800 and 640×800; trusted opening, Amber selection and closing pass at both sizes. | These are the measured layout and dropdown components, not whole-framework conformance. |
| React and ReactDOM 18.3.1 | At 1280×800 on `2f9fa8d`, the production runtimes export, the scheduled callback passes its HTMLIFrameElement check, and native editor/paint controls pass. | On `2f9fa8d` and `f2e1823`, the callback fails at 58:228 reading activeElement.contentEditable before component mount. Later native-interface changes are not a rerun of this application. |
| Vue 2.7.16 with Foundation 6.9.0 | On `067ac5e`, the application mounts, renders, accepts an edit and two clicks, and completes an asynchronous update. Foundation panels form two columns at 1280 px and stack at 640 px. | The result covers these two configurations and the pinned runtime/CSS versions. |
| Vue 3.5.45 runtime | At 1280×800 on `067ac5e`, the component mounts, accepts editing and two clicks, then completes next-tick and timer updates. Both `067ac5e` and the later `45f9491` run complete 17 actions with five fault-free captured stages and a retained independent editor. | The selected runtime-only application is measured; unrelated components and sites are not established. |

<a id="measured-progression"></a>
<a id="historical-evidence"></a>
<a id="earlier-source-bound-scorecard"></a>

### Source and comparison details

The earlier integrated Bootstrap dropdown measurement on
`ec7db2f` retains exact build, source-map, configuration and capture digests in
the catalogue. At both widths its menu crop is 225×39 pixels. Opening changes
8,760 pixels; selection/closing changes 8,763, with 7,631 returning to their
loaded colors. These are observed transitions, not equality with the initial
image or a reference browser. Reference interaction was not run for that result.

The Vue 3 record on `f0dc9ef` retains the actual state sequence: editing and two
clicks produce `count=2 name=Ada 雪 phase=ready`; next-tick and timer delivery
produce `phase=complete` and the title “Vue3 asynchronous update complete.”
A separate async function/arrow fixture on `c669dd9` completes 23 actions per
page, including pending, fulfilled and caught states with retained rejection
identity. These records keep their original sources.

Earlier failures and subsequent passes belong to their own builds and inputs.
A new native capability, a changed first failure or an accepted source change
does not retroactively change an application result.

<a id="recent-shared-increments"></a>

### Recorded integration fixtures

| Fixture | Observed behavior | Boundary |
| :--- | :--- | :--- |
| Selected component path on `cd0b372` | Original application composition exercises nested inert ownership, attribute import, styles, ARIA forwarding, observer/Promise jobs, icons, events, focus, typing and a native link. | Models calls required by the selected noPatch path. It does not run the matching deployed polyfill or establish full native Custom Elements/Shadow DOM. |
| Native base URLs on `cad5f7c` | Original loopback application checks admitted/CSP-blocked bases, real relative-image requests and pixels, retained edited input and native link activation. | Existing HTTP(S) projection and 16 KiB bounds remain. Wider schemes and live main-document base mutation are outside this result. |

These fixtures establish their described native application behavior. They are
separate from the signed-out homepage checkpoint and from physical-device results.

## Measurement method

The recorded language reference is ECMAScript 2025, edition 16; DOM uses its
pinned living-specification source. The catalogue preserves those reference
identities alongside the selected result records.

| Record | What must remain explicit |
| :--- | :--- |
| Inputs | Exact versions, unmodified distribution bytes, compatible dependencies and retained notices. |
| Execution | Actual source/build identity, viewport/configuration, and observed first failure. A later commit does not replace the compiled identity. |
| Stages | Loading, parsing where directly observed, startup, initial render, input effects and async behavior. |
| Failure | First causal blocker, subsequent cascades, unsupported host setup and failed assertions are distinct. |
| Comparison | ChezCat output and independently observed reference output retain separate identities and observed stages. |

A runtime error does not by itself prove a dedicated parser pass. An unavailable
test host is not a language assertion failure. Source inspection, model tests,
application journeys, reference captures, release checks and live sessions have
different evidence scopes.

### Frameworks are workloads, not production dependencies

Generated server output is evaluated as delivered HTML/CSS/JavaScript. A server
brand, TypeScript or JSX build tool does not establish the browser capabilities
its output needs. The selected fixtures do not imply support for other versions,
all components or an entire application family.

<a id="where-to-go-next"></a>

## Related documentation

[Capabilities](CAPABILITIES.md) describes implemented subsets and exclusions.
[Feature contracts](FEATURE_CONTRACTS.md) summarizes behavior and security
commitments. [Roadmap](ROADMAP.md) describes the capability sequence.
[compatibility.json](compatibility.json) is the compact source-bound result
catalogue; a documentation edit adds no new measurement.
