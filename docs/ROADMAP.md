# Finite development roadmap

The active milestone is **B2 release closeout for 0.81.0-alpha.2**: one consolidated
catch-up for completed finite tabs/windows/link-context behavior and the integrated
CP1–CP5, T1 and B1/A1 prerequisites. After publication is verified, the next
independent compatibility item is the measured String.prototype.concat dependency.
It is not implemented by CP5. T1 TLS/trust and B1 resource/rendering remain partial
shared baselines; measured workflows determine subsequent finite work.
The explicit CSAPUL 1.1 acceptance milestone is published as
[0.80.0-alpha.1](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.80.0-alpha.1).
The next release target is
[0.81.0-alpha.2](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2).
Its exact release gates are pending; a source milestone, green CI or development
delivery is not a published release. Completed named milestones close through the
exact-source build, package and download checks in the [release process](RELEASING.md)
before an independent item begins. Status below describes finite implemented
slices, not complete web-platform or public-service support.

Current development uses small original regressions and unmodified, pinned local
workloads to improve ordinary resource/render/interaction usability. Finite
tabs, link context and new windows followed the first measured workflow improvement;
universal framework or site coverage is not a prerequisite. The finite A1 About/
build-identity slice is implemented. See [compatibility scope](COMPATIBILITY.md).
One per-user Windows installer and staged update support follow the first page-
usability/tabs milestones, before optional media expansion. Further F6/YouTube
integration is deferred; completed finite media and interface slices remain
supported within their recorded limits.

| ID | Status | Current outcome | Remaining scope |
| --- | --- | --- | --- |
| CP5 | Delivered finite prerequisite; ten language/App controls pass | Static colon literal prototype setters preserve real null/Object/Function links, primitive RHS behavior, ordering and early duplicate errors; actual App input changes independent text pixels | Unchanged Vue now parses then fails on undefined concat at 11:3613; no Vue mount/input/async pass. Ordinary own/legacy prototype access remains a separate boundary |
| CP4 | Delivered finite prerequisite | Conventional select fragments create genuine native controls; whole-select retirement/reconnect preserves edited values and identities, projects owner/disabled state and submits connected tree order | Unchanged W2 recheck now refuses INPUT type=hidden mutation. Explicit associations and existing-control interior edits remain gaps |
| CP3 | Delivered finite responsive rendering prerequisite | Unchanged Foundation 6.9 generated-CSS fixture forms two columns at 1280 and stacks at 640 in normal JS mode; original App resize/zoom/pixel/link assertions pass | Finite em media, typed before/after stylesheet targets, explicit box-sizing inheritance and fractional flex line fitting; no generated-content painting or Vue initialization claim |
| CP2 | Delivered prerequisite slice | Genuine main-Document createComment with real leaf/data/topology and no rendering/resource promotion; native RegExp exec/test input ToString through existing conversion and Function source | The historical CP2 recheck stopped at select-containing innerHTML; CP4 now advances to hidden-input mutation. No library initialization or visible improvement is attributed to CP2 |
| CP1 | Delivered prerequisite slice | Main-Document/supported Node live child lists, existing borrowed slice, native array-like apply, fresh text replacement and genuine Document Node identity | No full DOM or whole-framework claim; subsequent prerequisites are tracked in CP2 |
| T1 | Finite corrections integrated; interoperability gaps remain | Guarded document diagnostics, a pinned conservative Mozilla NSS 98-authority selection, reviewed offline generation, shared 1024-root policy, bounded per-hop redirect clocks and family-interleaved TCP fallback with explicit resolver/attempt limits; coordinated document checks distinguish improved path coverage from later failures | Blocking OS DNS, serial-connect cancellation latency and concurrent Happy Eyeballs remain; remaining chain/parser/protocol/timeout/admission and resource-specific failures; no broad TLS or usable-site completion claim |
| B1 | In progress: stylesheets and finite viewport media rules | Shared unmarked cross-origin HTTP(S) stylesheet loading; host-context media rules with real resize/zoom geometry and pixels, source ownership and complete-sheet admission | Original executable width/zoom and dynamic-link cases passed; normal verification remains required for each checkpoint; retain request/security/lifecycle boundaries and explicit query/font gaps while addressing broader resource/render/input usability |
| B2 | Delivered finite tabs/windows and native link/context milestone | Bounded Session/shared authorities, native tab/window controls and pinned inspectors; genuine anchor activation, checked foreground/background/window admission and stale-safe native context/capture routing; passive popup hover retained with held-button invalidation | Prior checkpoint acceptance is recorded below; new checkpoints require exact normal/optimized and desktop evidence. Named contexts, script-opened windows and broader accessibility remain gaps |
| A1 | Implemented finite native About slice | Browser-owned About panel, embedded full product/source/state/build description, exact main legal texts and safe build-information Copy | Normal/build/executable validation is required for each checkpoint; metadata is descriptive, not authenticated publication or signing, and App payload capture does not prove OS clipboard delivery |
| I1 | Planned after first B1/B2 milestones | Portable runtime ZIP remains the distribution baseline | One per-user x64 MSI with complete exact payload, shortcuts/uninstall, deliberate upgrade version mapping and isolated data-preservation/recovery tests |
| U1 | Planned with delivery work | No automatic or manual release discovery is claimed | Real published-milestone discovery, startup opt-in default OFF, manual check, bounded async work and deliberate prerelease channels |
| U2 | Planned after installer/authenticity prerequisites | No automatic download/install/restart is claimed | Separate explicit approval, authenticated package identity, staged installer/recovery and changed-terms acceptance |
| L1 | Complete | Full offline native license text and explicit acceptance; personal/organization source and headless acceptance; strict external receipts, decline and changed-terms checks | No identity or organizational-authority verification; exact release packaging remains a separate gate |
| F1 | Complete for the main Window | Replaceable animation operations with real scheduling, cancellation, owner and work rules | Child-frame schedulers |
| F2 | Complete for the original local journey | Address/search intent, result/destination/one-Back/different-query input and startup redirect/replacement handling | Live behavior remains response/session-dependent; no every-response Google promise |
| F3 | Complete for one supported authored video | Real load, pause, currentTime seek and play promises/events tied to decoded output, with native controls and source retirement | Wider HTMLMediaElement interfaces, multiple players and unsupported profiles |
| F4 | Complete for bounded transfers | HTTP(S) IVF/Matroska playback with explicit MIME, redirects, credentials/CORS, cookies, CSP, TLS refusal and cancellation | Streaming/ranges, unmarked foreign media and dynamic crossOrigin controls |
| F5 | Complete for the selected profile | Visible version-zero VP8 I/P single-vector interframes, dependency-aware seek and constrained Matroska PCM16 synchronization | SPLITMV, hidden frames, other interframe versions and wider codecs/containers |
| C1 | Complete for one URL background layer | Stylesheet-relative resource ownership, shared cascade/load/layout/paint and original request/pixel/geometry checks on ordinary block boxes | Multiple layers, gradients and inline-fragment/replaced-element backgrounds |
| F6 Media/Video interfaces | Complete for the original bound-player model/App slice | Genuine owner-realm Media/Video roots, stable class identity and canonical controls/accessors through own shadows; actual Promise/time/independent-pixel/lifecycle proof | VIDEO width/height IDL reflection, additional/child players and broader media APIs remain unsupported |
| F6 CustomEvent interfaces | Complete for the original constructor/notification model/App slice | Genuine owner-realm CustomEvent roots, actual any detail and atomic initialization/publication; real input-driven constructed/legacy dispatch, cancellation, path and child-owner proof | Full normal verification is required for integration; child-document factories, native CustomEvent inputs and broader event interfaces remain unsupported |
| F6 controls reflection | Complete for the original model/App slice | Real Boolean controls reflection, including unbound Media nodes; native bar paint/hits/focus with preserved source, clock, frame and Promise lifetime | Full normal verification is required for integration; VIDEO dimensions, Element.toggleAttribute and NamedNodeMap mutation remain separate source gaps |
| F6 | Deferred for browser foundations | Authored synchronized A/V, genuine Media/Video interfaces and finite Event/UIEvent/MouseEvent/CustomEvent construction are implemented prerequisites; Event/UIEvent/CustomEvent dispatch and cancellation work | Actual ordinary non-DRM YouTube playback is unproved; synthetic Mouse dispatch, native pointer payloads beyond finite anchor activation, KeyboardEvent construction and broader page/player APIs remain unsupported |

