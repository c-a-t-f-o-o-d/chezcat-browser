# Capability and dependency matrix — 0.80.0-alpha.1

Every broad area remains partial. “Verified” below means the specified local slice,
not complete platform conformance. Source inspection, unit tests, App harness,
physical desktop/device evidence and live-service outcomes are different scopes.

| Area | Implemented and locally verified slice | Gaps / next acceptance | Main implementation and checks |
| --- | --- | --- | --- |
| Navigation/search/history | HTTP(S)/files, query encoding, Google/Bing/DuckDuckGo/None setting, local result/destination/one-Back/second-query journeys; request-time bridge replacement with bounded native activation | Live behavior remains response/session-dependent | src/navigation.rs, src/search.rs, src/app_session_history.rs, src/script_navigation.rs; check_input_harness.py, check_address_search.py, check_navigation_harness.py |
| HTTP/TLS/resources | Original transport, trust validation, redirects, decompression, workers, measured timings | No full protocol/browser security claim; media remains limited to complete transfers and explicit authored CORS modes | src/http.rs, src/tls.rs, src/x509.rs, src/loader.rs; native loopback/negative tests |
| Privacy/storage/files | Memory-only cookies, scoped profiles/bookmarks, optional history, staged downloads and local roots | No site storage/service workers/persistent cookies; local files are not encrypted vaults | src/cookies.rs, src/profile.rs, src/history.rs, src/downloads.rs |
| DOM/interfaces | Real mutation/Attr identity, selectors/collections, base Node/Element ancestry, distinct Image and genuine realm-scoped DOMException | Specialized prototypes/constructors, wider child contexts and DOM APIs missing | src/script_dom_tree.rs, src/script_attr.rs, src/host_objects.rs, src/dom_exception.rs; local namespace/Attr journey and exception originals |
| JavaScript/realms | Original language/built-ins/promises/jobs, owner-isolated intrinsic roots and replaceable real main-Window animation operations | UTF-16 code units, modules, fetch/XHR and many interfaces incomplete | src/javascript.rs, src/intrinsic_realms.rs, src/promises.rs, src/scripting.rs; normal suite, frozen references and check_animation_harness.py |
| Events | Genuine standalone EventTarget, synchronous Event/UIEvent dispatch, legacy CustomEvent with actual detail identity, Text/Attr paths and native inputs | Wider event constructors/prototypes, child-document factories and broader activation remain incomplete | src/script_dispatch.rs, src/script_created_events.rs; dispatch journey |
| CSS/layout/geometry | Loaded styles, token selectors, measured supported geometry/scrollHeight, clipping, software paint and one URL background layer on ordinary block/inline-block boxes | Inline-fragment/replaced-element backgrounds, multiple layers, gradients and broader layout/font policy remain incomplete; unsupported reverse/viewport overflow measurements stay explicit | src/css.rs, src/layout.rs, src/script_geometry.rs; stylesheet/render journeys |
| Fonts/images | Original selected font/image decoders and resource/presentation paths | Format subsets and cross-origin font loading need specific contracts | src/fonts.rs, src/image.rs and format modules; original pixel/negative fixtures |
| Media | Local IVF/VP8 visible fixed-canvas I/P (version-zero single-MV interframes), PCM, constrained Matroska A/V with dependency-aware seeking; one authored IVF or supported Matroska video with source/load lifecycle, script controls and play promises tied to decoded-frame/acknowledged PCM readiness; complete HTTP(S) IVF/Matroska transfers chosen by explicit MIME, with authored CORS/credential modes | SPLITMV, hidden/nonzero-version interframes, WebM/Opus/Vorbis, unmarked foreign media, dynamic crossOrigin controls, streaming/ranges, muted Matroska, multiple players, MSE and YouTube playback missing | src/media.rs, src/audio.rs, src/matroska.rs, src/script_media.rs; local media/source-replacement journeys and check_authored_video_harness.py and check_network_media_harness.py; check_authored_av_harness.py and check_network_authored_av_harness.py |
| Browser UI | Native settings, address input, bookmarks/history/download controls and local input paths | Full accessibility, richer UI and native locked-session presentation unverified | src/app*.rs, src/ui.rs, src/platform.rs; App/input journeys |
| Diagnostics | Source/activity/layout/pixels, inspector, measured network phases and finite local tooling | Reports can contain private URLs/paths/session data; titles/200/errors alone are not acceptance | src/diagnostics.rs, src/network_trace.rs; offline harness reports |
| Platform/isolation | Windows x64, explicit FFI ownership, worker retirement/cancellation and watchdogs | No OS renderer sandbox; independent native/crypto/aggregate-memory assurance remains incomplete | src/platform.rs, src/audio_output.rs; native negative tests and PE checks |
| Build/release | std-only production manifest, generated resources, pinned compiler and offline local gates | Each release requires an exact tagged build, complete payload, extracted-runtime checks and downloadable-asset verification | Build.cmd, tools/verify_offline.py, docs/RELEASING.md |

The original authored-video App/input/offscreen journey checks a trusted script
play click, synchronous state, one playing notification before fulfillment,
measured clock advancement and decoded pixels against independent clip frames.
It also checks script pause/seek, frozen time/pixels, native controls, EOF/replay,
Stop and navigation. The network journey repeats those assertions over original
loopback HTTP and authenticated TLS 1.3, checking complete response identities and
decoded pixels. Original App/transport tests cover policy refusal, MIME/status/
partial-response errors, cancellation, replacement and stale completions. This
also includes real App cross-origin playback and credential/header/cookie
controls. The TLS-refusal executable journey proves untrusted issuer and hostname
failures prevent media HTTP requests and player installation, while rendering the
actual error event. This finite evidence does not establish general Fetch/CORS,
general audio-device or physical-desktop acceptance.

The finite interframe journey reuses those real controls and source-lifecycle
checks, visits every original frame across two keyframe groups in IVF and
Matroska, and repeats HTTP(S) video playback. Independent native VP8 YUV and
explicit centered-bilinear BT.601 display references separate decoding from
presentation. The display profile requires mean RGB error below0.5 and maximum
channel error2. Codec tests cover entropy/reference updates, segment retention,
rollback, cancellation and bounded replay work. An explicitly requested WinMM
journey additionally checks acknowledged sample advancement, pause, seek, completed
PCM drain, replay and decoded video alignment. Routine verification keeps devices
closed; neither check claims physical audibility or live-service playback.

Google Search, Images and Maps are not fully supported. YouTube page compatibility
is limited, and playback is unsupported.

Main-window animation operations support replacement and real callback scheduling
and cancellation. Child-frame animation schedulers are unsupported. See the
[F1 feature contract](FEATURE_CONTRACTS.md#f1--replaceable-animation-frame-operations)
for the technical scope and acceptance criteria.
