# Capability and dependency matrix — 0.79.0-alpha.2

Every broad area remains partial. “Verified” below means the specified local slice,
not complete platform conformance. Source inspection, unit tests, App harness,
physical desktop/device evidence and live-service outcomes are different scopes.

| Area | Implemented and locally verified slice | Gaps / next acceptance | Main implementation and checks |
| --- | --- | --- | --- |
| Navigation/search/history | HTTP(S)/files, query encoding, Google/Bing/DuckDuckGo/None setting, local result/destination/one-Back/second-query journeys | Response/session-dependent live behavior; define finite redirect/history regression contract | src/navigation.rs, src/search.rs, src/app_session_history.rs; check_input_harness.py, check_address_search.py |
| HTTP/TLS/resources | Original transport, trust validation, redirects, decompression, workers, measured timings | No full protocol/browser security claim; network-media transaction remains missing | src/http.rs, src/tls.rs, src/x509.rs, src/loader.rs; native loopback/negative tests |
| Privacy/storage/files | Memory-only cookies, scoped profiles/bookmarks, optional history, staged downloads and local roots | No site storage/service workers/persistent cookies; local files are not encrypted vaults | src/cookies.rs, src/profile.rs, src/history.rs, src/downloads.rs |
| DOM/interfaces | Real mutation/Attr identity, selectors/collections, base Node/Element ancestry, distinct Image | Specialized prototypes/constructors, wider child contexts and DOM APIs missing | src/script_dom_tree.rs, src/script_attr.rs, src/host_objects.rs; local namespace/Attr journey |
| JavaScript/realms | Original language/built-ins/promises/jobs, owner-isolated intrinsic roots and replaceable real main-Window animation operations | UTF-16 code units, modules, fetch/XHR and many interfaces incomplete | src/javascript.rs, src/intrinsic_realms.rs, src/promises.rs, src/scripting.rs; normal suite, frozen references and check_animation_harness.py |
| Events | Genuine standalone EventTarget, synchronous Event/UIEvent dispatch, Text/Attr paths and native inputs | Legacy CustomEvent factory, constructible event families, broader activation missing | src/script_dispatch.rs, src/script_created_events.rs; dispatch journey |
| CSS/layout/geometry | Loaded styles, token selectors, measured supported geometry/scrollHeight, clipping and software paint | Broader layout/font policy; unsupported reverse/viewport overflow measurements remain explicit | src/css.rs, src/layout.rs, src/script_geometry.rs; stylesheet/render journeys |
| Fonts/images | Original selected font/image decoders and resource/presentation paths | Format subsets and cross-origin font loading need specific contracts | src/fonts.rs, src/image.rs and format modules; original pixel/negative fixtures |
| Media | Local IVF/VP8 intra, PCM, constrained Matroska A/V; one authored video with src/load lifecycle | Script pause/seek/play promises, element audio/network and YouTube playback missing | src/media.rs, src/audio.rs, src/matroska.rs, src/script_media.rs; four media journeys plus source replacement |
| Browser UI | Native settings, address input, bookmarks/history/download controls and local input paths | Full accessibility, richer UI and native locked-session presentation unverified | src/app*.rs, src/ui.rs, src/platform.rs; App/input journeys |
| Diagnostics | Source/activity/layout/pixels, inspector, measured network phases and finite local tooling | Reports can contain private URLs/paths/session data; titles/200/errors alone are not acceptance | src/diagnostics.rs, src/network_trace.rs; offline harness reports |
| Platform/isolation | Windows x64, explicit FFI ownership, worker retirement/cancellation and watchdogs | No OS renderer sandbox; independent native/crypto/aggregate-memory assurance remains incomplete | src/platform.rs, src/audio_output.rs; native negative tests and PE checks |
| Build/release | std-only production manifest, generated resources, pinned compiler and offline local gates | Each release requires an exact tagged build, complete payload, extracted-runtime checks and downloadable-asset verification | Build.cmd, tools/verify_offline.py, docs/RELEASING.md |

Google Search, Images and Maps are not fully supported. YouTube page compatibility
is limited, and playback is unsupported.

Main-window animation operations support replacement and real callback scheduling
and cancellation. Child-frame animation schedulers are unsupported. See the
[F1 feature contract](FEATURE_CONTRACTS.md#f1--replaceable-animation-frame-operations)
for the technical scope and acceptance criteria.
