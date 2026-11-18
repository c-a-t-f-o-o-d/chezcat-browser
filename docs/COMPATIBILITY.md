<div align="center">

# Browser compatibility scope

*Measured workflows—not a blanket list of supported libraries.*

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Capabilities](CAPABILITIES.md) · [Documentation](README.md)

</div>

---

## How to read this page

> [!IMPORTANT]
> **Load → startup → render → interact → async.** These are separate outcomes.
> HTTP 200, successful parsing, zero startup errors, or one green test does not
> establish a working website or support for every version of a framework.

This page retains the earlier evidence snapshot through source `a65d37c` on
November 17, 2026, including PR #31, #32 and #34 results. The Bootstrap row adds
later measurements on `3456e67` and `5c89eea`. PRs #53 and #55 integrated those
changes into `main` by `0ef43d0`.
The [catalogue](compatibility.json)
retains its exact source-bound records and classification of 44 named families
and formats; older records are not silently relabeled as newer runs. Source
changes after this snapshot require their own rechecks.

[Current scorecard](#current-scorecard) · [Progression](#measured-progression)
· [Method](#measurement-method) · [Historical evidence](#historical-evidence)

## Current scorecard

| Workload | Observed progress | Remaining boundary | Evidence |
| :--- | :--- | :--- | :--- |
| **Original async form/search/navigation** | Ready state, Unicode submission, readable results, destination activation, exactly one Back and a different query pass the bounded App workflow | Not a live public-service claim | Existing original workflow records |
| **jQuery 1.12.4 + Bootstrap 3.4.1** | Select and hidden/text INPUT prerequisites advance the unchanged legacy sample | `document.createDocumentFragment` at line 4461 after the INPUT correction; complete initialization/interaction not established | [PR #32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32) |
| **jQuery 4.0.0, separate input app** | The exact optimized PR #34 candidate passes the unchanged offline App workflow through initialization, genuine trusted input and asynchronous timer text update | Limited to this pinned app; no AJAX/Deferred, every-version or full-framework claim | [PR #34](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/34), candidate `a9eadfa` |
| **Foundation 6.9.0 generated CSS** | Unchanged JS-enabled fixture forms two columns at 1280 px and stacks at 640 px, with independent image pixels | Vue startup and optional Foundation JavaScript are separate; this is not complete styling | CP3 fixture and App geometry/pixel/hit controls |
| **Vue 2.7.16 runtime** | Gets past the earlier literal-parser and concat boundaries at 1280/640; initial rendering/Foundation CSS pass | Object-prototype access at 11:3808; mount/input/async remain failed. Vue 3 has separate prerequisites | Unchanged W3 concat recheck |
| **Bootstrap 5.3.9 + bundled Popper 2.11.8** | Cards render side by side at 1280 px and stack at 640 px. The load listener reaches the results of its Element selector call. | Spreading that NodeList at line 741, column 14 stops initialization before the dropdown can respond. | Modern fixture revision 2 on `3456e67` and `5c89eea`. [PR #53](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/53) and [#55](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/55) |
| **Original CSS-variable box reduction** | PR #31 changes the box from 80×30 to 104×46, matching a literal control's opaque pixels, anchor hits and live reflow | Supported padding/border values only; alpha colors, calc(), CSSOM and full Bootstrap styling unproved | [PR #31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31), candidate `5cd5e52` |
| **Reference browser** | Initial DOM and pixel captures exist for the original fixture | Interactions were not run; no reference-interaction denominator | Initial-state comparison only |

**Status of a PR is not a new measurement.** The linked observations retain their
actual source and executable identity even after integration. Published alpha.2
is older than the later DOM/string/anchor/text-input/Event/CSS-variable changes.

## Measured progression

| Shared capability | What changed | What it does not prove |
| :--- | :--- | :--- |
| **Collections, calls and Node identity** | Real live lists, checked borrowed slice/apply, genuine Document accessors and fresh text replacement | Full DOM or library startup |
| **Comments and RegExp conversion** | Genuine native-created Comment leaves and standard input ToString on existing exec/test paths | Parsed comments, complete CharacterData, generic RegExpExec or object-valued lastIndex |
| **Responsive CSS** | Initial-font em media, terminal pseudo-target recognition, explicit box-sizing inheritance and fractional flex fitting | Generated-content painting or unblocked Vue execution |
| **Select and hidden inputs** | Genuine native controls, retained identities/edited values and guarded tree/form ownership | Arbitrary input types, explicit new associations or unrestricted fragment mutation |
| **Concat and Node roots** | Ordered generic string concatenation and real connected/detached/reconnected roots | Broader prototype operations or Shadow DOM |
| **Anchor URL reflection** | Scoped live HTTP(S) URL values used by ordinary page code | Unrestricted URL/navigation behavior |
| **Text INPUT fragments** | Staged native creation, dirty/default value semantics, real Reset and focus retirement | Complete library interaction from the input correction alone |
| **Base Event property lookup** | Genuine ordinary string/symbol lookup and protected native state; enables the pinned modern input/timer workflow | AJAX/Deferred or general framework conformance |
| **CSS custom properties** | Winning custom cascade, inherited computed values, bounded fallbacks/cycles and real box painting | Alpha colors, calc(), CSSOM or complete component styling |
| **Element siblings** | Element and CharacterData sibling getters skip Text and Comment nodes and enforce document access checks. Bootstrap reads its next element sibling on `3456e67`. | Full CharacterData, unrestricted child-context access or Bootstrap initialization |
| **Canonical Element selectors** | Scripts can borrow `Element.prototype.querySelector` and `querySelectorAll`. The latter returns a static NodeList, which Bootstrap tries to iterate on `5c89eea`. | Wider selector grammar, child Element queries, iterable NodeLists or dropdown interaction |

The text-input PR's original App checks cover trusted fill, Backspace/Delete,
latest Reset defaults, neighboring pixels/hits, and a removed editor refusing
subsequent typing/paste. Its local and hosted candidate gates passed 2,411 normal
tests and 25 journeys on `3d45d7e`; those numbers describe that candidate, not an
unbounded compatibility percentage or a fresh test of every later commit.

The Event candidate `a9eadfa` passed 70 related interface controls and its local/hosted
normal gates (2,434 tests and 25 journeys). The CSS candidate `5cd5e52` passed
2,427 tests and 25 journeys. These are distinct candidate records, not numbers
recomputed by this documentation refresh. Earlier failed gates remain historical
evidence; later green results do not erase them.

## Measurement method

The active language reference is ECMAScript 2025, edition 16; DOM uses its living
specification. Use original model tests, pinned upstream cases, actual App/input/
render journeys, and reference captures for their distinct purposes.

| Record | Required distinction |
| :--- | :--- |
| **Inputs** | Exact versions, unmodified distribution bytes, compatible dependencies and retained notices |
| **Execution** | Build/source identity, viewport/configuration, actual command and observed first failure |
| **Stages** | Loading, parsing where directly observed, startup, initial rendering, input effects and async behavior |
| **Failures** | First causal blocker versus cascades, unsupported host/runner setup, failed assertion or unavailable prerequisite |
| **Comparison** | ChezCat output versus independently observed reference output; never substitute one for the other |

Do not infer a dedicated parser pass when only a later runtime error is observed.
Do not treat an unavailable test host as a language assertion failure. Hosted
verification remains offline; this document authorizes no live-service probing.

### Frameworks are workloads, not production dependencies

Generated server output is assessed as delivered HTML/CSS/JS, not by server brand.
TypeScript/JSX build tools are not browser engines; their compiled output still
requires browser capabilities. External test runners need an honest adapter.
Unresolved product identities remain unassessed. Modules, async networking,
strings, storage, workers, accessibility, fonts/layout and frames retain their
recorded partial/missing scope.

## Historical evidence

<details>
<summary><strong>🧭 Earlier blockers remain useful evidence</strong></summary>

Legacy W2 advanced from select fragments to hidden INPUT mutation at 1265:4,
then to `div.innerHTML = '<input/>'` at 2684:2. PR #32's later source-bound check
advances to createDocumentFragment. Those earlier failures remain valid for their
original builds; they are not the current first blocker at the newer checkpoint.

Modern jQuery originally failed getRootNode at 4321:7, then anchor href assignment
at 7963:1. Revision 1 also had a distinct harness problem: controls had IDs but
the App driver required names. Revision 2 supplies those names. Startup observations
from revision 1 remain valid; its input result is HarnessBlocked, not an invented
library-input failure. PR #32 later reports startup success and an Event-property
interaction boundary. PR #34 subsequently passes the unchanged pinned input/timer
workflow after the genuine base Event property correction.

Vue's earlier undefined concat at 11:3613 is replaced by the later prototype
boundary. Bootstrap's class failure at 599:3 is an earlier source-bound result.
After the sibling prerequisite, `3456e67` reaches the missing canonical
`Element.prototype.querySelectorAll` call at 741:27. On `5c89eea`, that operation
exists and the next first failure is host NodeList iteration at 741:14. Load,
initial rendering and responsive card layout pass at both widths. Initialization
and input still fail. No
reference browser was run for the separate modern cohort. Original records,
including failed runs, are not overwritten by this presentation update.

</details>

<details>
<summary><strong>📐 The responsive rendering improvement</strong></summary>

At the same 1280 px viewport, the unchanged Foundation panels moved from stacked
positions (38,163)/(38,266) to columns (38,163)/(648,163); 640 px stacking remained.
Normal JavaScript, 100% zoom, original fixtures, unchanged distributions and
independent pixels were retained. Separate App controls cover 1023/1024 thresholds,
125% zoom, odd-width fitting, padding pixels and owned link hits.

This establishes that finite rendering behavior—not successful Vue initialization,
physical desktop output, or complete component styling. The later merged CSS
custom-property work has its own original App reduction and candidate evidence;
it does not retroactively rerun the older Foundation fixture.

</details>

<details>
<summary><strong>🧪 Selected language and native-interface records</strong></summary>

CP2's selected Test262 observations include 18 passes and two Array constructor-
poisoning failures across 20 mode runs; 30 runner controls passed. No full-suite
conformance percentage follows from that subset.

Historical focused records include nine concat groups plus four substring neighbors,
nine hidden-input groups plus twelve select neighbors, and nine Node-root groups
plus four Node constants and thirty-two namespace controls. The genuine Node click
fixture reports correct live roots, changed visible text/pixels and zero errors.
These results retain their actual scopes; physical desktop and release validation
are separate.

</details>

## Where to go next

[Roadmap](ROADMAP.md) owns work order. [Capabilities](CAPABILITIES.md) owns implemented
limits. [Feature contracts](FEATURE_CONTRACTS.md) owns lasting behavior and
acceptance. The catalogue remains the machine-readable source-bound record;
update it only from actual new measurements, not from a visual documentation pass.
