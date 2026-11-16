# Capability and dependency matrix — 0.80.0-alpha.1

Every broad area remains partial. “Verified” below means the specified local slice,
not complete platform conformance. Source inspection, unit tests, App harness,
physical desktop/device evidence and live-service outcomes are different scopes.

Current work selects evidenced shared platform blockers through original browser
workflows and unmodified local libraries. Delivered CP1 supplies the shared
collection/call/Node prerequisites. CP2 adds genuine native-created Comments and
existing RegExp input ToString; focused Page/App/language controls pass, while
normal/build/executable integration stays required. CP3 supplies a measured
responsive Foundation6.9 rendering improvement with the original JS-enabled
fixture: two columns at1280 and stacking at640. TLS/trust and resource/render
slices remain partial baselines. Finite tabs/new windows follow the first measured
workflow improvement. See the [compatibility scope](COMPATIBILITY.md) and its
machine-readable [catalogue](compatibility.json).
A finite About/build-description panel is implemented at this UI checkpoint.
CP4 adds conventional select fragments backed by real native controls and
projected ownership, with twelve parser/model/Page/App controls passing. Whole
container retirement removes popup/focus/hit/submission participation while
retaining identities and edited values for reconnection. Exact-source full
verification and the unchanged pinned legacy workflow recheck remain required;
no framework initialization or visible workflow improvement is claimed.
The B2 profile prerequisite now provides one shared writer and immutable App
snapshots with captured editor/gesture revisions. Original two-App tests retain
live VM/form/history/viewport state across peer refresh and refuse stale bookmark
identities. These authorities now support the finite Session/native tab shell.
The download prerequisite now retains one browser-owned Manager with weak App
views and explicit startup attachment, while preserving legacy single-window
ownership. Original held transfers survive the originating App, remain visible
to a peer and use browser wake routing. Saved history now has a browser authority
with weak App handles, preserved selected-entry identity and checked policy
edits. Attached Apps cannot clear history on individual close; explicit final
shutdown belongs to the browser authority. An internal Session core now owns real
boxed Apps, checked process-scoped tab/window IDs, bounded polling and consuming
page retirement. Original tests exercise independent live page state, held
resources, stale wakes and final-only history cleanup. The native shell adds a
software tab strip, new/switch/close actions, Ctrl+T/W/Tab/Shift+Tab/N, real window
creation and tab-pinned detached inspectors. Original pure UI and task-created
hidden-HWND controls cover input ownership, reentry, scheduling, text purge and
history retry. Full normal/optimized integration is required at each checkpoint;
these controls do not establish physical desktop interaction. Native link gestures
and explicit foreground/background/window context commands now use checked Session
admission and source-bound intents.
Passive popup hover/leave retains the original link target; dragging and all five
held-button move bits still invalidate it, even without native capture. This
lifetime correction preserves the existing source/document and input guards.
The engine provides genuine trusted anchor click/auxclick,
guarded App hit/context tokens and owned resolved link targets with source cookie
initiators. Live HTML href/download mutations use the existing checked DOM/Attr
path; rendering and link styling read the live href. Original focused controls
cover cancellation, source authority, mutations and obscuring. Direct background
admission preserves source state. Native release envelopes, capture generations
and popup origin/completion checks cover reentry and stale input. Complete normal,
optimized and desktop acceptance remains required for every checkpoint.
One per-user installer and staged release discovery/upgrade remain planned in the
[roadmap](ROADMAP.md), not delivered capabilities. Further
F6/YouTube work is deferred; completed finite media evidence remains valid.

