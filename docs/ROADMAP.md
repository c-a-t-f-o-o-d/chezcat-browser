# Finite development roadmap

The current milestone is **0.80.0-alpha.1: explicit CSAPUL 1.1 acceptance**.
Its implemented offline acceptance and execution gates are complete; publication
still requires the exact release build and package checks in the
[release process](RELEASING.md). Status below describes finite implemented slices,
not complete web-platform or public-service support.

Current development order is shared TLS/trust and data coverage, everyday
resource/render/interaction usability, then finite tabs, link context and new
windows. Further F6/YouTube integration is deferred; completed finite media and
interface slices remain supported within their recorded limits.

| ID | Status | Current outcome | Remaining scope |
| --- | --- | --- | --- |
| T1 | Next | Existing original TLS/X.509 and explicit browser-owned PEM trust data form the baseline | Verify shared trust/data authority and certificate coverage across ordinary document/resource paths, preserving validation and actionable failures |
| B1 | Queued after T1 | Existing resource, renderer and input paths are partial foundations | Address demonstrated everyday stylesheet/script/font/image, layout and interaction failures with real request-to-pixel/input proof |
| B2 | Queued after page usability | No new tab or window capability is claimed | Define finite tabs, link-context and new-window behavior with real context/lifetime ownership |
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

Next, establish the shared TLS/trust/data coverage baseline, then improve
ordinary resource loading, rendering and interaction before finite tabs/window
work. VIDEO dimensions, Element.toggleAttribute, NamedNodeMap mutation and
remaining media/player work stay recorded gaps outside the active queue.
Service playback remains unproved by local media evidence.

Wider Unicode, CSS, storage, frames and accessibility remain gaps. Complete Maps,
every codec and perfect site screenshots are not prerequisites for useful staged
progress. Security defects and demonstrated prerequisites can interrupt the active
slice. Detailed behavior and acceptance belong in the
[feature contracts](FEATURE_CONTRACTS.md); evidence boundaries belong in the
[capability matrix](CAPABILITIES.md).