Every new implementation checkpoint requires normal verification. The accepted
B2 source [67c5d69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/67c5d691012eeb39767e44000d9c1aeac8cdea1a),
select checkpoint [73f3315](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/73f3315ae23760966efb1c1e6bf8ff3e2492c6c4)
and literal checkpoint [fb385cb](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/fb385cb6c0b06988d55beb8c60f9f9a7cab6e431)
have recorded local/hosted offline checks. Separate actual Windows desktop records
cover six groups at 67c5d69, six at 73f3315 and four at fb385cb. Those source-scoped
records do not substitute for the new release's
exact clean source/executable, payload, extraction and published-byte checks.
The 0.81.0-alpha.2 target closes these accumulated milestones once those checks
and actual publication complete; no release is asserted by this roadmap alone.

CP1 and CP2 use shared DOM/language paths. CP4's single unchanged W2 recheck now
reaches refused INPUT type=hidden mutation after select-fragment admission.
CP5's single unchanged W3 invocation admits the pinned Vue file, then first fails
on undefined concat at 11:3613 in both selected widths. It does not establish that
the later prototype literal executed. Loading and initial rendering pass while
library initialization/input/async remain unproved; CP3's independent Foundation
rendering improvement forms columns widely and stacks narrowly. Reference-browser
initial DOM/pixels remain comparison evidence, with reference interactions unrun.

Selected CP2 Test262 observations retain 18 passes and two ordinary Array
constructor-poisoning failures across 20 mode runs; all 30 runner controls pass and
inputs are unchanged. These denominators establish no full-suite conformance.
T1's bounded dual-stack correction is integrated through
[PR22](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/22). Preserve remaining
chain/parser/protocol/timeout/admission failures, blocking DNS and serial-connect
cancellation limits; neither document diagnostics nor trust data establish a
newly usable public service. B1 broader page usability remains in progress.

After release closeout, continue the separate concat reduction and unchanged
workflow evaluation. Hidden-input mutation remains a distinct backlog item.
VIDEO dimensions, Element.toggleAttribute, NamedNodeMap mutation and remaining
media/player work stay outside the active queue. Installer/update work remains
planned; service playback is unproved by local media evidence.

Wider Unicode, CSS, storage, frames and accessibility remain gaps. Complete Maps,
every codec and perfect site screenshots are not prerequisites for useful staged
progress. Security defects and demonstrated prerequisites can interrupt the active
slice. Detailed behavior and acceptance belong in the
[feature contracts](FEATURE_CONTRACTS.md); evidence boundaries belong in the
[capability matrix](CAPABILITIES.md).
