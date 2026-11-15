# Capability and dependency matrix — 0.80.0-alpha.1

Every broad area remains partial. “Verified” below means the specified local slice,
not complete platform conformance. Source inspection, unit tests, App harness,
physical desktop/device evidence and live-service outcomes are different scopes.

Current priority is shared TLS/trust/data coverage, everyday resource/render/
interaction usability, then finite tabs, link context and new windows. Remaining
F6/YouTube integration is deferred. This priority records no new implementation
or public-service acceptance; completed local media evidence remains valid.

| Area | Implemented and locally verified slice | Gaps / next acceptance | Main implementation and checks |
| --- | --- | --- | --- |
| License acceptance | Full offline native terms and explicit personal/organization acceptance; source/headless commands, strict minimal external receipts, same-text reuse and changed-terms/decline/write-failure refusal before browsing | Records a local choice, not verified identity or organizational authority; no account service is required | src/license_acceptance.rs, src/platform_license.rs, tools/accept_license.py, tools/check_license_acceptance.py; docs/LICENSE_ACCEPTANCE.md |
| Navigation/search/history | HTTP(S)/files, query encoding, Google/Bing/DuckDuckGo/None setting, local result/destination/one-Back/second-query journeys; request-time bridge replacement with bounded native activation | Live behavior remains response/session-dependent | src/navigation.rs, src/search.rs, src/app_session_history.rs, src/script_navigation.rs; check_input_harness.py, check_address_search.py, check_navigation_harness.py |
| HTTP/TLS/resources | Original transport, explicit local PEM trust selection, certificate validation, redirects, decompression, workers and measured timings | Remaining: shared trust/data and ordinary-resource coverage, with demonstrated certificate/chain gaps; bounded document diagnostics are a prerequisite, not a completed trust expansion. No OS trust-store lookup, root auto-update, revocation/CT or full browser-security claim; media remains bounded complete transfers with explicit CORS modes | src/http.rs, src/trust.rs, src/tls.rs, src/x509.rs, src/loader.rs; native loopback/negative tests |
| Privacy/storage/files | Memory-only cookies, scoped profiles/bookmarks, optional history, staged downloads and local roots | No site storage/service workers/persistent cookies; local files are not encrypted vaults | src/cookies.rs, src/profile.rs, src/history.rs, src/downloads.rs |
| DOM/interfaces | Real mutation/Attr identity, selectors/collections, base Node/Element ancestry, distinct Image and genuine realm-scoped DOMException | Specialized prototypes/constructors, wider child contexts and DOM APIs missing | src/script_dom_tree.rs, src/script_attr.rs, src/host_objects.rs, src/dom_exception.rs; local namespace/Attr journey and exception originals |
| JavaScript/realms | Original language/built-ins/promises/jobs, owner-isolated intrinsic roots and replaceable real main-Window animation operations | UTF-16 code units, modules, fetch/XHR and many interfaces incomplete | src/javascript.rs, src/intrinsic_realms.rs, src/promises.rs, src/scripting.rs; normal suite, frozen references and check_animation_harness.py |
| Events | Genuine owner-realm Event/UIEvent/MouseEvent/CustomEvent constructors, prototypes and stored fields; actual any CustomEvent detail and atomic initialization/publication; synchronous Event/UIEvent/CustomEvent dispatch/cancellation and independent path arrays; standalone EventTarget, Text/Attr paths and native inputs | Mouse dispatch/default activation and native pointer payloads, KeyboardEvent construction and native-input immediate prototypes, child-document factories and shadow retargeting remain unsupported | src/event_interface.rs, src/script_dispatch.rs, src/script_created_events.rs; original Event/UI/Mouse/Custom interface tests and dispatch journey |
| CSS/layout/geometry | Loaded styles, token selectors, measured supported geometry/scrollHeight, clipping, software paint and one URL background layer on ordinary block/inline-block boxes | Inline-fragment/replaced-element backgrounds, multiple layers, gradients and broader layout/font policy remain incomplete; unsupported reverse/viewport overflow measurements stay explicit | src/css.rs, src/layout.rs, src/script_geometry.rs; stylesheet/render journeys |
| Fonts/images | Original selected font/image decoders and resource/presentation paths | Format subsets and cross-origin font loading need specific contracts | src/fonts.rs, src/image.rs and format modules; original pixel/negative fixtures |
| Media | Local IVF/VP8 visible fixed-canvas I/P (version-zero single-MV interframes), PCM, constrained Matroska A/V with dependency-aware seeking; one authored IVF or supported Matroska video with source/load lifecycle, script controls and play promises tied to decoded-frame/acknowledged PCM readiness; complete HTTP(S) IVF/Matroska transfers chosen by explicit MIME, with authored CORS/credential modes | SPLITMV, hidden/nonzero-version interframes, WebM/Opus/Vorbis, unmarked foreign media, dynamic crossOrigin controls, streaming/ranges, muted Matroska, multiple players, MSE and YouTube playback missing | src/media.rs, src/audio.rs, src/matroska.rs, src/script_media.rs; local media/source-replacement journeys and check_authored_video_harness.py and check_network_media_harness.py; check_authored_av_harness.py and check_network_authored_av_harness.py |
| Media interfaces | Genuine owner-realm HTMLMediaElement/HTMLVideoElement roots, constants and slot accessors; stable VIDEO class identity and canonical load/pause/play/src/currentTime transactions through ordinary shadows; actual Boolean controls reflection and native bar/input behavior without source/clock/frame/Promise reset | One bound player; controls reflects genuine main HTML VIDEO/AUDIO attributes without a player, while unbound playback calls give real NotSupportedError outcomes and other playback-state getters/setters decline. VIDEO dimensions, extra/child players and wider media APIs remain unsupported; Element.toggleAttribute and NamedNodeMap mutation are separate gaps | src/media_interface.rs, src/script_media.rs, src/host_objects.rs, src/app_media.rs; original media-interface/controls tests, check_media_interface_harness.py and check_media_controls_harness.py |
| Browser UI | Native settings, address input, bookmarks/history/download controls and local input paths | Full accessibility, richer UI and native locked-session presentation unverified | src/app*.rs, src/ui.rs, src/platform.rs; App/input journeys |
| Diagnostics | Source/activity/layout/pixels and inspector; document-only connection reports with optional exact-host checks before DNS on each hop, actual destination/phase evidence, and escaped bounded metadata; eight new native diagnostics regressions verified | No App/profile, page scripts, subresources or usability assessment in --diagnose. Status and possible body markers do not establish access/challenge outcomes. Reports can contain private URLs/paths/session data; full normal integration and public-host checks remain separate | src/diagnostics.rs, src/diagnostics_tests.rs, src/network_trace.rs; original loopback/guard/metadata tests and offline harness reports |
| Platform/isolation | Windows x64, explicit FFI ownership, worker retirement/cancellation and watchdogs | No OS renderer sandbox; independent native/crypto/aggregate-memory assurance remains incomplete | src/platform.rs, src/audio_output.rs; native negative tests and PE checks |
| Build/release | std-only production manifest, generated resources, pinned compiler and offline local gates | Each release requires an exact tagged build, complete payload, extracted-runtime checks and downloadable-asset verification | Build.cmd, tools/verify_offline.py, docs/RELEASING.md |