| Area | Implemented and locally verified slice | Gaps / next acceptance | Main implementation and checks |
| --- | --- | --- | --- |
| License acceptance | Full offline native terms and explicit personal/organization acceptance; source/headless commands, strict minimal external receipts, same-text reuse and changed-terms/decline/write-failure refusal before browsing | Records a local choice, not verified identity or organizational authority; no account service is required | src/license_acceptance.rs, src/platform_license.rs, tools/accept_license.py, tools/check_license_acceptance.py; docs/LICENSE_ACCEPTANCE.md |
| Navigation/search/history | HTTP(S)/files, query encoding, Google/Bing/DuckDuckGo/None setting, local result/destination/one-Back/second-query journeys; request-time bridge replacement with bounded native activation | Live behavior remains response/session-dependent | src/navigation.rs, src/search.rs, src/app_session_history.rs, src/script_navigation.rs; check_input_harness.py, check_address_search.py, check_navigation_harness.py |
| HTTP/TLS/resources | Original transport and certificate validation; bounded deduplicated IPv4/IPv6 interleaving, eight serial attempts with deadline reservation for the alternative family and redacted per-attempt diagnostics; explicit local PEM stores with shared 1024-root capacity; a pinned conservative 98-authority Mozilla NSS selection, offline generation/source-identity gates and native CA/time/self-signature checks; bounded per-hop/overall redirect deadlines, decompression, workers and measured timings | Coordinated document checks show that some previously refused certificate paths now validate; chain/parser/protocol/timeout/admission and resource-specific gaps remain. The data subset is not full Firefox policy or broad TLS compatibility; no OS trust-store lookup, root auto-update, revocation/CT or security-certification claim. Media remains bounded complete transfers with explicit CORS modes | src/http.rs, src/trust.rs, src/tls.rs, src/x509.rs, src/loader.rs; tools/generate_trust.py, trust/mozilla/selection.json; native signed-path/negative and data-tool tests |
| Privacy/storage/files | Memory-only cookies, scoped profiles/bookmarks, optional history, staged downloads and local roots | No site storage/service workers/persistent cookies; local files are not encrypted vaults | src/cookies.rs, src/profile.rs, src/history.rs, src/downloads.rs |
| DOM/interfaces | Real mutation/Attr identity, selectors/collections, stable live main-Document/supported Node childNodes, checked native array-like apply, genuine Document Node accessors, text replacement and native-created Comment leaves; conventional select fragments with staged native ownership and retained whole-select state | The prior legacy workload refused select-containing innerHTML; its CP4 recheck and full integration remain required. No initialization/visible-improvement claim. New explicit select form associations, existing-control interior edits, createElement(select), hidden-input admission, parsed comments, full CharacterData, DocumentType/template-content and child-Document collections remain unsupported | src/script_queries.rs, src/script_dom_tree.rs, src/scripting.rs, src/script_select_fragments.rs, src/forms_select_fragments.rs, src/array_ops.rs; original parser/native-model/Page/App select-fragment and existing namespace/Attr/owner controls |
| JavaScript/realms | Original language/built-ins/promises/jobs, actual native RegExp exec/test input ToString through existing callback/Function-source rules, owner-isolated intrinsic roots and replaceable real main-Window animation operations | UTF-16 code units, generic RegExpExec/custom exec and object-valued lastIndex, modules, fetch/XHR and many interfaces incomplete | src/javascript.rs, src/intrinsic_realms.rs, src/promises.rs, src/scripting.rs; normal suite, frozen references and check_animation_harness.py |
| Events | Genuine owner-realm Event/UIEvent/MouseEvent/CustomEvent constructors and branded fields; real Event/UIEvent/CustomEvent dispatch/cancellation; finite native anchor click/auxclick with actual host payload, frozen listener paths and microtask cancellation | Synthetic Mouse dispatch, general native pointer delivery, KeyboardEvent construction/specialized native keyboard prototypes, child-document factories and shadow retargeting remain unsupported | src/event_interface.rs, src/script_dispatch.rs, src/script_created_events.rs, src/script_anchor_mouse.rs; original interface, anchor and dispatch controls |
| CSS/layout/geometry | Loaded styles, token selectors, finite host-context viewport/hover media groups, measured supported geometry/scrollHeight, clipping, software paint and one URL background layer on ordinary block/inline-block boxes; original native/App resize/zoom, pixels and link-hit checks | Normal verification remains required for each integration checkpoint. Conditional font faces, dynamic/resize font fetching, wider media-query/attribute grammar, inline-fragment/replaced backgrounds, multiple layers and broader layout remain incomplete; unsupported reverse/viewport overflow measurements stay explicit | src/css.rs, src/css_media.rs, src/layout.rs, src/script_geometry.rs; original media-query/initial-font/App checks and stylesheet/render journeys |
| External stylesheets | Shared ordinary unmarked HTTP(S) parser/dynamic loading, final CSS provenance and bounded response accounting; broader page usability remains in progress | Per-hop CSP/request/mixed-content, cookie, MIME/CORP/COEP, cache and cancellation rules remain; no general stylesheet CORS, imports or CSSOM source access claim | src/http.rs, src/loader.rs, src/script_stylesheets.rs, src/app_stylesheets.rs; B1 contract and original local request/pixel/lifecycle fixtures |
| Fonts/images | Original selected font/image decoders and resource/presentation paths | Format subsets and cross-origin font loading need specific contracts | src/fonts.rs, src/image.rs and format modules; original pixel/negative fixtures |
| Media | Local IVF/VP8 visible fixed-canvas I/P (version-zero single-MV interframes), PCM, constrained Matroska A/V with dependency-aware seeking; one authored IVF or supported Matroska video with source/load lifecycle, script controls and play promises tied to decoded-frame/acknowledged PCM readiness; complete HTTP(S) IVF/Matroska transfers chosen by explicit MIME, with authored CORS/credential modes | SPLITMV, hidden/nonzero-version interframes, WebM/Opus/Vorbis, unmarked foreign media, dynamic crossOrigin controls, streaming/ranges, muted Matroska, multiple players, MSE and YouTube playback missing | src/media.rs, src/audio.rs, src/matroska.rs, src/script_media.rs; local media/source-replacement journeys and check_authored_video_harness.py and check_network_media_harness.py; check_authored_av_harness.py and check_network_authored_av_harness.py |
| Media interfaces | Genuine owner-realm HTMLMediaElement/HTMLVideoElement roots, constants and slot accessors; stable VIDEO class identity and canonical load/pause/play/src/currentTime transactions through ordinary shadows; actual Boolean controls reflection and native bar/input behavior without source/clock/frame/Promise reset | One bound player; controls reflects genuine main HTML VIDEO/AUDIO attributes without a player, while unbound playback calls give real NotSupportedError outcomes and other playback-state getters/setters decline. VIDEO dimensions, extra/child players and wider media APIs remain unsupported; Element.toggleAttribute and NamedNodeMap mutation are separate gaps | src/media_interface.rs, src/script_media.rs, src/host_objects.rs, src/app_media.rs; original media-interface/controls tests, check_media_interface_harness.py and check_media_controls_harness.py |
| Browser UI | Native settings, address input, bookmarks/history/download controls; finite About panel; Session tabs/windows/pinned inspectors; guarded primary/middle links and explicit foreground/background/window context commands | Complete normal/optimized and desktop acceptance is required per checkpoint. Hidden HWND/pure controls do not establish physical desktop interaction. Named contexts, script-opened windows, full accessibility and native locked-session presentation remain gaps. Copy capture does not prove OS clipboard delivery | src/app*.rs, src/build_identity.rs, src/ui.rs, src/platform*.rs; original About/chrome, Session admission/input and owned native-window controls |
| Diagnostics | Source/activity/layout/pixels and inspector; document-only connection reports with optional exact-host checks before DNS on each hop, actual destination/phase evidence, and escaped bounded metadata; eight new native diagnostics regressions verified | No App/profile, page scripts, subresources or usability assessment in --diagnose. Status and possible body markers do not establish access/challenge outcomes. Reports can contain private URLs/paths/session data; full normal integration and public-host checks remain separate | src/diagnostics.rs, src/diagnostics_tests.rs, src/network_trace.rs; original loopback/guard/metadata tests and offline harness reports |
| Platform/isolation | Windows x64, explicit FFI ownership, worker retirement/cancellation and watchdogs | No OS renderer sandbox; independent native/crypto/aggregate-memory assurance remains incomplete | src/platform.rs, src/audio_output.rs; native negative tests and PE checks |
| Build/release | std-only production manifest, generated resources, pinned compiler, portable payload and offline local gates | Per-user MSI and opt-in release discovery/authenticated upgrades are planned, not implemented. Each release requires an exact tagged build, complete payload, extracted-runtime checks and downloadable-asset verification | Build.cmd, tools/verify_offline.py, docs/RELEASING.md |

