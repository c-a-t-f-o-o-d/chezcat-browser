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

**Earlier source snapshots (November 17–18, 2026).**

This page retains the earlier evidence snapshot through source `a65d37c` on
November 17, 2026, including PR #31, #32 and #34 results. The Bootstrap row adds
later measurements on `3456e67`, `5c89eea` and `5c22afd`, followed by the combined
`main` recheck on `25654fa`. The latest rows separate selector candidate `02cd0f8`,
integrated in `main` at `991cba3`, from measured computed-style build `63a2259`,
integrated through PR #62 at `5c21400`.
PR #64/#65 add measurements on `a82267e` and `7b8a5fb`, integrated by `bf0c737`.
PR #67/#68 retain Slottable measurement `a6a2619` and HTML-container candidate
`13d7dae`, integrated by `main` `5309a68`. Candidate and main CI each pass 2,695
normal tests and 25 journeys. The interaction captures below use `13d7dae`.
PR #69/#71 integrate computed `position` and layout-backed `offsetParent`.
Their measured builds `c74503c` and `96666d3` retain open-menu and trusted-choice
evidence at both widths, with the next errors recorded separately below.
PR #74/#75/#76 add offset coordinates, bounded translations and genuine
`Node.contains`. The unchanged Bootstrap dropdown passes on integrated source
`ec7db2f61cddb78c457b2966b72e1e362b89e66a`: trusted opening, Amber selection and
closing have paired DOM/pixel evidence at 1280/640.
The [catalogue](compatibility.json)
retains its exact source-bound records and classification of 44 named families
and formats; older records are not silently relabeled as newer runs. Source
changes after this snapshot require their own rechecks.

