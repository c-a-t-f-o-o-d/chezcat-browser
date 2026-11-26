# Browser capabilities and limits

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Compatibility](COMPATIBILITY.md) · [Documentation](README.md)

ChezCat implements a growing subset of the web platform. The tables below describe
supported behavior and its limits; none of these broad areas is complete.

The current published prerelease is
[0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4),
from tagged source `122d0b7`. This page also covers later integrated development
source. A capability described here is not necessarily present in that release.
The [release record](RELEASING.md#current-verified-release) identifies the verified
download, and [Compatibility](COMPATIBILITY.md) keeps each application result tied
to the source that was actually measured.

## Browser and platform

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| Native shell | Tabs and windows, tab-pinned inspectors, guarded primary/middle links, and foreground, background and new-window link actions. | Named contexts, script-opened windows, broader accessibility and locked-desktop presentation remain incomplete. |
| Navigation | HTTP(S) and local files, explicit URL/file intent, encoded address searches, Google/Bing/DuckDuckGo/None providers, and native session navigation. | Live results depend on the response and session. A failed navigation remains distinct from the retained page. |
| Profile and downloads | Settings, bookmarks, optional history, staged downloads, guarded local roots and memory-only cookies. | No general site storage, service workers or persistent cookies. Local files are not encrypted vaults. |
| Identity and legal information | About shows descriptive version/source/state/build kind, offline licenses and allowlisted Copy data. License acceptance supports explicit personal/organization choices, same-text reuse and refusal after changed terms, decline or write failure. | Descriptive metadata is not authentication. Local acceptance does not verify identity or organizational authority. An application capture alone does not establish OS clipboard delivery. |
| Build and delivery | Original Rust implementation with no third-party production Cargo dependencies, generated resources, pinned tools and portable ZIP delivery. | Installer, update discovery and authenticated upgrade remain planned. |
| Isolation | Checked ownership, worker retirement/cancellation, explicit foreign-function boundaries and watchdogs. | One process; no OS renderer sandbox or whole-process memory sandbox. |

## DOM and JavaScript

### Documents, identity and traversal

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| HTML documents | Loaded and inert HTML share native structural nodes, omitted html/head/body construction, selected formatting recovery, named character references, script-data scanning and supported raw-text handling. | Tokenizer diagnostics, streaming, complete tree construction and full HTML conformance remain incomplete. Loaded trees retain node and depth bounds. XML/XHTML byte parsing is not implemented. |
| Source decoding | Original Encoding decoders serve HTML/plain text, linked stylesheets and classic scripts. BOMs, recognized transport labels and the supported HTML meta/CSS charset rules select an encoding; documents expose their committed canonical encoding name. | Parser-triggered encoding restarts, confidence state, full MIME/XML routing and environment inheritance remain incomplete. Dynamic local-file stylesheets are refused. |
| Native DOM identity | Supported `importNode` boolean/options conversion finishes before clone construction or prohibited-node errors, preserving dictionary getter order, abrupt values and author effects. Supported nodes, attributes and owner documents retain genuine identity through checked mutation, clone/import and selected adoption paths. Main-document HTML ID, class, name, title and language values preserve exact UTF-16 in their supported producers and consumers. | Raw-string support is not universal across SVG, child documents, active attributes or every native interface. Unsupported owner transitions and resource-sensitive mutations remain guarded. |
| Dataset attributes | Readonly `dataset` accessors on `HTMLElement` and `SVGElement` prototypes retain the same main-document backing object and genuine declaring-interface brands. Canonical window-specific `DOMStringMap` interface roots describe main-document dataset objects; actual supported names override author prototype members. Main-document HTML dataset writes preserve exact UTF-16 values, validate names with native DOM exceptions and deliver attribute mutation records with the actual element owner. Supported deletion uses native attribute removal; live keys, Attr identity and property descriptors reflect the backing attributes. | Other document/realm dataset owners, full SVG, raw property-key and Symbol integration, MathML accessors, and custom-element reactions remain incomplete. |
| Parent mutation | Supported ordinary Element and DocumentFragment trees expose `append`, `prepend` and `replaceChildren` with ordered Node/string conversion, exact UTF-16 Text nodes, duplicate-node moves and native fragment expansion. Replacement publishes combined child-list records and updates live collections and Ranges. Represented inert Documents validate replacement root structure. Failures preserve author effects and earlier committed staging moves. | Cross-owner replacement adoption, displayed or child Document replacement, custom-element reactions and unsupported native resource interiors remain outside this subset. |
| Child mutation | Supported Element, CharacterData and DocumentType nodes expose native `before`, `after` and `replaceWith`. Ordered argument conversion precedes parent lookup; detached calls return before native construction. Viable sibling references survive receiver and neighbor moves, with actual native records and live Range updates. Self-insertion and references moved by fragment expansion retain their specified mutation and error effects. | Native ownership and resource checks remain in force. Complete child-document mutation, custom-element reactions and broader lifecycle integration remain incomplete. |
| State-preserving node moves | Native `moveBefore` moves supported Element and CharacterData nodes within one root, retaining node, editor and eligible blank-frame identities while updating observer records, live views, iterators and Ranges. Same-owner inert trees validate Document constraints. | Shadow/custom-element moving reactions, child/displayed Document moves and broader resource lifecycle support remain incomplete. |
| Processing instructions | Main and supported inert native ProcessingInstructions expose ordered pseudo-attribute reads and writes. The original XML pseudo-attribute parser decodes predefined entities and legal character references; malformed input discards the complete map. Attribute writes escape serialized data and share native CharacterData observation and live Range updates. Direct CharacterData changes and copied nodes parse their actual data independently. | XML stylesheet fetching/application, full XML byte parsing and broader child-document lifecycles remain incomplete. |
| Collections and queries | Live child, tag, class, name and namespace collections; static selector NodeLists; native token-list operations and canonical NodeList iteration/callbacks over the represented owners. Queries use the supported shared CSS selector grammar. | Complete named-property descriptors, all generic collection methods and wider owner/prototype behavior remain incomplete. Query work and result storage are bounded. |
| Templates and inert content | Native template content fragments have separate owners. Supported nested clone/import and later activation preserve node/attribute identity, observers and resource policy. Factory documents and represented XML-shaped DOM trees support selected native operations. | Inert content does not gain fetch or execution authority. Complete cross-document adoption, all element lifecycles and actual XML parsing remain incomplete. |
| Traversal | Main, genuine inert and connected same-origin initial blank child Documents create TreeWalkers and NodeIterators over supported native trees. Factories retain their method realm's canonical interfaces independently of the receiver/root owner. TreeWalker supplies seven movement operations; NodeIterator supplies next/previous navigation with filtering and synchronous reference/candidate adjustment through supported removals, moves, replacements, content changes and normalization. | Broader child-document mutation/navigation/teardown, callable Proxy execution and full traversal qualification remain incomplete. Creation retains filter identity without invoking it; callbacks and mutations keep native authority/resource checks. |
| Range | Live boundaries over supported displayed, genuine inert, native Attr and connected same-origin initial blank child Document/Element nodes; relative endpoints, collapse, clone, exact UTF-16 stringification and boundary/point comparisons. Supported main/inert cloneContents, deleteContents, extractContents, insertNode and surroundContents use native tree/data transactions, actual owners and typed hierarchy errors. Copies preserve selected partial ancestors and independent subtrees/template roots; extraction moves fully selected identities. Supported mutations update live boundaries, observations and iterators while StaticRange retains fixed native snapshots. Attr boundaries have zero length and independent roots; empty fragments use their current owner. Factories and cloneRange preserve canonical method realms independently of receiver documents and copied endpoints, with endpoint authority checked before and after author conversion. | Automatic cross-document inert insertion/surrounding, child Range content producers, broader child navigation/teardown, displayed Document-child content mutation, full native lifecycle/entry-path qualification, Range geometry and DOM Selection remain incomplete. StaticRange does not supply live Range behavior. |
| Observation and events | Supported mutation records retain actual old values and native identity through observer/Promise delivery. Genuine Event subclasses, synthetic dispatch and native input events share checked targets and owner realms. Supported Event type names preserve exact UTF-16 code units through construction, legacy initialization, listener matching/removal and synthetic dispatch. Supported inert-node dispatch follows actual native parents to fragment or Document roots, preserving propagation phases and path snapshots. Connected initial blank child trees dispatch through their own Document/Window path without an embedding-page bridge. Child Window EventTarget methods resolve through their own canonical prototype chain, including ordinary prototype overrides and deletion. AbortSignal.timeout schedules native abort tasks in supported main and initial blank child documents, with owner-realm TimeoutError reasons, listener cleanup and cancellation when the child disconnects or timers stop. AbortSignal.any consumes supported iterables and creates native dependent signals, preserving first reasons and source/dependent abort ordering through linked listener cleanup. Listener signal options bind genuine AbortSignals to actual registrations; abort removes linked listeners before abort callbacks and skips them in active dispatch snapshots. Omitted passive options for touch and wheel event types resolve from actual Window, Document, document-element and body identities; explicit passive values retain their cancellation behavior. | Not every mutation, event interface, propagation case or shadow/frame lifecycle is implemented. Synthetic events do not acquire trusted-input authority. |

### Language and execution

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| Core JavaScript | Implemented object/prototype operations, functions, classes, symbols, arrays, strings, RegExp, JSON, maps/sets and weak collections, typed storage, Reflect and ordinary Proxy behavior. | Syntax and built-in coverage remain finite; modules remain incomplete. Each family has unimplemented operations and resource bounds; this is not an ECMAScript conformance claim. |
| Exact strings and JSON | Supported DOMString and language paths distinguish raw UTF-16 from literal replacement characters. JSON parse/stringify preserves exact keys and string data, with supported reviver/replacer behavior and explicit storage/work admission. | Scalar-only native consumers retain their own limits. A successful string or JSON operation does not extend an unrelated interface's grammar. |
| Promises and async functions | Genuine Promises and jobs support selected async functions/arrows/methods, await expressions and control flow, retaining lexical state, receivers and rejection reasons. Supported generators use a separate suspension path. | Async generators and unimplemented syntax/control-flow forms remain outside the subset. Suspension storage, compilation and execution remain bounded. |
| Async iteration | Selected for-await-of loops in async functions use a lexical binding and an authored Symbol.asyncIterator, awaiting results and early closure through Promise jobs. | Synchronous iterable fallback, async generators and other binding forms remain incomplete. |
| Scheduling and animation | Supported timers and replaceable animation-frame operations use actual callback scheduling/cancellation. Pinned Web Animations fixtures exercise selected style and opacity behavior. | This does not establish every timer behavior, native animation timelines, compositor support or whole-library compatibility. |
| History and location | Native pushState/replaceState share same-document entries with Page/App navigation, URL/base state and Back/Forward. Supported serializable primitives retain exact values. Active main and supported same-origin child documents expose their own Location. | Structured object/array state, popstate, the full History prototype, script go/back/forward, scrollRestoration and general child navigation remain incomplete. |

### Forms, focus and editing

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| Native controls | Supported text/search/tel/password/hidden inputs, checkboxes, radios, select/options, textarea, buttons and output use native state and checked form ownership. Supported clone/import preserves their admitted values and identities. | All HTML input states, full form collections, RadioNodeList, general reassociation and every control lifecycle are not complete. |
| Input and focus | Trusted pointer/keyboard input, editing, queued control focus, committed activeElement readback and native blur/focusout/focus/focusin events preserve editor state and current targets. | Full synchronous/reentrant focusing, arbitrary non-control focus, interactive child frames, shadow retargeting and OS/chrome focus transitions remain incomplete. |
| Validation and reset | Supported required, custom, length and pattern validity, native resets and requestSubmit use actual control state. Output has its noncandidate validation behavior and checked default-value reset. | Full constraint validation, all validation UI, every form IDL operation and child/import lifecycles remain partial. |
| Submission | Supported GET and URL-encoded POST submission use native form data, navigation and security policy. Named controls retain checked native lookup and method-shadowing behavior. | Multipart, text/plain and dialog submission, full submission semantics and inert form submission remain incomplete. |

## Rendering and resources

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| CSS selectors and cascade | Loaded/inline styles, supported attribute selectors and flags, root, is/where groups, inherited custom properties, finite viewport/hover media rules and live supported computed values. | Full CSS tokenization, escapes/namespaces, has/scope, broader media grammar and pseudo-element generated painting remain incomplete. Successful substitution does not extend a property's grammar. |
| Layout | Finite block, inline, flex, table and implicit single-column grid behavior; responsive sizing, supported item order/alignment, box sizing and typed calculations for selected dimensions, margins and padding. | Authored grid tracks/placement, broad table repairs/spans/collapsed borders, RTL/writing modes and complete flex/grid algorithms remain incomplete. |
| Text | Supported family/weight/style, sizes and line heights feed native metrics, glyph paint, wrapping and link geometry. Selected whitespace/alignment values and custom-property/defaulting behavior share the cascade. | Shaping, bidi, justification, variable axes, wider font grammar and full modern whitespace/alignment semantics remain unsupported. |
| CSSOM and geometry | Live supported computed styles, inline declaration enumeration/replacement, actual offset parents/coordinates, standard-root vertical scroll, finite visual rectangles and bounded 2D translations. | Full CSSOM/source editing, computed pseudo-elements, child renderer bindings, nested element scrolling, scrollbar widgets and wider transforms remain incomplete. |
| Painting | Software rendering provides rectangular clipping, selected rounded color backgrounds/borders, color-background clipping, group opacity, and one URL background layer on supported boxes and buttons. | Multiple image layers, broad native-control decoration, general compositing/blending and GPU acceleration remain incomplete. Root/body group opacity retains its separate limitation. |
| Resource lifecycle | Guarded parser/dynamic stylesheets, decoded images, supported responsive image reselection and font selection share owner, request and retirement checks. Failed responsive-image replacement retains the previous displayed image. | CSS imports, general stylesheet CORS, dynamic/resize font loading and cross-origin font fetching remain incomplete. |
| Canvas | Stable 2D contexts provide solid-color fillRect, clearRect, dimension reset, ImageData construction and independent RGBA8 readback. Transparent canvas backing participates in actual layout and paint. | Paths, transformations, broader drawing state, image imports, writing ImageData, encoded export, gradients/patterns and additional color spaces remain open. Backing has per-canvas pixel/axis and shared document limits. |
| SVG | A static image subset and supported genuine inline geometry, cascade and fill/stroke painting. | Complete SVG, filters/patterns, nested image resources, animation and wider text/viewport/transform behavior remain unfinished. Local references do not grant script or fetch authority. |

### Image and font formats

Format names identify supported decoder families, not every profile or encoding
option. The [image workshop](../pages/image-workshop.html) contains original
examples.

| Family | Supported coverage | Important exclusions |
| :--- | :--- | :--- |
| PNG / APNG, GIF, WebP | PNG standard depths/color types, Adam7 and transparency; GIF87a/89a; original VP8/VP8L WebP with alpha. Supported animations retain composition, disposal, timing and repeat data. | Decode/work/storage limits apply. A validated static poster may be an explicit fallback; malformed input remains an error. |
| JPEG | Original Huffman/DCT decoding, supported baseline/progressive images, grayscale/RGB/CMYK/YCCK and bounded Exif orientation. | Arithmetic/lossless/hierarchical JPEG, ICC color management and HDR output. Basic CMYK conversion is not print-profile accuracy. |
| TIFF | First classic image; supported strips/tiles, integer gray/palette/RGB/CMYK, planar/chunky layouts, PackBits/LZW/Deflate and selected CCITT/JPEG paths. | BigTIFF, additional pages, specialized samples/compression and progressive JPEG inside TIFF. |
| BMP, ICO/CUR, TGA, PCX | Original bounded bitmap/icon/legacy raster decoders with supported palette, bitfield and run-length forms. | Each keeps its format subset. CUR hotspots do not redefine native pointers. |
| QOI and Netpbm | QOI RGB/RGBA and supported PBM/PGM/PPM/PAM, including selected alpha and higher-depth conversion. | Internal output remains eight bits per RGBA channel; uncommon tuple/sample forms are refused. |
| SVG | Static image subset and the separate inline geometry/cascade/paint support above. | Full SVG and its broader resource, scripting and animation behavior. |
| TrueType / WOFF1 | Original outline parsing, metrics, grayscale rasterization, WOFF1 reconstruction and finite family/weight/style/Unicode-range selection. | WOFF2/Brotli, CFF, variable fonts, shaping and cross-origin font fetching. Missing glyphs use available fallback faces. |

The shared image entry point admits at most **32 MiB input**, **4,194,304 pixels**
per image and **16,384 pixels per axis**. Animation storage additionally retains
**256-frame** and **16,777,216-pixel** limits. Individual codecs and callers can
apply stricter work, intermediate-storage or request policies. Higher-depth samples
convert to RGBA8; color profiles and linear-light compositing remain unimplemented.
**AVIF, HEIF/HEIC and JPEG XL are not implemented.**

## Networking, trust and diagnostics

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| Transport | Original HTTP/TLS/certificate handling, redirects, decompression, workers and timings; bounded serial dual-stack fallback. | Protocol/parser/chain gaps remain. OS DNS can block; there is no concurrent Happy Eyeballs. |
| Trust | Reviewed conservative 98-authority Mozilla NSS subset, or explicit PEM replacement under the shared 1,024-root capacity. | No OS-store discovery, automatic root updates, revocation/CT, full Firefox policy or security certification. |
| Request policy | Original hostname for TLS/Host/origin, with per-hop CSP, cookie, credential, mixed-content, MIME and response-policy checks. | A successful response does not establish a usable website or clearance through a service challenge. |
| XMLHttpRequest | Main-document asynchronous HTTP(S), admitted text/binary request bodies, filtered headers, text responses, abort/timeout and generation/navigation cancellation. Completion uses a complete bounded response buffer. | CORS preflight, synchronous/child requests, Blob/FormData/Document bodies, non-text responses and XML documents remain unsupported. Existing TLS, cookies, CSP/CORS and credentials still govern transport. |
| Headers and Response | Native header values and buffered response/body consumption through intrinsic Promises for text, JSON, ArrayBuffer and Uint8Array. | These value interfaces supply no network Fetch/Request API or readable stream. Blob/FormData consumption, static redirect and wider request/response behavior remain unfinished. |
| Diagnostics | Document-only diagnose mode, optional exact-host restrictions before DNS, phase/timing evidence and escaped bounded metadata. | Diagnostic results retain their actual document, source and session scope; they do not establish complete site interaction. |

<a id="remaining-capacity-correction"></a>

### Resource limits

Checked bytes, retained storage, work, deadlines, cancellation, ownership and
native-stack headroom govern supported operations. A capacity refusal is distinct
from a format or feature being unsupported. Mutations retain their documented
atomicity and recovery behavior; author callbacks may have effects before a later
native refusal.

Limits differ by consumer. For example, loaded HTML has a 262,144-node and
128-open-element bound; local HTML keeps a 32 MiB encoded/decoded allowance, while
network document text has a 2 MiB decoded allowance. Canvas uses up to 4,194,304
pixels per backing, a 16,384-pixel side limit and a shared 64 MiB document allowance.
These are individual admission policies, not a whole-process memory guarantee.
The removal of a record-count ceiling does not remove byte, work, format or
security checks.

## Media

| Area | Supported behavior | Limits |
| :--- | :--- | :--- |
| Formats | Local IVF/VP8 visible fixed-canvas version-zero single-motion-vector I/P frames, PCM and constrained Matroska audio/video. | SPLITMV, hidden/nonzero-version interframes, WebM/Opus/Vorbis and general codec support remain absent. |
| Authored playback | One supported player, native controls, source/load lifecycle, script play/pause/seek and promises tied to actual decoded output. | Multiple players, MSE, streaming/ranges, muted Matroska and complete HTMLMediaElement behavior remain incomplete. |
| Network playback | Complete bounded HTTP(S) IVF/Matroska responses selected by MIME and explicit authored CORS/credential modes. | Unmarked foreign playback, general Fetch/CORS and ordinary YouTube/service playback are not established. |
| Native interfaces | Genuine owner-realm media/video roots, constants, supported accessors and controls reflection. Hiding/restoring controls preserves the source, clock, frames, seek and play promises. | Video dimensions, dynamic crossOrigin, extra/child players and wider APIs remain incomplete. Offscreen checks do not establish physical input or audible output. |

## Evidence boundaries

Model checks establish their conversion, ownership, queue and lifetime cases.
Application journeys add actual input, state, layout, hit targets and decoded
pixels. Pinned libraries establish only their measured version and workload;
reference captures establish only the stages actually compared. Release checks
establish the named source and delivered bytes. Physical devices and live services
require their own observations.

The [compatibility record](COMPATIBILITY.md) separates these scopes and retains
measured revisions. [Feature contracts](FEATURE_CONTRACTS.md) summarizes lasting
behavior and security commitments. The [roadmap](ROADMAP.md) describes the broader
capability sequence.