TCP fallback retains candidates from both families within a 256-entry resolver
scan and eight-attempt queue; resolver preference and within-family order remain.
Scan exhaustion reports remaining results as unknown. Blocking OS DNS, up to one
two-second connect before cancellation, and concurrent Happy Eyeballs remain
limitations. Offline simulated failures and real local sockets establish this
scope; external IPv6 or paused-service success is not claimed. The original
hostname still drives TLS, HTTP Host and origin/cookie/redirect policy.

## Evidence boundaries

| Evidence type | What it establishes | What it does not establish |
| --- | --- | --- |
| Original model/unit tests | Specific conversions, brands, owners, queues, failure and retirement rules | Full standards conformance or visible application behavior |
| Trust-data and native certificate checks | Exact reviewed data reproduction, source/policy pins, all selected anchors' supported forms/CA/time/self-signatures, retained identities and original signed-path refusals | Complete Firefox policy, revocation/CT, general TLS interoperability or a public website's actual behavior |
| Document-only diagnostics | Original transport response, trust selection, guarded destinations and measured native phases for the actual attempt | Page JavaScript/subresources, readability, interaction, challenge clearance or an audit of the complete trust selection |
| Actual App/input/offscreen journeys | Real application input, script state/events, source/layout/canvas identity, decoded pixels and finite HTTP/TLS response checks | Physical desktop presentation, speaker output or public-service compatibility |
| App build-information copy capture | Actual typed safe metadata action returned by owned About keyboard/pointer controls, captured without changing the OS clipboard | Successful native clipboard delivery, authenticated publisher/source/binary identity or remote publication |
| Pinned local library/workflow checks | Exact selected assets, first-root versus cascaded errors, and separately observed initialization/input/navigation/layout stages | Whole-library/version-family compatibility, unrelated deployment output, or a public service |
| Initial reference-browser DOM and pixels | An independent initial-state comparison for the original fixture at the recorded viewport | Reference interactions, cross-browser conformance or a successful ChezCat workflow |
| Hosted offline CI | The recorded commit's build, normal checks and local fixture journeys on its runner | A later commit, release payload, hardware output or live service |
| Explicit opt-in WinMM checks | Device acknowledgements, PCM sample advancement, pause/seek/drain/replay and video alignment for the selected profile | Physical audibility, arbitrary devices or routine no-device CI |
| Physical desktop / live service | Requires separate, explicit evidence for the actual session and outcome | Not inferred from source inspection, HTTP status, titles or offscreen pixels |

