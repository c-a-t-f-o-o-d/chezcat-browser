<div align="center">

# Capability and dependency matrix

*Implemented behavior, explicit limits, and the evidence behind each claim.*

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Documentation](README.md) · [Compatibility](COMPATIBILITY.md)

</div>

---

## Read the matrix correctly

> [!IMPORTANT]
> **Every broad platform area remains partial.** “Implemented” describes a finite
> contract, not complete standards or website support. Local tests, App journeys,
> physical devices, and live services are different evidence scopes.

**Source snapshot:** public `main` at `a65d37c` (November 17, 2026). This includes
PRs [#29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29),
[#30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30),
[#31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31),
[#32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32), and
[#34](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/34).
The fixed [alpha.2 downloads](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2)
remain at `36adf20`; these later changes do not update those binaries.

[Browser & platform](#browser-and-platform) · [DOM & JavaScript](#dom-and-javascript)
· [Rendering](#rendering-and-resources) · [Media](#media)
· [Evidence](#evidence-boundaries) · [Implementation map](#implementation-map)

## Browser and platform

| Capability | Implemented slice | Important limits |
| :--- | :--- | :--- |
| **Native shell** | Tabs/windows, tab-pinned inspectors, guarded primary/middle links and foreground/background/new-window context actions | Named contexts, script-opened windows, wider accessibility, and locked-desktop presentation remain gaps |
| **Navigation** | HTTP(S)/files, explicit URL/file intent, encoded address searches, Google/Bing/DuckDuckGo/None providers, local result/destination/one-Back/second-query journeys | Live outcomes are response/session-dependent; failed navigation is distinct from the retained page |
| **Profile and storage** | Settings, bookmarks, optional history, staged downloads, guarded local roots, memory-only cookies | No general site storage, service workers, or persistent cookies; local files are not encrypted vaults |
| **About / identity** | Full descriptive version/source/state/kind, offline license access, allowlisted Copy data | Metadata is not signing/authentication; App copy capture alone does not prove OS clipboard delivery |
| **License acceptance** | Full offline terms, explicit personal/organization acceptance, strict minimal external receipts, same-text reuse, changed-terms/decline/write-failure refusal | Records a local choice—not verified identity, organizational authority, or an online account |
| **Build and delivery** | Zero third-party production Cargo dependencies, pinned tools, generated resources, portable ZIP and offline gates | Installer and update discovery/upgrade remain planned; releases require owner requests |
| **Isolation** | Checked ownership, worker retirement/cancellation, explicit FFI boundaries and watchdogs | One process; no OS renderer sandbox or whole-process memory sandbox |

## DOM and JavaScript

| Capability | Implemented slice | Important limits |
| :--- | :--- | :--- |
| **WeakMap** | The constructor accepts supported iterable pairs. `get`, `set`, `has` and `delete` use object, function or unregistered symbol identity. Native object keys retain their document access checks. | During callable collection, an entry retains its value while both the map and its key are reachable. Native and symbol identities may remain live for the document. The existing per-collection and document entry limits apply. |
| **WeakSet** | The constructor accepts a supported iterable. `add`, `has` and `delete` track object, function and unregistered symbol identity. Each realm supplies its own constructor and prototype. | Membership alone does not keep a callable alive. Native and symbol identities may remain live for the document. The same entry limits and native access checks apply as for WeakMap. |
| **Reflect.get** | Each realm supplies an ordinary `Reflect` namespace and native `get(target, key, receiver)`. String/symbol keys use normal conversion; ordinary object/function lookup preserves inherited accessor receivers, native getter brands and existing work limits. | Other Reflect operations and Proxy remain outside this slice. Native targets support the omitted or identical receiver through existing access checks; a different receiver on a native target refuses explicitly. Ordinary canonical prototypes can still invoke their genuine branded getters with a supported native receiver. |
| **DOM class inheritance** | A script class can extend the canonical Node, Element, SVGElement, HTMLElement, HTMLBodyElement, HTMLButtonElement, HTMLDivElement, HTMLHtmlElement and HTMLUListElement interfaces. Its constructor and prototype inherit from the selected realm’s actual interface objects. | Calling `super()` for an unregistered HTML element or for SVGElement throws the interface’s TypeError. Custom element registration and other native class bases remain outside this slice. |
| **SVG element identity** | Balanced inline SVG containing supported groups and basic shapes exposes the SVG namespace, case-preserving element names and `SVGElement` ancestry. HTML elements with the same local name keep their HTML identity. | The parser recognizes a bounded set of inert markup. Resource-bearing or ambiguous foreign content retains unknown namespace provenance. SVG-specific element interfaces and programmatic SVG creation remain incomplete. |
| **DOM identity and collections** | Genuine Document/Node accessors, stable live supported childNodes, checked native array-like apply/slice, Attr identity, text replacement and native-created Comment leaves | Parsed comments, full CharacterData, DocumentType/template-content and child-Document collections remain outside the finite scope |
| **Node roots** | Genuine `Node.getRootNode()` for ordinary accessible connected/detached trees, Attr and supported same-origin child trees; ordered options conversion and owner/work checks | No Shadow DOM or fabricated child scope |
| **Node containment** | Integrated [PR #76](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/76) adds genuine inclusive `Node.contains`, physical ancestry, Attr distinction and checked receiver/argument ownership. | Ordinary trees and supported same-origin child Nodes only; no shadow-tree or cross-origin access grant. |
| **DOM object stringification** | BUTTON, UL, DIV, BODY and HTML inherit `Object.prototype.toString` through their own interfaces. MAIN uses `HTMLElement.prototype`. [PR #64](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/64) · [PR #68](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/68) | In supported child documents, HTML and BODY use prototypes from their own realm. |
| **Element selectors, siblings, Slottable and NodeLists** | Scripts can borrow Element selectors. `querySelectorAll` returns a static snapshot. Element/CharacterData sibling getters skip Text/Comment nodes. [PR #67](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/67) adds branded Element/Text `assignedSlot` getters returning null in supported ordinary trees. Main-document NodeLists have bounded indexed iteration. | Existing grammar, work and document access checks apply. Shadow DOM, child Element queries and complete NodeList APIs remain outside this scope. |
| **Select fragments** | Conventional select/option/optgroup fragments; whole-subtree removal/reconnection retains identities, edited values, owner/order/disabled state and retires stale focus/hits | Rich select content, existing-select interior edits, explicit new select associations, and createElement(select) remain unsupported |
| **Hidden inputs** | Fresh hidden-input admission, finite name/value/default reflection and whole-tree moves through native ownership; no box or focus target | Arbitrary existing input-type transitions and unrelated unsupported input kinds remain refused |
| **Text-input fragments** | Default/explicit text INPUT fragments through staged native transactions; dirty/default values, caret/selection, Reset and focus retirement; finite BUTTON.type reads | Unsupported kinds, foreign/inert ancestry, new explicit form associations and exhausted limits reject atomically. This is not unrestricted innerHTML/form support |
| **Anchor URLs** | Scoped live HTTP(S) `href` reflection and related URL values through shared URL/DOM behavior | Wider URL semantics remain contract-bounded; reflection does not grant navigation authority |
| **Language/runtime** | Original interpreter, selected built-ins/promises/jobs, owner-isolated intrinsic roots, RegExp input ToString and generic ordered `String.prototype.concat` | Classes, broader prototype access, UTF-16 code-unit semantics, generic RegExpExec/custom exec, object-valued lastIndex, modules, fetch/XHR and many APIs remain incomplete |
| **Literal prototypes** | Finite static-colon object-literal prototype setters with real null/Object/Function links and ignored primitive RHS | Ordinary own/legacy `__proto__` operations and wider native-host prototype chains remain guarded |
| **Events** | Genuine owner-realm Event/UIEvent/MouseEvent/CustomEvent construction and branded state; real Event/UIEvent/CustomEvent dispatch/cancellation; finite trusted anchor click/auxclick; dispatched base Events use genuine ordinary string/symbol property lookup with metered own storage and protected isTrusted | Synthetic Mouse dispatch, general pointer delivery, specialized keyboard interfaces and shadow retargeting remain gaps |
| **Animation frames** | Replaceable/deletable/restorable main-window scheduling and cancellation through real callbacks and budgets | Child-frame schedulers remain unsupported |

**Integration is not full framework support.** PR #34's exact optimized candidate
passes the unchanged pinned jQuery 4.0.0 offline app through startup, genuine
trusted input, and an asynchronous timer text update. This is one workflow, not
AJAX/Deferred or full-framework support. Legacy jQuery and Vue retain their earlier
recorded fragment and language boundaries. Bootstrap initializes at both widths.
After PR #67/#68, measured candidate `13d7dae` visibly paints the open menu and
runs its trusted amber-choice callback at 1280/640. The menu stays open. The first
microtask fault is computed `position` at 1990:5, and dropdown input remains failed.
Candidate and integrated `main` `5309a68` CI each pass 2,695 normal tests and 25
journeys. The App captures identify the candidate source.
Integrated PR #69/#71 add computed `position` and layout-backed `offsetParent`.
Measured `c74503c` reaches `offsetParent` at 1994:12, then `96666d3` reaches
`scrollLeft` at 3154:19. Both runs open the menu and execute its trusted
Amber-choice callback at 1280/640. The menu remains visible after selection,
and dropdown input remains failed.
Those observations remain bound to their earlier builds. On integrated
`ec7db2f61cddb78c457b2966b72e1e362b89e66a`, PR #74/#75/#76 complete the pinned
Bootstrap dropdown: trusted open, Amber selection and true close pass with
paired DOM/pixels and responsive cards at 1280/640, with zero script errors.
On `acd5703`, the original WeakMap App probe reaches count 4 after a trusted
click and displays its completion title after one timer. On that source the
Vue 3.5.45 runtime stops at its first use of `WeakSet` before mounting.
See [compatibility outcomes](COMPATIBILITY.md) for exact versions and stages.

## Rendering and resources

| Capability | Implemented slice | Important limits |
| :--- | :--- | :--- |
| **CSS/layout** | Loaded styles, token selectors, finite viewport/hover media groups, measured supported geometry/scrollHeight, clipping and software painting | Wider media/selector grammar, pseudo-element generated painting and broad layout remain incomplete |
| **Responsive foundation** | Initial-font em media, explicit box-sizing inheritance and fractional flex fitting; Foundation fixture stacks at 640 and forms two columns at 1280 | Does not establish framework initialization or complete component styling |
| **Document root selector** | Stylesheets and supported DOM queries match `:root` against the document element. Grouped theme rules can define custom properties that descendants inherit. | Detached elements and document fragments have no document element to match. |
| **Stylesheets** | Ordinary unmarked parser/dynamic HTTP(S) sheets, guarded redirects, final URL ownership and bounded accounting | No general stylesheet CORS, imports or CSSOM source access. Marked modes retain their documented subset |
| **CSS backgrounds** | One URL layer on ordinary block/inline-block boxes with ownership, cascade, repeat/position/size and shared resource lifecycle | Inline-fragment/replaced backgrounds and multiple layers remain incomplete |
| **Fonts and images** | Selected original decoders and bounded resource/presentation paths | Format subsets and cross-origin font loading require specific contracts; conditional/dynamic/resize font fetching remains limited |
| **Custom properties** | PR #31 integrates bounded, case-sensitive inherited variables for supported padding/border values, nested fallbacks, cycles, token boundaries and invalid-computed-value handling; original App pixels/hits/reflow match a literal control | Alpha colors, calc(), CSSOM and full Bootstrap component styling remain outside this qualification |
| **Computed custom styles** | [PR #62](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/62), integrated at `5c21400`, provides live read-only custom-property values through the renderer. | Full CSSOM, computed pseudo-element declarations and child Element renderer bindings remain outside this slice. |
| **Computed overflow** | [PR #65](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/65) supplies live `overflow`, `overflowX` and `overflowY` reads from the rendering cascade, including `getPropertyValue`. Rectangular axis clips govern painting and hit bounds. | Clipping is qualified at initial element scroll positions. Element scrolling and scrollbar widgets remain unimplemented. |
| **Computed position and offset parents** | Integrated [PR #69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/69) supplies live computed `position` through named reads and `getPropertyValue`. [PR #71](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/71) returns supported HTMLElement `offsetParent` identities from the actual layout tree. | Existing document access and work checks apply. General child-element geometry and full CSSOM remain outside this slice. |
| **Viewport scroll offsets** | [PR #72](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/72), integrated at `13f5cd4`, adds standard-root vertical scroll reads/writes and zero offsets for supported non-scrollable axes. Qualified feature `e7be7cd` passes 2,732 normal tests and 25 journeys locally and in CI. | Nested element scrolling and scrollbar widgets remain outside this slice. On the measured feature at both Bootstrap widths, the chosen menu remains visible with first fault `offsetLeft` at 1927:10. |
| **Offset coordinates** | Integrated [PR #74](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/74) derives supported `offsetLeft`/`offsetTop` from first border boxes and actual offset-parent padding edges, preserving scroll independence. | Unmeasured fragments/parent boxes refuse explicitly; offsets retain untransformed geometry. |
| **Translations** | Integrated [PR #75](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/75) adds bounded 2D translations, nested composition, visual DOMRects, consistent paint/hits/clips and transformed absolute/fixed containing blocks, including identity translation. | Wider transforms and unmeasured reference geometry remain outside this slice; fallback uncertainty is explicit. |
| **Rounded backgrounds** | Borderless block, flex and inline-block boxes paint a background color with four equal circular corners. Radii support px, em, rem and custom-property substitution. Overlapping curves shrink to fit the final box. | Borders, background images, unequal or elliptical corners, and percentage radii retain square painting. Descendant clipping and pointer hit regions remain rectangular. |
| **Native button padding** | Button, submit and reset controls apply supported CSS padding to their boxes and caption placement. [PR #54](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/54) is integrated at `25654fa`. | Finite button-like control scope. Complete native appearance and authored border behavior remain separate. |
| **GPU acceleration** | Not implemented | Planned measured graphics backend with software fallback; not a replacement decoder grant |

<details>
<summary><strong>📐 Viewport and responsive behavior</strong></summary>

Finite em media lengths use the browser's initial 16 CSS-pixel font basis, not
an author root/body font or physical zoom. Recognized terminal ::before/::after
stylesheet targets preserve valid atomic selector lists without matching ordinary
elements or painting generated content. Explicit box-sizing inherit uses the
actual parent; initial/unset remain content-box. Fractional flex main sizes are
retained until cumulative pixel allocation so two 50% items fit odd widths while
real padding/minimum overflow still wraps.

Nested matching media rules use the real CSS viewport and an explicit desktop
mouse profile. Tests distinguish full viewport width from available line width,
retain fractional zoom thresholds, and verify geometry/background pixels/link
ownership. Admission counts inactive descendants; whole-sheet refusal cannot
publish a prefix. Initial font selection uses the same context, but nested font
faces and dynamic/resize font fetching are not added. Offscreen results are not
physical desktop or complete live-page results.

</details>

## Networking, trust, and diagnostics

| Area | Current behavior | Boundary |
| :--- | :--- | :--- |
| **Transport** | Original HTTP/TLS/certificate handling, redirects, decompression, workers and timings | Protocol, parser, chain and admission gaps remain |
| **Dual-stack fallback** | Up to 256 resolver entries, per-family retention, eight bounded serial attempts, opposite-family time reservation and redacted diagnostics | Blocking OS DNS, up to one two-second connect before cancellation, and no concurrent Happy Eyeballs |
| **Trust** | Reviewed conservative 98-authority Mozilla NSS subset; explicit PEM replacement with shared 1,024-root capacity | No OS-store discovery, automatic root updates, revocation/CT, full Firefox policy or security certification |
| **Policy** | Original hostname for TLS/Host/origin; per-hop CSP, cookies, credentials, mixed-content, MIME and response-policy checks | A successful response is not a usable website or challenge clearance |
| **Diagnostics** | Document-only `--diagnose` with optional exact-host restrictions before DNS, phase/timing evidence and escaped bounded metadata | No page scripts/subresources, profile or usability assessment; raw reports may contain private data |

See [explicit HTTPS trust](../trust/README.md) and the [T1 contract](FEATURE_CONTRACTS.md#t1--shared-tlstrust-and-data-coverage-baseline).
The selected roots exclude unsupported forms and restrictions rather than
silently broadening trust. Custom stores replace the default and remain strict.

## Media

| Area | Implemented slice | Not established |
| :--- | :--- | :--- |
| **Formats** | Local IVF/VP8 visible fixed-canvas version-zero single-MV I/P, PCM and constrained Matroska A/V | SPLITMV, hidden/nonzero-version interframes, WebM/Opus/Vorbis or all codecs |
| **Authored playback** | One supported player, source/load lifecycle, native controls, script play/pause/seek, promises tied to real decoded output | Multiple players, MSE, streaming/ranges, muted Matroska or complete HTMLMediaElement |
| **Network playback** | Complete bounded HTTP(S) IVF/Matroska responses selected by MIME and explicit authored CORS/credential modes | Unmarked foreign playback, general Fetch/CORS, ordinary YouTube or service playback |
| **Interfaces** | Genuine owner-realm Media/Video roots/constants/accessors; saved canonical operations and Boolean controls reflection | VIDEO dimensions, dynamic crossOrigin, extra/child players and broader APIs |
| **Controls lifecycle** | Hide/restore bar without resetting source, clock, frames, seek or play promises; real hit/focus retirement | Physical input or audible output inferred from offscreen tests |

<details>
<summary><strong>🎞️ Decoding and playback evidence</strong></summary>

Original authored journeys check trusted play, event/Promise order, actual clock
advancement, independently expected frame pixels, pause/seek/freeze, native
controls, EOF/replay, Stop and navigation. Loopback HTTP/TLS journeys retain
those assertions and response identity. Policy, MIME/status/partial-response,
cancellation, replacement, and stale-completion negatives remain independent.
Issuer/hostname refusal checks prevent media HTTP/player installation and render
a real error event.

Interframe journeys visit original frames across two keyframe groups in IVF and
Matroska. Native YUV and centered-bilinear BT.601 display references separate
codec and presentation results; the recorded display profile bounds mean RGB
error below 0.5 and maximum channel error at 2. Entropy/reference updates,
segment retention, rollback, cancellation and bounded replay are tested.

Routine verification keeps audio devices closed. Opt-in WinMM checks establish
acknowledged PCM advancement, pause/seek/drain/replay and alignment for the selected
profile—not physical audibility or arbitrary-device compatibility.

</details>

## Evidence boundaries

| Evidence | Establishes | Does not establish |
| :--- | :--- | :--- |
| **Model/unit checks** | Specific conversion, owner, queue, rollback and lifetime rules | Full standards conformance or visible page success |
| **App/input/offscreen** | Real application input, state, layout, hit targets and decoded pixels | Physical desktop, speakers, or public-service compatibility |
| **Pinned library fixtures** | Exact versions and separately observed startup/input/async/layout results | Every library version or an unrelated website |
| **Reference-browser capture** | The actually captured comparison stages | Unrun reference interactions or successful ChezCat execution |
| **Hosted offline CI** | The recorded commit/configuration on its runner | Later source, release assets, devices or live services |
| **Published release** | Actual source/tag, payload, extracted checks and downloaded bytes | Signing, exhaustive certification, reproduction or universal support |
| **Physical/live check** | Explicit observed session and outcome | Results inferred from HTTP 200, titles, elapsed time or zero script errors |

The alpha.2 release has its own normal/optimized/hosted and scoped desktop evidence.
Later source changes retain separate checks and do not retroactively change those
released bytes. Google Search/Images/Maps are not fully supported; ordinary
YouTube playback remains unsupported.

## Implementation map

Paths are repository-root-relative references, not additional runtime payloads.

| Concern | Primary implementation / checks |
| :--- | :--- |
| DOM and native forms | `src/script_dom_tree.rs`, `src/script_node_root.rs`, `src/script_hidden_inputs.rs`, `src/script_select_fragments.rs`, native form modules and original Page/App controls |
| Language/realms | `src/javascript.rs`, `src/string_concat.rs`, `src/object_literals.rs`, `src/prototypes.rs`, `src/intrinsic_realms.rs`, `src/promises.rs` |
| Events | `src/event_interface.rs`, `src/script_dispatch.rs`, `src/script_created_events.rs`, `src/script_anchor_mouse.rs` |
| Network/trust | `src/http.rs`, `src/tls.rs`, `src/x509.rs`, `src/trust.rs`, `src/loader.rs`, original signed-path/data/negative checks |
| Layout/resources | `src/css.rs`, `src/css_media.rs`, `src/layout.rs`, `src/script_geometry.rs`, stylesheet/render journeys |
| Media | `src/media.rs`, `src/audio.rs`, `src/matroska.rs`, `src/script_media.rs`, authored/interface/source-replacement journeys |
| Browser shell | `src/app*.rs`, `src/platform*.rs`, `src/ui.rs`, `src/build_identity.rs`, session/native-owner controls |
| Delivery | `Build.cmd`, `tools/verify_offline.py`, [release guide](RELEASING.md) |

For full lasting acceptance requirements, consult [feature contracts](FEATURE_CONTRACTS.md).
For task order, use the [roadmap](ROADMAP.md); this matrix is not a release schedule.
