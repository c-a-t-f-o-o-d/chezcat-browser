# Finite development roadmap

The current milestone is **0.80.0-alpha.1: explicit CSAPUL 1.1 acceptance**.
Its implemented offline acceptance and execution gates are complete; publication
still requires the exact release build and package checks in the
[release process](RELEASING.md). Status below describes finite implemented slices,
not complete web-platform or public-service support.

| ID | Status | Current outcome | Remaining scope |
| --- | --- | --- | --- |
| L1 | Complete | Full offline native license text and explicit acceptance; personal/organization source and headless acceptance; strict external receipts, decline and changed-terms checks | No identity or organizational-authority verification; exact release packaging remains a separate gate |
| F1 | Complete for the main Window | Replaceable animation operations with real scheduling, cancellation, owner and work rules | Child-frame schedulers |
| F2 | Complete for the original local journey | Address/search intent, result/destination/one-Back/different-query input and startup redirect/replacement handling | Live behavior remains response/session-dependent; no every-response Google promise |
| F3 | Complete for one supported authored video | Real load, pause, currentTime seek and play promises/events tied to decoded output, with native controls and source retirement | Wider HTMLMediaElement interfaces, multiple players and unsupported profiles |
| F4 | Complete for bounded transfers | HTTP(S) IVF/Matroska playback with explicit MIME, redirects, credentials/CORS, cookies, CSP, TLS refusal and cancellation | Streaming/ranges, unmarked foreign media and dynamic crossOrigin controls |
| F5 | Complete for the selected profile | Visible version-zero VP8 I/P single-vector interframes, dependency-aware seek and constrained Matroska PCM16 synchronization | SPLITMV, hidden frames, other interframe versions and wider codecs/containers |
| C1 | Complete for one URL background layer | Stylesheet-relative resource ownership, shared cascade/load/layout/paint and original request/pixel/geometry checks on ordinary block boxes | Multiple layers, gradients and inline-fragment/replaced-element backgrounds |
| F6 | In progress | Authored synchronized A/V and finite genuine Event/UIEvent/MouseEvent construction are implemented prerequisites; Event/UIEvent dispatch and legacy CustomEvent detail/dispatch work | Actual ordinary non-DRM YouTube playback is unproved; Mouse dispatch/native pointer payloads, KeyboardEvent/CustomEvent constructors and broader page/player APIs remain unsupported |

Next, extend the authored-player interfaces needed by the selected page path,
then address demonstrated event/page dependencies with original fixtures. Each
slice needs its own model, App/input and lifecycle acceptance before a separately
authorized service check. Service playback is not inferred from local media.

Wider Unicode, CSS, storage, frames and accessibility remain gaps. Complete Maps,
every codec and perfect site screenshots are not prerequisites for useful staged
progress. Security defects and demonstrated prerequisites can interrupt the active
slice. Detailed behavior and acceptance belong in the
[feature contracts](FEATURE_CONTRACTS.md); evidence boundaries belong in the
[capability matrix](CAPABILITIES.md).