The default trust bundle selects 98 authorities from pinned Mozilla NSS 2.94
data with separate MPL notices. Four leaf-issuance cutoffs, one externally imposed
name restriction, ten unsupported forms and 54 non-server-auth entries are
excluded entirely. Original native checks validate every selected anchor and
retain prior exact identities; offline tool tests exercise deterministic
selection, strict refusal and verifier-source pins. Custom PEM replacement stays
strict, and identity, expiry, signatures and constraints are unchanged. Normal
verification remains required for integration. A coordinated browser recheck
must establish actual request/resource behavior; these checks alone establish
no newly usable public service. See [trust scope and refresh](../trust/README.md).

The finite About view is a browser-owned native panel over the same live page.
Build text preserves full prerelease SemVer and descriptive source/state/kind;
unknown metadata stays unknown and a declared tag stays publication-unverified.
It is not code signing or release authentication. Offline legal views retain the
exact main LICENSE, scope and third-party notice index. Full referenced third-
party terms remain packaged separately; the viewer does not open each referenced
file. Safe Copy contains only allowlisted build/source/license fields, with no
page, profile, history, receipt or path data. Its App input capture intentionally
does not invoke the native clipboard. Original App and executable acceptance
must establish legal-text/rendering identity, native focus/scroll/copy controls,
return to the same page and preservation of held resource work. Normal build,
PE and package verification remain required; no installer/update control is added.

The responsive slice admits finite em media lengths using the browser's initial
16CSSpx font basis, independently of author root/body fonts and physical zoom.
Recognized terminal ::before/::after stylesheet targets preserve valid atomic
selector lists without matching ordinary elements or painting generated content.
Explicit box-sizing inherit uses the actual parent; initial/unset remain
content-box and the property is not naturally inherited. Flex main-size line
admission retains fractions until cumulative pixel allocation, so two50% items
fit odd widths while genuine padding/minimum overflow still wraps. Wider units,
pseudo targets and generated content remain unsupported in this finite slice.
The unchanged Foundation fixture's rendering stage improves independently of
its remaining Vue prototype blocker; full normal integration remains required.

The finite viewport-media slice evaluates nested matching rules against the
real CSS viewport and explicit desktop mouse-input profile. Original native/App
checks distinguish full viewport width from available line width, preserve
fractional zoom thresholds, verify changed geometry/background pixels and real
link ownership/hit removal, and retain document/source lifetime. Admission
counts inactive descendants, keeps cascade/stylesheet URL ownership and refuses
whole sources without publishing prefixes. Initial font-family selection uses
the same context; nested font faces are not requested, and dynamic/resize font
fetching remains unsupported. Executable offscreen and full normal integration
checks are separate required acceptance, not inferred from model tests. This
does not establish complete page appearance, script-enabled site behavior or
physical desktop presentation.

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