[Current scorecard](#current-scorecard) · [Progression](#measured-progression)
· [Method](#measurement-method) · [Historical evidence](#historical-evidence)

## Current scorecard

The published 0.82.0-alpha.1 tag is `f2e1823`. The downloaded archives, publication metadata and extracted runtime passed their release checks. The framework results below come from development builds of the identified revisions. The combined run used `067ac5e` and the integration baseline was `6089546`. [Release downloads](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.1) · [Machine-readable records](compatibility.json)

| Workload | Observed progress | Remaining boundary | Evidence |
| :--- | :--- | :--- | :--- |
| **Signed-out homepage and original script probes** | On `12c844d`, 40 of 43 scripts completed. The YouTube homepage showed three gray placeholders and no search box. In the offline App on `ed0491f`, computed opacity, a quoted substring selector and an escaped JavaScript property name all succeeded. Seven independent controls passed, including a trusted click and a native text edit. | A live signed-out retest is needed to establish whether the header, search results or video playback work on the corrected build. | [Substring selectors, PR #131](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/131) · [Escaped identifiers, PR #133](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/133) · [Computed opacity, PR #135](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/135) |
| **Original compressed-script and Document probes** | On `e66b0ad`, an 8 MiB + 1 byte gzip script compiles and paints its marker. The 27-case script policy suite includes two exact 16 MiB cases covering parser and dynamic TLS loading. On `1ff9c51`, all six Document interface stages and seven App controls pass, including distinct main/child constructors and borrowed getters. | A 16 MiB + 1 byte script remains refused. Live signed-out shell, search and playback remain unverified. | [Document, PR #121](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/121) and [script allowance, PR #122](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/122), merged into `main` at `83385ca`. |
| **Original canvas rectangles and colors** | On `f53ab3e`, the 80×48 App passes all five drawing stages and nine controls for stable `getContext('2d')`, default-black `fillRect`, transparent `clearRect`, same-width reset and saved-context repaint. App pixel controls cover `object-fit: none` and `scale-down`. On `124686e`, all three color App stages pass, including trusted paint, the `#704040` alpha blend and reset. | The same binary's independent App paints and preserves its edited field, then stops at undefined `getImageData` at 28:18. Paths, transforms and export remain outside these measurements. The later signed-out homepage observation is recorded above. | [Rectangles, PR #125](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/125), integrated at `ca845b4`, and [solid colors, `124686e`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/124686ee77f4a972a288aa46178cc2553e009dfe), integrated in `main` at `9610baf`. |
| **Bootstrap 5.3.9 progress meter and action stack** | At 640 and 1280 px on `5298918`, the 240×16 meter matches the literal fill at 25% and 75%, with its caption on one line. At 640 px on `067ac5e`, the two 160×40 action links form one column. A trusted click changes the gap from 8 to 16 px. The second link receives its own trusted click. | The stack uses default grid placement. Explicit tracks and per-item inline alignment are outside this baseline. | [Implicit grid rows, PR #112](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/112) |
| **jQuery 1.12.4 + Bootstrap 3.4.1** | At 1280×800 on `7608f4c`, W2 loads its pinned assets, paints the initial control and passes the deep radio-clone step. | Both `7608f4c` and `f2e1823` stop during initialization at 4743:4 while assigning the raw `onfocusin` attribute on a DIV. The dropdown, form and async stages remain blocked. | [Native radio clones, PR #115](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/115) |
| **jQuery 4.0.0, separate input app** | At 1280×800 on integrated `067ac5e`, the application initializes, handles trusted input and displays its asynchronous timer update. | This case exercises the pinned input app. AJAX and Deferred have separate requirements. | [Workflow record](compatibility.json) |
| **Bootstrap 5.3.9 + Popper 2.11.8 dropdown** | On `067ac5e`, the original cards render responsively at 1280×800 and 640×800. Trusted opening, Amber selection and closing pass at both sizes. | The responsive layout and dropdown are the measured component scope. | [Workflow record](compatibility.json) |
| **React and ReactDOM 18.3.1, original 1280×800 app** | On `2f9fa8d`, both pinned production runtimes export successfully. The scheduled ReactDOM callback passes its `HTMLIFrameElement` test. The native editor and paint controls pass. | On both `2f9fa8d` and `f2e1823`, the callback stops at 58:228 reading `activeElement.contentEditable` before the component mounts. | [Iframe interface, PR #114](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/114) |
| **Vue 2.7.16 runtime** | On `067ac5e`, the W3 app mounts, renders, accepts an edit and two clicks, and completes its asynchronous update. Foundation places the panels side by side at 1280 px and stacks them at 640 px. | The two W3 configurations use Foundation CSS with Vue 2.7.16. | [Workflow record](compatibility.json) |
| **Vue 3.5.45 runtime, separate 1280×800 app** | On `067ac5e`, Vue mounts its blue 64×24 box, input and two buttons. Editing and clicking twice displays `count=2 name=Ada 雪 phase=ready`. The next-tick and timer callbacks display `phase=complete` and update the title. | Both `067ac5e` and `45f9491` completed all 17 actions. All five captured stages were fault-free, and the independent edited field retained its live and default values. | [Workflow record](compatibility.json) |

### Earlier source-bound scorecard

The following rows preserve the initial measurements and carry their original result boundaries.

| Workload | Observed progress | Remaining boundary | Evidence |
| :--- | :--- | :--- | :--- |
| **Original canvas dimension reflection** | Measured `3e7366a` passes all five dimension stages and seven App controls. The stages cover parsed/default values, IDL and Attr changes, detached ownership and saved accessors. The edited native and literal pixels remain unchanged. | The next stage stops at unsupported `getContext('2d')` at 17:86. Dimension reflection does not establish 2D drawing. | [Canvas dimensions, PR #123](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/123). This result retains its measured feature source. |
| **Bootstrap 5.3.9 progress meter and action stack** | On `5298918`, the meter matches its 240×16 reference at 25 and 75 percent. Its automatic caption fits on one line and matches the explicit-width reference at 640 and 1280 px. | At 640 px, two links in a `d-grid gap-2` stack sit side by side. The literal and flex controls each draw two 160×40 rows, with an 8 px gap that grows to 16 px after a trusted click. | [Caption sizing](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/108) and [recorded geometry](compatibility.json). |
| **Original async form/search/navigation** | Ready state, Unicode submission, readable results, destination activation, exactly one Back and a different query pass the bounded App workflow | Not a live public-service claim | Existing original workflow records |
| **jQuery 1.12.4 + Bootstrap 3.4.1** | On `bfc6b26`, W2 creates a radio, sets its checked attribute and name, and appends it. An independent App selects the created radio with trusted input and resets its form while preserving an edited field in another form. | At 1280×800, initialization stops at 4509:23 on the deep clone of the radio’s container. Dropdown input and async updates remain unreached. | [Native radio inputs](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/113). |
| **Original async function and async arrow pages** | On integrated `c669dd9`, both pages visibly entered a pending state, resumed with 11, and caught a rejection with 21. The page also displayed the effect of a throw after suspension and a later handler observed the same reason. | Each page exercised 23 actions and one intentional unhandled rejection. The measurements cover the await forms used on those pages. | [Await continuations](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/100). |
| **jQuery 4.0.0, separate input app** | The exact optimized PR #34 candidate passes the unchanged offline App workflow through initialization, genuine trusted input and asynchronous timer text update | Limited to this pinned app; no AJAX/Deferred, every-version or full-framework claim | [PR #34](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/34), candidate `a9eadfa` |
| **Foundation 6.9.0 generated CSS** | Unchanged JS-enabled fixture forms two columns at 1280 px and stacks at 640 px, with independent image pixels | Vue startup and optional Foundation JavaScript are separate; this is not complete styling | CP3 fixture and App geometry/pixel/hit controls |
| **React and ReactDOM 18.3.1, original 1280×800 app** | On `a7959b7`, the scheduled ReactDOM callback gets past the input’s `defaultChecked` assignment. The native editor and paint controls pass. | Initialization stops at 57:363 when ReactDOM checks whether the active element is an instance of `window.HTMLIFrameElement`. The component has yet to mount. | [Input default checkedness](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/111) and [workflow record](compatibility.json). |
| **Vue 2.7.16 runtime** | On `1561f4d`, the unchanged W3 app mounts, renders, accepts edited input and two clicks, and completes its asynchronous update. Its Foundation layout uses two columns at 1280 px and stacks at 640 px. | These results cover the two W3 configurations. | Exact source and stages in the [catalogue](compatibility.json). |
| **Vue 3.5.45 runtime, separate 1280×800 app** | On integrated `f0dc9ef`, the component mounts with its blue 64×24 box, text input, and two buttons. Typing and two clicks display `count=2 name=Ada 雪 phase=ready`. Vue’s next-tick and timer callbacks then display `phase=complete` and set the document title. | All 17 actions completed. The five captured stages and retained final frame were fault-free. The independent edited input kept its live and default values. | [HTML label properties, PR #96](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/96) |
| **Bootstrap 5.3.9 + Popper 2.11.8, selector checkpoint** | The unchanged sample initializes and renders responsive cards at 1280/640 on selector candidate `02cd0f8`. | The first trusted click reaches missing `getComputedStyle` at 3734:21 before a menu choice. | [PR #60](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/60) integrated in `main` at `991cba3`. The measurement retains candidate identity `02cd0f8`. |
| **Bootstrap 5.3.9 + Popper 2.11.8, computed-style checkpoint** | Measured feature build `63a2259` passes initialization, initial rendering and responsive layout at both widths. Its local and CI gates pass 2,657 normal tests and 25 journeys. | The first trusted click fails Popper's `node.toString()` at 1736:9 before a menu choice. | [PR #62](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/62) integrated in `main` at `5c21400`. Earlier source-bound observations remain in the catalogue. |
| **Bootstrap 5.3.9 + Popper 2.11.8, overflow checkpoint** | Measured `7b8a5fb` passes initialization, initial rendering and responsive layout at 1280/640. | The first trusted click reaches unsupported `assignedSlot` at 1979:7. The menu remains closed and the choice snapshot is absent. | [PR #65](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/65) integrated at `bf0c737`. The preceding `a82267e` observation is retained separately. |
| **Bootstrap 5.3.9 + Popper 2.11.8, Slottable/container checkpoint** | At 1280/640 on `13d7dae`, initialization/render/responsive layout pass, the menu visibly paints open, and its trusted amber-choice callback runs. | The menu stays visibly open after selection. The first microtask fault is computed `position` at 1990:5. Input remains failed, with 0/2 configurations passing. | [PR #67](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/67) measurement `a6a2619` first reached `node.toString()` at 1736:9. [PR #68](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/68)'s later candidate `13d7dae` supplies the partial interaction evidence, distinct from integrated `main` `5309a68` CI and published alpha.2. |
| **Bootstrap 5.3.9 + Popper 2.11.8, computed-position checkpoint** | On measured `c74503c` at 1280/640, initialization/render/responsive layout pass, the menu opens and the trusted Amber-choice callback runs. | The menu remains visible after selection. The first microtask fault is `offsetParent` at 1994:12. Input remains failed at both widths. | [PR #69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/69) integrates live computed `position` reads from the rendering cascade. |
| **Bootstrap 5.3.9 + Popper 2.11.8, offset-parent checkpoint** | On measured `96666d3` at 1280/640, initialization/render/responsive layout pass, the menu opens and the trusted Amber-choice callback runs. | The menu remains visible after selection. The first microtask fault is `scrollLeft` at 3154:19. Input remains failed at both widths. | [PR #71](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/71) integrates layout-backed `offsetParent` identities. |
| **Bootstrap 5.3.9 + Popper 2.11.8, scroll-offset checkpoint** | Qualified feature `e7be7cd` passes 2,732 normal tests and 25 journeys locally and in CI. At 1280/640 the unchanged sample initializes, renders responsively, opens its menu and runs the trusted Amber choice. | The chosen menu remains visible. The first microtask fault is `offsetLeft` at 1927:10. Input remains failed at both widths. | [PR #72](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/72) integrates at `main` `13f5cd4`. These captures retain feature source `e7be7cd`. Relative offset coordinates are the next capability after PR #71's offset-parent identities. |
| **Bootstrap 5.3.9 + Popper 2.11.8, integrated dropdown** | On clean integrated `ec7db2f`, load, startup, initial render, trusted open/Amber-choice/close and responsive cards pass at 1280×800 and 640×800, with zero script errors: 2/2 selected configurations. Open and closed states have paired DOM and pixel evidence. | One pinned offline app; async is not applicable and reference interaction was not run. Whole-framework support remains unproved. | Integrated [PR #74](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/74) offsets, [PR #75](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/75) translations and [PR #76](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/76) contains. Exact build and capture digests are in the [catalogue](compatibility.json). |
| **Bootstrap card styling, 640 and 1280 px** | On `7d40d7f`, the page's root variables supply its caption color and 1 px border. On `8ebf051`, all 144 samples just outside the rounded corners show white and the straight edges retain the same pixels. Trusted open, Amber selection and close pass at both widths. | The two wide cards still touch. The same executable gives a 24 px gutter with literal column padding and a 0 px gutter with calculated padding in the original grid reduction. | [Root selector PR #82](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/82) and [rounded border PR #85](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/85), integrated at `82053ba` and `903cc6e` respectively. |
| **Original CSS-variable box reduction** | PR #31 changes the box from 80×30 to 104×46, matching a literal control's opaque pixels, anchor hits and live reflow | This measurement covers the selected padding and border values. | [PR #31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31), candidate `5cd5e52` |
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
| **Canonical Element selectors** | Scripts can borrow `Element.prototype.querySelector` and `querySelectorAll`. The latter returns a static NodeList. `02cd0f8` also clears the recorded dropdown selector boundary. | Wider selector grammar, child Element queries or dropdown interaction |
| **NodeList iteration** | Supported main-document NodeLists expose bounded indexed iteration. Bootstrap spreads its selector results and initializes at both widths on combined `main` `25654fa`. | Complete NodeList APIs or a working dropdown |
| **Native button padding** | Supported padding changes native button boxes and caption placement. The separate Bootstrap comparison below records the visible result. | Complete native appearance or a combined main workflow result |
| **Computed custom styles** | Live read-only custom-property values on measured `63a2259`, integrated at `5c21400`, advance the unchanged Bootstrap click into Popper at both widths. | Full CSSOM or a working dropdown |
| **DOM object stringification and Slottable** | PR #67 adds ordinary-tree Element/Text `assignedSlot` getters. PR #68 adds ordinary object stringification for DIV, BODY and HTML through their own interfaces. MAIN uses HTMLElement. `13d7dae` paints the open menu and runs the trusted choice at both widths. | The close step is still open. The catalogue retains earlier measured failures. |
| **Computed overflow** | Resolved physical axes share the rendering cascade and rectangular paint/hit clipping. `7b8a5fb` advances to `assignedSlot` at 1979:7. | Initial element scroll positions only. Element scrolling and scrollbar widgets remain unimplemented. |
| **Computed position and offset parents** | Integrated PR #69/#71 expose live computed `position` and supported layout-backed HTMLElement `offsetParent` identities. Measured `c74503c` reaches `offsetParent`, then `96666d3` reaches `scrollLeft`. | The menu remains visible after trusted selection at both widths. Scroll offsets and a completed dropdown workflow are not established by these runs. |
| **Viewport scroll offsets** | PR #72 integrates at `13f5cd4`. Its qualified feature `e7be7cd` reads and writes the standard-document root's vertical scroll and returns zero for supported non-scrollable axes. Real App scrolling, pixels and hits are tested. | Nested element scrolling and scrollbar widgets remain outside this slice. The measured dropdown still reaches an unimplemented relative offset coordinate. |
| **Offsets, translations and contains** | PR #74/#75/#76 preserve raw offset geometry, add bounded translated paint/hits and visual rectangles, and use genuine inclusive Node ancestry. Together, integrated `ec7db2f` completes the pinned dropdown at both widths. | Wider transforms, nested scrolling and whole-framework conformance remain outside this result. |

The integrated menu crop is 225×39 px at both widths. Opening changes 8,760
pixels; selection/closing changes 8,763 from the open state, with 7,631 returning
to their loaded colors. The DOM records `Bootstrap dropdown open`, then
`Bootstrap dropdown closed` and `Bootstrap trusted choice amber`. The cards are
beside each other at 1280 and stacked at 640. These are transition measurements,
not a claim that the final image equals the initial or a reference browser.

A separate original App probe stopped at missing `WeakMap` at 11:92 on
`ec7db2f`. On measured `acd5703`, a trusted click sets the visible count to 4
and one timer completes with the title `WeakMap pass`.
[WeakMap PR #78](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/78) integrated at `5c8fe78`.

The separate padding comparison uses loaded captures from `5c89eea` and
`e94388c`. At both widths the unchanged Bootstrap toggle grows from 167×29 to
191×41 px. Its caption shifts 12 px right and 6 px down, and the aligned original
content crop matches. Caption attribution uses a 102 px content offset derived
from source constants. Capture metadata did not independently calibrate that
offset. The padding branch predates the NodeList merge and initialization fails
on both compared builds. [PR #54](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/54)
integrates padding in `main` at `25654fa`. A recheck on that combined main build
initializes at both widths and records 191×41 px loaded toggles. The first trusted
click on that build fails the dropdown selector at 357:27 before the menu opens.

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
exists and the next first failure is host NodeList iteration at 741:14. At that
checkpoint initialization and input still fail. On `5c22afd`, NodeList iteration
allows the unchanged sample to initialize with zero startup errors at both widths.
The first trusted click then rejects
`[data-bs-toggle="dropdown"]:not(.disabled):not(:disabled)` at 357:27 before the
menu opens. Load, initial rendering and responsive card layout continue to pass.
On selector candidate `02cd0f8`, the first click advances to missing
`getComputedStyle` at 3734:21. Measured feature build `63a2259` advances further to
Popper's `node.toString()` at 1736:9. Both later runs still fail dropdown input.
No reference browser was run for the separate modern cohort. Original records,
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