## Evidence boundaries

| Evidence type | What it establishes | What it does not establish |
| --- | --- | --- |
| Original model/unit tests | Specific conversions, brands, owners, queues, failure and retirement rules | Full standards conformance or visible application behavior |
| Document-only diagnostics | Original transport response, trust selection, guarded destinations and measured native phases for the actual attempt | Page JavaScript/subresources, readability, interaction, challenge clearance or completed trust-data expansion |
| Actual App/input/offscreen journeys | Real application input, script state/events, source/layout/canvas identity, decoded pixels and finite HTTP/TLS response checks | Physical desktop presentation, speaker output or public-service compatibility |
| Hosted offline CI | The recorded commit's build, normal checks and local fixture journeys on its runner | A later commit, release payload, hardware output or live service |
| Explicit opt-in WinMM checks | Device acknowledgements, PCM sample advancement, pause/seek/drain/replay and video alignment for the selected profile | Physical audibility, arbitrary devices or routine no-device CI |
| Physical desktop / live service | Requires separate, explicit evidence for the actual session and outcome | Not inferred from source inspection, HTTP status, titles or offscreen pixels |

The finite CustomEvent interface journey uses two trusted base-input clicks to
construct and deliver real untrusted notifications, retaining any detail
identity, canonical initialization through own shadows, capture/target/bubble
cancellation and independent path copies. Genuine child constructor roots and
child-realm detail are checked over owned HTTP. Readable results have current
zero interpreter errors and paired document/DOM/layout/canvas pixels. The original
legacy notification and stack/base/UI/Mouse cases remain. Model tests separately
cover ordered conversion, foreign-owner guards, combined retention, newest-only
unpublished rollback, bounded refusal and ordinary/fatal recovery. Normal
verification remains required for integration; this establishes no device,
physical-desktop or public-service behavior.

The finite Media/Video interface journey drives saved canonical Play/load/pause
and the currentTime setter despite own method/data/accessor shadows. It reuses
the original real clock, independent frame pixels, Promise/event order, frozen
pause and seek, source identity, EOF/replay, Stop and navigation checks, with
paired document/DOM/layout/canvas observations and no audio device. Model tests
separately cover descriptor/brand rules, saved/fresh/cached owner guards,
source-selection FIFO, no-binding refusal and bounded getter-work recovery.
This completes that finite interface/App slice. Every implementation checkpoint
also requires normal verification; broader F6/service acceptance remains deferred.

The controls-reflection journey keeps hidden and restored native bars in
distinct actual snapshots. It checks decoded pixels and independent frame time,
the exposed image bottom strip versus exact bar pixels, real hit queries,
hidden/unfocused Space refusal and restored focused native pause/freeze. A Play
Promise survives removal and fulfills once; raw source Attr, selected source,
clock and frame lifetime persist with one full loopback clip fetch. Original App
controls separately verify armed-press/focus cancellation at removal reflow and
pending-seek preservation. These are App/input/offscreen and model proofs, with
paired document/DOM/layout/canvas observations, zero current interpreter errors
and a closed audio owner. Normal integration verification remains required;
physical OS input, audible output and service compatibility are separate.

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
