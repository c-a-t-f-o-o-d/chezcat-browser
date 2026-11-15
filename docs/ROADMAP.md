# Finite development roadmap

The active workstreams are **T1: TLS/trust compatibility** and
**B1: ordinary resources and viewport rendering**.
The explicit CSAPUL 1.1 acceptance milestone is published as
[0.80.0-alpha.1](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.80.0-alpha.1).
New completed milestones follow the exact-source build, package and download
checks in the [release process](RELEASING.md). Status below describes finite
implemented slices, not complete web-platform or public-service support.

Current development order is shared TLS/trust and data coverage, everyday
resource/render/interaction usability, then finite tabs, link context and new
windows. A finite A1 About/build-identity slice is implemented at this UI checkpoint.
One per-user Windows installer and staged update support follow the first page-
usability/tabs milestones, before optional media expansion. Further F6/YouTube
integration is deferred; completed finite media and interface slices remain
supported within their recorded limits.

| ID | Status | Current outcome | Remaining scope |
| --- | --- | --- | --- |
| T1 | In progress: remaining interoperability gaps | Guarded document diagnostics, a pinned conservative Mozilla NSS 98-authority selection, reviewed offline generation, shared 1024-root policy and bounded per-hop redirect clocks; coordinated document checks distinguish improved path coverage from later failures | Remaining chain/parser/protocol/timeout/admission and resource-specific failures; no broad TLS or usable-site completion claim |
| B1 | In progress: stylesheets and finite viewport media rules | Shared unmarked cross-origin HTTP(S) stylesheet loading; host-context media rules with real resize/zoom geometry and pixels, source ownership and complete-sheet admission | Original executable width/zoom and dynamic-link cases passed; normal verification remains required for each checkpoint; retain request/security/lifecycle boundaries and explicit query/font gaps while addressing broader resource/render/input usability |
| B2 | Planned after first page-usability milestone | No new tab or window capability is claimed | Finite tabs, link-context and new-window behavior with real page/history/worker/focus ownership |
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
| F6 | Deferred for browser foundations | Authored synchronized A/V, genuine Media/Video interfaces and finite Event/UIEvent/MouseEvent/CustomEvent construction are implemented prerequisites; Event/UIEvent/CustomEvent dispatch and cancellation work | Actual ordinary non-DRM YouTube playback is unproved; Mouse dispatch/native pointer payloads, KeyboardEvent construction and broader page/player APIs remain unsupported |

Every implementation checkpoint requires normal verification. B1 now includes
the bounded stylesheet path and a finite viewport-media slice, with original
local request, source-order, geometry/pixel and lifetime checks. Require actual
offscreen width/zoom and dynamic-stylesheet acceptance before treating that
rendering checkpoint as integrated; broader page usability remains in progress.
Distinguish remaining trust/path,
parser, timeout and intentional admission failures from page rendering. Continue
meaningful page usability, then finite tabs/windows; perfect TLS or every-page
coverage is not a prerequisite. Installer/update work remains planned.
VIDEO dimensions, Element.toggleAttribute, NamedNodeMap mutation and remaining
media/player work stay recorded gaps outside the active queue. Service playback
remains unproved by local media evidence.

Wider Unicode, CSS, storage, frames and accessibility remain gaps. Complete Maps,
every codec and perfect site screenshots are not prerequisites for useful staged
progress. Security defects and demonstrated prerequisites can interrupt the active
slice. Detailed behavior and acceptance belong in the
[feature contracts](FEATURE_CONTRACTS.md); evidence boundaries belong in the
[capability matrix](CAPABILITIES.md).
