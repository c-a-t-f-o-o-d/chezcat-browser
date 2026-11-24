<div align="center">

# 🐱 HTML and XHTML Foundation Plan

*One standards-based document foundation, with explicit behavior and evidence.*

[Roadmap](ROADMAP.md) · [Coverage register](HTML_FOUNDATION_COVERAGE.json) · [Capabilities](CAPABILITIES.md) · [Compatibility](COMPATIBILITY.md)

</div>

---

## Decision and objective

The HTML/XHTML document foundation precedes the saved YouTube shell, search,
navigation and playback milestones. Required parser and DOM behavior is selected
from the [coverage register](HTML_FOUNDATION_COVERAGE.json).

F00 met its requirements, input-lock and baseline criteria and was delivered in [v0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4). The original
source-bound native results retain their failures and harness gaps. F01 now has
shared native document, node, namespace and UTF-16 operations; F02 has shared
decoders and initial document charset selection; F04 has a partial raw-text
producer. Required integrations and unchanged browser assertions remain open.
Named release verification is tracked in the [release history](RELEASING.md#published-baseline).

The initial review was anchored to
[`250705b`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/250705b6c46ad472f7dc8fd6a1442503a85e7dfd)
through PR #438, with PR #439 adding Event author-prototype behavior. The
historical review below and the current package register serve different purposes.

### What completion means

The target is a **complete, explicitly measured document foundation**:

- Standards-based HTML document and fragment parsing, including malformed input,
  encoding, modes, foreign content, templates and serialization.
- Genuine XML 1.0 and Namespaces in XML parsing, XML DOM operations and
  application/xhtml+xml document behavior.
- Shared DOM/Web IDL, element, form, event, editing, lifecycle, navigation and
  native component contracts needed to make those documents behave correctly.
- Actual integration with the existing renderer, resource loader, native controls
  and JavaScript engine, including default HTML presentation and mode consumers.
- A fixed, reviewed requirement register and reproducible tests with all failures,
  blocked cases and remaining dependencies visible.

This is not a claim of complete CSS, ECMAScript, graphics, font, media, networking
or whole-browser conformance. HTML itself also specifies workers, storage and
other application APIs. The chapter disposition below keeps those requirements
visible. An external dependency needed by an included contract is a **blocker**
until implemented; it is not a reason to report that contract complete.

The final review must distinguish **document foundation complete**, **HTML
Standard coverage still open**, and **YouTube journey still unverified**. Changes
to the agreed scope require an explicit owner decision. Required failed or
blocked contracts remain visible in the acceptance register.

## Standards and design decisions

### One HTML algorithm, a separate XML frontend

Use one modern HTML tokenizer/tree builder for loaded documents, fragment
operations, inert DOMParser documents and template parsing. Entry points supply
their real context, scripting flag, owner, document mode and activation policy.
Do not maintain different parsers for HTML 2, HTML 3.2, HTML 4 or XHTML doctypes.

The HTML doctype algorithm selects **no-quirks**, **limited-quirks** or **quirks**.
Retain that distinction internally. Both no-quirks and limited-quirks expose
CSS1Compat; quirks exposes BackCompat. The mode has real parsing, DOM and rendering
consumers, not just a getter. For example, the in-body table rule treats an open p
differently in quirks mode. Fragment parsing inherits the context document's mode.
Document factories, srcdoc and XML have their specified mode rules.

Select HTML versus XML through the computed MIME type and the API contract.
A document with an XHTML doctype served as text/html still uses HTML parsing.
Application/xhtml+xml and the supported XML MIME types use strict XML parsing.
Local files require deliberate type classification; never quietly route an
XHTML file through the permissive HTML parser. XML well-formedness failures must
produce the required failure/error document, not HTML-style recovery.

Both frontends feed the same genuine DOM model. Preserve arbitrary namespace
URIs, prefix/local-name distinctions, document type and owner. Native HTML, SVG
and MathML interfaces are selected by namespace/local name, not by tag spelling
alone. No SGML parser is planned.

### Correct the assumptions in the initial proposal

1. HTML defines much more than tags, and builds on DOM, Infra, Web IDL, Encoding,
   URL, MIME Sniffing and other standards. Its index is useful inventory, not a
   substitute for each normative algorithm.
2. HTML owns encoding selection, prescan, confidence and possible restart;
   Encoding owns labels, indexes, decoder states and error handling. The 1,024-byte
   metadata authoring/prescan rule is not permission to ignore every later parser
   encoding declaration.
3. Quirks are distributed across HTML, DOM, CSSOM and the Quirks Mode Standard.
   Implementing the doctype lookup without its consumers is incomplete.
4. The current html5lib-tests repository directs tree-construction testing to
   WPT. Its tokenizer, encoding and serializer data remain useful.
5. The reviewed current HTML revision has **84 tokenizer states and 21 insertion
   modes**. It includes processing-instruction states, and current select handling
   belongs in the in-body rules. Do not implement an older 80-state/23-mode list.
   These are inventory counts at the pinned revision, not completion percentages.
6. WPT is an executable interoperability corpus, not a mathematically complete
   list of every normative requirement. A requirement register and original
   integration/negative tests remain necessary.
7. Default HTML rendering and CSS algorithms are different responsibilities.
   Passing DOM tests does not prove layout, fonts, selection or hit targets match
   another browser. Exact browser pixels also depend on fonts, OS and allowed UA
   presentation choices.

### Normative authority and reproducible inputs

Living standards remain the normative authority. The following reviewed commits
are reproducibility anchors for the initial work. They do not turn an old snapshot
into a new independent standard. Review later normative changes explicitly,
record the delta and update the cohort before claiming new results.

| Input | Reviewed revision or edition | Use |
| :--- | :--- | :--- |
| [HTML](https://html.spec.whatwg.org/) | 0e292f402d0c3dd17fe00c1bb2ef3a3c94cd8bce | Parsing, elements, document lifecycle and HTML APIs |
| [DOM](https://dom.spec.whatwg.org/) | e4f4f4f0ace83fe6c52ab68e047351ce7f2dcd69 | Nodes, mutation, events, ranges, traversal and shadow trees |
| [Web IDL](https://webidl.spec.whatwg.org/) | 54a5f2de6a34e27a517a49370bc07d8943b0be11 | Bindings, conversions, descriptors, receivers and realms |
| [Encoding](https://encoding.spec.whatwg.org/) | 7d2e3e08e11f57ba1b080390b490d53648c6f69c | Labels, decoders, indexes, byte/character conversion |
| [URL](https://url.spec.whatwg.org/) | 4f461b81a748443b235840599cea6290df40329a | URL records, resolution, serialization and form encoding |
| [MIME Sniffing](https://mimesniff.spec.whatwg.org/) | ba4b3c385b9765ddfe4a994e2bb89e0ab96068e2 | Type parsing and context-specific document/resource routing |
| [Quirks Mode](https://quirks.spec.whatwg.org/) | 5a06deb58ae2644d62c3c2026bbef86e6137b779 | Required legacy presentation behavior |
| [XML 1.0](https://www.w3.org/TR/2008/REC-xml-20081126/) | Fifth Edition, with published errata reviewed | Nonvalidating XML processor and well-formedness |
| [Namespaces in XML](https://www.w3.org/TR/2009/REC-xml-names-20091208/) | 1.0 Third Edition, with published errata reviewed | Expanded names, bindings and namespace constraints |
| [WPT](https://github.com/web-platform-tests/wpt/tree/42e27408034f57afe4c726476d1a0bd96b873129) | 42e27408034f57afe4c726476d1a0bd96b873129 | Tree data, DOM/HTML/XML/API, URL and integration fixtures |
| [html5lib-tests](https://github.com/html5lib/html5lib-tests/tree/c5495c539ca8a79c3c5107f5f14010bcff9a35d5) | c5495c539ca8a79c3c5107f5f14010bcff9a35d5 | Tokenizer data and supplementary encoding/serializer data |
| [HTML-build entity data](https://github.com/whatwg/html-build/blob/283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70/entities/out/entities.json) | 283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70; blob 557170b41f47a13a46ec695561eb5fe76da73bdb | Official named-character-reference input |

F00 locks the exact files, SHA-256 digests, selectors, licenses and transitive
test inputs before execution. It also records precise current revisions for
[Infra](https://infra.spec.whatwg.org/), the
[XML serialization algorithm](https://w3c.github.io/DOM-Parsing/),
[UI Events](https://w3c.github.io/uievents/),
[Selection API](https://w3c.github.io/selection-api/),
[Input Events](https://www.w3.org/TR/input-events-2/) and
[Pointer Events](https://www.w3.org/TR/pointerevents3/), and the required Fetch,
CSSOM, CSSOM View, Unicode and security dependencies when their packages are
selected. A link without a pinned input digest is not a completed test manifest.

The official entity data contains 2,231 names, including 93 multi-code-point
mappings and 106 semicolonless legacy names. Generate an original Rust lookup
from pinned data, preserving full strings, spellings and context rules. Preserve
the upstream data and applicable attribution; do not type the table by hand or
describe imported standards data as wholly original authorship. Apply the same
procedure to encoding indexes, IDNA and Unicode properties.

Production remains original Rust/std-only. Specifications, reviewed data and
test fixtures are permitted inputs with their own notices. This does not authorize
copying or translating html5ever, html5lib, libxml, Chromium, Gecko, WebKit or other
engine implementations. Development corpora/reference browsers are not runtime
dependencies. Keep corpus material out of the runtime allowlist and retain its
licenses in any source archive that includes it.

## Source baseline: substantial reusable work, foundational gaps

| Area | Reviewed state through PR #438 | Consequence for this plan |
| :--- | :--- | :--- |
| Loaded HTML parsing | Generic stack/subset parsing, selected implied closes and raw-text cases; incomplete implicit roots, table repair, formatting reconstruction and entities | Implement the standard algorithms and migrate real entry paths; do not add another tag-specific recovery patch |
| Mode and decoding | Loaded doctypes discarded; displayed compatMode is effectively fixed; document decoding largely UTF-8/windows-1252 | Add real mode state/consumers and full Encoding/HTML selection behavior |
| URL processing | Separate bounded HTTP(S) parsing/API models; IDNA and non-HTTP schemes are restricted | Share standard URL records while keeping permission to fetch/navigation separate |
| DOM and components | Genuine handles, attributes, collections, observers, ranges, nested templates, selected author properties and native focus exist | Preserve those semantics; audit full operation families, ownership and reentrancy |
| Clone/import | PR #437 closes inert subtree clone reservation; PR #438 closes main-template import identity reservation and late-failure cleanup | Reuse and regression-test these increments; do not schedule them again as absent |
| Forms/editing | Selected native controls and submission work; many input states, multiple selection and editing behavior remain unsupported | Cover the entire form family and actual editing/activation behavior |
| XHTML/XML | Network XML/XHTML refused, local classification not a genuine XML path, DOMParser XML modes unsupported | Build a real XML frontend, XML DOM and namespace/serialization support |
| Native components | Selected authored noPatch polyfill path has made progress; native Custom Elements and Shadow DOM remain incomplete | Include native component semantics in the document foundation, independent of whether YouTube's current cohort needs them |
| Evidence | Source reviews and original App cases exist; no complete pinned HTML/DOM/XML conformance denominator is established | Seed the register honestly, then measure. No overall completion percentage is justified |

Representative retained parser witnesses include omitted html/head/body,
head-to-body transition, table text foster parenting and implied tbody, formatting
reconstruction across p, iframe raw text, and multi-code-point character references.
These witnesses preserve the original baseline. The loaded iframe case now has
a native raw-text producer. The coverage register tracks the remaining parser
and integration requirements.

## Coverage boundaries: every HTML chapter has a disposition

**Included** means required for foundation closure. **Related program** means
tracked wider HTML/platform work, not a passed or forgotten requirement.
**Dependency** means its necessary contracts must work for the included feature;
full conformance of the external specification is a separate program.

| HTML area | Foundation disposition | Owner package / wider work |
| :--- | :--- | :--- |
| Introduction, conformance and dependencies | Required scope/term/reference interpretation; distinguish browser behavior from authoring rules | F00; do not reject malformed HTML because an author content model disallows it |
| Common infrastructure | Microsyntax, URL/base, reflection, collections, resource attributes, nonce and policy integration included | F01–F03, F06, F09–F10; general transfer/structured-clone cases tracked with communication/storage, while history-required state is included |
| Document semantics/APIs | Document metadata, ready state, accessors, HTML constructors, global attributes and element interfaces included | F01, F06, F09–F10 |
| Elements | Every conforming element and applicable required obsolete behavior inventoried; parsing/DOM/activation/fallback obligations included | F04–F05, F09–F13; full canvas graphics, media decoding, SVG/MathML rendering remain explicit related programs |
| Microdata | HTML element attributes, token lists, item ownership/value and standard DOM behavior included | F09; no unrelated search-engine or metadata service |
| User interaction | Focus, activation, hidden/inert, editing, selection, keyboard/access keys, dialog/popover, drag data and event contracts included | F10–F11; OS clipboard/drag permission and accessibility mappings are explicit integration dependencies |
| Loading pages | HTML/XML MIME dispatch, ordinary navigation/history, document lifecycle, browsing contexts, iframe/srcdoc and origin/sandbox policy included | F02–F03, F10; speculative loading/prerender/BFCache optimizations remain related programs and must not be falsely advertised |
| Web application APIs | Parser/script integration, timers, microtasks, errors, markup insertion, serialization, sanitizer/streaming entry contracts included | F06–F10, F12; full ECMAScript is a dependency program; general scheduler/worklet/offscreen APIs remain explicit wider work |
| Communication | Document-required load/message/security boundaries accounted for; full channels, transferable/structured data and messaging conformance tracked | Related program COMM; any required iframe/document operation remains blocked until its real dependency exists |
| Workers | Worker environment/loading/lifecycle not part of the document baseline | Related program WORKERS, with truthful feature detection |
| Worklets | Not part of the document baseline | Related program WORKLETS |
| Web storage | Web Storage remains unimplemented; session history is a separate existing feature, not sessionStorage | Related program STORAGE; persistence/events/quota policy remains open, with no empty-success fallback |
| HTML syntax | Complete applicable tokenizer, construction, fragments, serialization and error recovery | F04–F07 |
| XML syntax | XML 1.0 nonvalidating processor, Namespaces, XML DOM, XHTML and XML fragment/serialization integration | F08 |
| Rendering | Default HTML presentation, presentational hints, all mode-dependent consumers, controls and hit-target integration included | F13; required CSS/text dependencies are blockers; full CSS/typography parity remains a separate program |
| Obsolete features | Required parsing/interface/presentation compatibility included, authoring advice kept separate | F04–F05, F09–F13; no SGML engine or external plugin runtime |
| IANA/MIME registrations | Correct document/script/resource type handling included | F02, F10 |

HTML parsing of SVG/MathML integration points is included even when a particular
graphic cannot yet be painted. Media elements must reflect actual readiness,
errors, resources and controls; unsupported codecs must not produce fabricated
playback. Canvas must retain correct element/fallback/dimensions and real supported
context behavior; complete 2D/WebGL/WebGPU work remains separately named. These
boundaries prohibit a blanket claim of "all HTML implemented."

## Dependency-ordered execution packages

Each package may require several small coherent PRs. The package name is not a
requirement to deliver an unsafe large rewrite in one PR. Work known contracts
in dependency order, updating the register with exact source and evidence. The
default **start order** is F00 → F01 → F02 → F03 → F04 → F05 → F06 → F07 → F08 →
F09 → F10 → F11 → F12 → F13 → F14. This is not a requirement to close every
integration gate before beginning its producer package. Split a package into
core implementation and named integration contracts where needed: F01's serializers
come from F07/F08; F02's restart integration uses F05/F10; F06 observer delivery
uses F10 scheduling; F07 shadow parsing uses F12; F08 actual controls and lifecycle
use F09–F13. Mark these contracts partial or blocked, implement the known
prerequisite, then verify the integration before closure. A passing core does not
complete a package whose required integrations remain open.

XML grammar/namespace work can start after F01/F02 once the shared node model is
ready; complete API/App acceptance still uses F06/F07 and later consumers.

### F00 — Freeze requirements and make the baseline executable

**Deliverables**

1. Establish the current main's source-bound baseline, including already
   integrated behavior and remaining requirements.
2. Freeze the supplied complete family inventory and expand the first selected
   package into requirement-level rows: normative anchor/revision, affected entry
   paths, current code, expected behavior, dependencies, fixture IDs/modes, current
   status and closing evidence. Expand later families immediately before their
   implementation; do not postpone coding for an open-ended browser-wide audit.
3. Pin the listed corpora and data with licenses, hashes and stable case IDs.
   Inventory the relevant pinned corpus independently of which tests pass.
   Freeze required versus supplementary evidence before results. Each required
   browser test records its harness dependency; availability cannot later turn
   it into a supplementary case.
4. Add a small original native adapter for tokenizer and tree-construction data,
   reusing existing confined results, hashing and process handling. Do not wait
   for Fetch, Blob, document.write or WebDriver to exist.
5. Run one initial fixed parser cohort plus the retained source witnesses.
   Unimplemented entry modes and diagnostics remain explicitly unrun/blocked.
   Start implementation after that baseline; do not turn F00 into repeated research.

**Gate:** the adapter distinguishes deliberate wrong output, malformed fixtures,
missing inputs, timeout/crash and a genuine product failure. Baseline output is
source-bound, input-bound and complete for its declared selection. No parser
correctness or WPT pass is claimed merely because the adapter runs.

**First implementation after F00:** F01 document/mode/namespace representation,
followed by the known encoding/tokenizer/tree deficiencies. No new YouTube run.

### F01 — Shared document, node and namespace representation

**Deliverables**

- Separate HTML/XML document kind, content type, URL/base/origin, encoding,
  scripting state, ready state, doctype and three document modes.
- Store arbitrary namespace URI plus prefix/local name for elements and
  attributes. Preserve HTML ASCII folding only where specified; XML names remain
  case-sensitive. Keep namespace declarations and null-namespace attributes real.
- Audit native DOMString/USVString bridges and storage. The VM already preserves
  unpaired UTF-16 through StringUnits; reuse that work. DOMString code units and
  XML legal characters are distinct contracts. Do not scalar-normalize input
  before the specified algorithm handles it.
- Supply real Document, XMLDocument, DocumentType, Text, Comment, CDATASection,
  ProcessingInstruction, DocumentFragment and Attr behavior/factories.
- Retain stable node/Attr/collection identity, owner-document links, generation
  checks, document order and separate template-content ownership.
- Define shared checked insertion/removal/adoption primitives and a parser sink.
  Preserve parser provenance and specified insertion, attribute, children-parsed,
  observer and Custom Element upgrade/reaction hooks. Parser flags do not
  authorize extra effects or suppress required shared semantics.
- Preserve actual allocation/identity admission and rollback. Use iterative
  storage for input-controlled depth instead of a hidden native-stack dependency.

**Existing anchors:** html.rs, html_svg_namespace.rs, host_objects.rs,
script_inert_documents.rs, script_clone_node.rs, script_template_imports.rs and
the clone/import reservation modules.

**Gate:** equivalent native records across parser, factory, clone/import and
serialization paths; null/arbitrary namespace/prefix cases; template owners;
doctype identity; stale/cross-owner/late-allocation failure and recovery checks.
Do not conflate legitimate whole-document replacement with preserving aliases
within an existing document mutation.

Exact ID-selector consumer increment: direct selectors and cached stylesheet
candidate filtering read authoritative native ID DOMString values. Raw records
bypass candidate hashing and use metered exact matching, preserving lone-surrogate
versus replacement-character identity without tokenizing IDs. Original native
cases include scalar/pair transitions, cascade, refusal and recovery; public inert
producers and App pointer-driven paint retain actual node/editor/link/page identity.
Main-document raw ID producers, quirks-mode folding, XML ID typing and frozen
qualification remain separate. F01 stays partial.

Exact collection named-lookup increment: HTMLCollection.namedItem retains converted
UTF-16 keys across current native children/tag/image views. Prepaid native ID/name
comparisons preserve actual node identity and skip scalar projection aliases.
Native work/copy refusal, conversion ordering, origin recovery and App interaction
have original checks. Raw named-property keys, full named descriptors, main raw-ID
producers and frozen qualification remain separate; F01 stays partial.

Exact visual-ID consumer increment: native anchors read authoritative scalar IDs,
omitting raw IDs rather than publishing replacement-character aliases. Table
anchor spans and fixed state follow actual admission. Label association compares
borrowed exact native ID/for records while preserving first tree-order matches.
Component and App pointer/navigation fixtures qualify raw records; production
probes cover scalar controls. Main raw-ID/for producers, complete fragment/label
behavior and frozen qualification remain separate; F01 stays partial.

### F02 — Bytes, encodings and MIME routing

**Deliverables**

- Implement all required Encoding decoder families and labels: UTF-8, UTF-16,
  single-byte encodings, GBK/gb18030, Big5, EUC-JP, ISO-2022-JP, Shift_JIS,
  EUC-KR, replacement and x-user-defined, with the applicable encoder operations.
- Implement streaming state, split multibyte sequences, EOF handling, BOM rules,
  replacement/fatal modes and invalid-sequence handling that cannot mask ASCII
  syntax delimiters.
- Implement HTML transport/BOM/meta precedence, prescan, confidence, parser
  encoding changes and restart semantics. No resource activation or duplicate
  script effects from discarded tentative parses.
- Share decoding with linked text resources where their standards require it;
  keep differing HTML, script, stylesheet, XML and API rules explicit.
- Implement MIME parsing/sniffing and HTML/XML/plain text/resource classification
  for network, local and DOMParser entry paths. Preserve nosniff and security
  policies; XML's byte/declaration rules are completed in F08.

**Existing anchors:** loader.rs, text_encoding.rs and document/resource loading.

**Gate:** BOM/header/meta conflict matrix, legacy-language inputs, malformed and
chunk-split bytes, late declaration/restart, local versus network classification,
wrong MIME and no unauthorized side effects. Record all used normative data.

### F03 — URL records, origins and document bases

**Deliverables**

- Consolidate standard URL parsing/resolution/serialization rather than leave
  competing HTTP-only DOM and network interpretations.
- Implement special/opaque URLs, relative references, file/Windows paths,
  username/password records, IPv4/IPv6, IDNA/UTS #46, percent-encoding and
  URLSearchParams/form-urlencoded conversion.
- Connect URL setters, anchors/areas, base element ordering and mutation,
  fallback/about/srcdoc bases, Node.baseURI, document URL and history.
- Keep URL parseability distinct from authority to navigate or fetch. A URL
  record containing credentials, file paths or a new scheme does not bypass
  resource policy, cross-origin restrictions, TLS or credential rules.

**Existing anchors:** url.rs, url_api.rs, script_anchor_url.rs,
script_node_base_uri.rs, loader.rs.

**Gate:** pinned URL JSON/native cases and separately identified JS-only/setter
cases; parse/serialize stability; base changes and owned relative resources;
same-origin tuples/opaque origins and denied scheme/credential crossings.

### F04 — Complete HTML tokenizer and character references

**Deliverables**

- Implement every tokenizer state in the machine-readable register, with
  reconsume, return state, temporary buffer, current token/attribute, EOF and
  input preprocessing semantics.
- Cover data/RCDATA/RAWTEXT/script escaped and double-escaped text/PLAINTEXT;
  tag and attribute variants; comments/bogus comments; all doctype states;
  foreign CDATA; processing instructions in the reviewed current standard;
  named and numeric references.
- Preserve duplicate-attribute evidence required by security-sensitive consumers
  while producing the standard DOM attribute result.
- Handle nulls, CRLF, noncharacters, surrogate/code-unit boundaries, overflow,
  invalid references, missing semicolons and longest matches correctly.
- Generate the full entity table with multi-code-point values. Attribute-context
  ambiguity and legacy names are algorithmic behavior, not a lookup-only task.
- Support incremental input without treating a chunk boundary as EOF. Produce
  structured parse diagnostics independently of resource refusal.

**Gate:** every initial-state variant in the fixed tokenizer cohort, exact token
and error assertions, original chunk-boundary/continuation and cancellation tests.
Unknown diagnostic positions may be tracked separately but block a claim of full
tokenizer-fixture fidelity. Deep input cannot overflow the Rust call stack.

### F05 — Complete HTML tree construction and document modes

**Deliverables**

- Implement all 21 reviewed insertion modes, plus the foreign-content rules,
  reset-insertion-mode algorithm and current select handling.
- Maintain open elements, active formatting entries/markers, form/head pointers,
  template insertion modes, original insertion mode, frameset-ok and pending
  table-character state.
- Implement implicit html/head/body, implied end tags and scope checks,
  formatting reconstruction/adoption agency, foster parenting and all table
  contexts, nested templates and template EOF, ruby and required obsolete rules.
- Implement MathML/SVG integration points, name/attribute adjustments, HTML
  breakouts, foreign self-closing behavior and namespace rules.
- Produce doctype nodes and mode selection from the actual token/public/system
  identifiers. Test every trigger category, missing doctype, srcdoc and relevant
  XML-created HTML contexts.
- Establish suspend/resume/parser-insertion-point hooks for real script
  integration. F10 completes execution and lifecycle; no temporary "all scripts
  after parsing" behavior is counted as parser/script conformance.

**Gate:** fixed WPT tree data in document/fragment and scripting flag modes,
canonical tree/namespace/template/doctype comparisons and retained malformed
witnesses. Parser error recovery must preserve the correct DOM; resource refusal
or silent input truncation is not an acceptable recovery result.

### F06 — Complete shared DOM and Web IDL contracts

**Deliverables**

- Implement real insertion/removal/replacement/adoption/clone/import, live
  collections, attributes/namespaces/token lists/dataset, text normalization and
  document/element queries across the valid document types.
- Complete native interfaces, constructors/prototypes, property descriptors,
  named/indexed property precedence, reflection conversions and errors using
  Web IDL. Preserve concrete brands and realm/receiver rules.
- Inventory the full DOM interfaces/members, including NamedNodeMap,
  CharacterData, Text, AbortController/AbortSignal and the current AbstractRange,
  NodeRange, Range and StaticRange interfaces. Implement actual range extract,
  clone, delete, insert and surround operations and code-unit boundaries.
- Complete mutation observer records and microtask delivery, NodeIterator and
  TreeWalker filters/cursors, live range updates and boundary checks.
- Complete base EventTarget dispatch: propagation, cancellation, passive/once/
  signal listeners, removal/reentrancy, exception reporting and real native
  event brands. F10–F12 extend activation, UI and shadow behavior.
- Handle detached/inert/connected transitions and ordinary owner changes with
  the same shared operations. Real resource/form activation remains separate.
- Reuse landed reservation and author-property work; complete missing families
  rather than add interface-name placeholders.

**Gate:** native DOM tests and the predeclared required unchanged DOM/Web IDL
WPT selection. Required unavailable harness functionality remains an owned blocker.
Also cover connected App identity, observer, edited-control and resource invariants;
invalid receiver, stale generation, hierarchy, late failure and recovery cases.
Native data checks do not imply unchanged browser WPT has passed.

### F07 — Contextual fragments, markup APIs and serialization

**Deliverables**

- Route innerHTML, outerHTML, insertAdjacentHTML, Range.createContextualFragment,
  template content and relevant shadow parsing through the shared contextual
  fragment algorithm with actual namespace/mode/form/template state.
- DOMParser text/html uses the shared **full-document** HTML parser with scripting
  disabled. Document.parseHTML and parseHTMLUnsafe use their specified document
  entry algorithms. Sharing tokenizer/tree-builder machinery does not make these
  fragment APIs or erase differences in scripting/sanitization/activation.
- Implement all table/select/raw-text/template/foreign fragment contexts.
  Preserve inert script semantics and the specified activation of ordinary nodes.
- Implement HTML serialization with proper void/raw-text/escaping/attribute/
  namespace/template rules. Do not serialize XML through HTML shortcuts.
- Cover current parse/setHTML safe and unsafe entry contracts, sanitization,
  Trusted Types/CSP integration and streaming fragment APIs. Core parsing can
  land before their stream/security dependencies; those rows remain blocked
  until the dependencies are real. Unsupported globals cannot masquerade as a
  complete implementation.
- Contain prepublication admission/validation failure without publishing a
  partial replacement. Follow the specified order of conversions, Trusted Types
  callbacks, constructors, reactions and reentrant mutations. Do not undo already
  observable author effects or completed normative mutations if a later step
  throws. Streaming and document.write preserve specified partial/aborted state.

**Gate:** all context variants and serialization fixtures, round trips where
specified, inert versus activated resource cases, security and allocation
rollback. Run a loaded App with native controls inside the mutated subtree,
not only neighboring unaffected controls. F12 closes declarative shadow and
component-specific parsing integration.

### F08 — XML 1.0, Namespaces and XHTML

**Deliverables**

- Implement a conforming nonvalidating XML 1.0 processor: UTF-8/UTF-16 and
  declaration/transport rules, names/characters, case, one document element,
  quoted attributes, end-tag matching, comments, PI, CDATA, references and
  required fatal well-formedness handling.
- Implement the internal DTD/entity/attribute declaration processing required
  for that processor class, normalization and recursive expansion controls.
  Nonvalidating does not mean "ignore the internal subset."
- Implement Namespaces in XML 1.0: prefix scope/rebinding, default namespaces,
  attributes outside the default namespace, reserved xml/xmlns rules, expanded
  name uniqueness and namespace errors.
- Complete XMLDocument, createDocument/createDocumentType, namespace-aware
  factories/queries/attributes, CDATA and processing instructions, XML DOMParser
  MIME variants and XMLSerializer using the current referenced algorithm.
- Integrate real application/xhtml+xml network/local documents, XHTML element
  behaviors, template contents, policy-controlled XML stylesheet processing
  instructions, script lifecycle and XML fragment APIs. document.write must not
  acquire HTML behavior in an XML document.
- Follow HTML's recognized XHTML public-identifier/entity compatibility
  requirements. Do not fetch arbitrary external DTDs/entities, local files or
  network resources as a consequence of XML parsing.
- Record interoperability choices where HTML's XML byte-to-DOM integration is
  underspecified. XML 1.1 and validating-DTD processing are separately named
  unsupported capabilities, not silently interpreted as XML 1.0.

**Gate:** valid/invalid namespace and well-formedness matrices, XML API/WPT
fixtures, XML serialization prefix fixup and round trips, entity-expansion/
external-resource denial and recovery, plus equivalent authored HTML and XHTML
App pages with actual controls and styling. Assert fatal error behavior and
side-effect containment without requiring one browser's incidental error-page
wording.

### F09 — HTML document and element behavior

**Deliverables**

- Inventory every conforming element, global attribute, HTML interface,
  interface mixin, event handler and required obsolete behavior from the pinned
  indexes plus normative sections. An entry needs parse/DOM/reflection/activation/
  default-presentation disposition, not a checkbox for recognizing its name.
- Complete metadata and base/title/link/style/meta effects, document accessors,
  microsyntaxes, reflected attributes, enumeration defaults, booleans, numeric
  constraints, named access and HTMLCollection behavior.
- Complete sections/headings/grouping/text/ruby/bidi/edit annotations, list and
  table models/DOM APIs, microdata, links/image maps and semantic global
  attributes including hidden, inert, language, direction and data.
- Include current additions such as search, selectedcontent, commands and other
  index entries; do not freeze a legacy hand-written tag list.
- Preserve required obsolete parsing and native interface/default behavior
  without reintroducing obsolete plugin runtimes.
- Treat accessibility names/relationships and form association as semantic
  contracts; ARIA string reflection alone is not an accessibility tree.

**Gate:** element-family requirements each mapped to real native records and
behavior; original default-document fixtures plus selected element/reflection/
table/list WPT. Resource, form, component and presentation subcontracts close in
F10–F13 rather than being marked complete by this inventory.

### F10 — Parser, resources, scripts and document lifecycle

**Deliverables**

- Implement parser-blocking classic scripts, style blocking, async/defer,
  dynamic scripts, parser insertion points, document.open/write/writeln/close
  and load/readiness ordering. Preserve currentScript, script errors and
  parser-created versus author-created distinctions.
- Implement module/script-type/import-map host loading and scheduling contracts
  required by HTML, with real module graph/evaluation dependencies. A missing
  ECMAScript feature is a named blocker; do not report a successful empty module.
- Connect tasks/microtasks, promises, observers, rendering opportunities, RAF,
  setTimeout/setInterval, cancellation and cleanup to actual document lifetime.
- Complete ordinary document navigation, fragment navigation, session history,
  state serialization, traversal/events, page lifecycle, unloading/cancellation
  and document replacement, with real same-/cross-origin rules.
- Complete iframe/srcdoc/initial about:blank ownership, Window/WindowProxy
  boundaries, target lookup, sandbox/referrer/CSP/permissions integration and
  frame load/error behavior.
- Complete image/picture/source and responsive selection, load/error/decode
  state; style/link/script resource activation; object/embed fallback;
  audio/video/source/track element and supported resource state. Never claim
  a request or successful decoding that did not occur.
- Keep unsupported graphics/codec features and broader communication/persistence
  requirements in their explicit related programs. If required to close an
  included lifecycle case, implement that prerequisite first.

**Gate:** loopback HTTP fixtures with known headers, bodies, redirects and
ordering; parser scripts observing the correct partial tree; document.write
reentrancy; async/defer/module ordering; aborted navigation and teardown; genuine
frame origins; blocked policy requests stay blocked and allowed ones occur.
Use Windows App behavior, not a synthetic DOM host, for native integration.

### F11 — Forms, focus, activation and editing

**Deliverables**

- Implement all HTML input states and fallback rules: hidden, text/search/tel,
  url/email/password, date/month/week/time/datetime-local, number/range/color,
  checkbox/radio/file, submit/image/reset/button. Include all value modes,
  sanitization, dirty flags, valueAsNumber/valueAsDate, stepping and constraints.
- Complete textarea, button, select (including multiple/listbox and current
  customizable-select/selectedcontent contracts), option/optgroup, datalist,
  label, fieldset/legend, output, meter and progress.
- Complete form ownership/reassociation, disabled/read-only semantics,
  collections/named controls, labels, checked/selected/default state, reset,
  constraint validation, submit/requestSubmit, implicit submission, submitter
  overrides, successful controls, charset/enctype/FormData serialization,
  multipart/file input and actual permitted navigation.
- Complete focus/blur and activeElement, sequential/spatially relevant keyboard
  navigation, tabindex/autofocus, inert/disabled behavior, access keys, activation,
  trusted/native versus synthetic events and cancellation/reentrancy.
- Implement editing hosts and designMode, selection and caret/range operations,
  input/beforeinput/composition semantics, native text editing and necessary
  clipboard/drag data boundaries. Reflection of contentEditable is insufficient.
- Complete details/summary, dialog/top layer, popover, command activation and
  their focus restoration/light-dismiss/default actions.
- Preserve native control identity and edited state when ancestor DOM,
  templates, styles and owners change.

**Gate:** native App editing/focus/selection and real form requests, including
controls inside moved/replaced/imported subtrees; keyboard and pointer paths;
invalid values, canceled submissions, disabled controls, file authority,
cross-document moves and rollback. Native dialog styling need not imitate one
OS theme, but semantics, usable layout and hit targets must agree.

### F12 — Native Custom Elements and Shadow DOM

**Deliverables**

- Implement custom element names/definitions, constructors, upgrade candidates,
  autonomous/customized built-ins, definition/whenDefined/upgrade, scoped
  registry behavior where specified, reaction queues and ordered lifecycle
  callbacks, including adoption and specified state-preserving moves.
- Implement ElementInternals, form-associated custom elements, disabled/form/
  reset/state restoration and semantic accessibility hooks.
- Implement attachShadow with actual open/closed roots, declarative shadow,
  delegatesFocus and specified clonable/serializable/scoped-registry behavior.
- Implement slot assignment, assigned nodes/elements, slotchange, flattened
  tree behavior, composed events/retargeting, focus and selection boundaries.
- Integrate style scoping, inherited styles, slot/host/part behavior and layout/
  hit testing with the existing engine. Missing required CSSOM/style mechanics
  are explicit prerequisites, not fake shadow roots.
- Preserve the earlier noPatch polyfill path as a regression workload.
  Native components are included here because of the foundation objective,
  not because that selected YouTube bundle proved they were mandatory.

**Gate:** selected native component WPT and original connected Apps covering
upgrade order, nested templates, style/slot updates, native controls, form
association, real events, focus, owner moves, teardown and negative receiver/
closed-root/policy cases. A constructor existing globally does not close this
package.

### F13 — Default presentation, document modes and text integration

**Deliverables**

- Implement the HTML UA stylesheet/presentational hints and semantic defaults
  for all included families, preserving cascade precedence.
- Complete no-quirks/limited-quirks/quirks consumers across parsing, selectors/
  DOM, CSSOM/viewport and layout. Cover table cells, line height, percentages,
  body/root scrolling and the current Quirks Mode Standard rules.
- Validate block/inline/list/table/form/replaced-element default presentation,
  whitespace, hidden/inert/details/dialog, bidi direction, ruby and selection.
- Supply the CSS/text primitives required by those defaults: line construction,
  shaping/bidi/line-breaking and font metrics as actual dependencies. Pin the
  needed Unicode/OpenType standards/data. A subset cannot be called complete
  for scripts it visibly breaks.
- Keep HTML versus XML namespace styling and image/SVG/MathML integration
  consistent. Unsupported external visual capabilities remain named in the
  related-program register rather than being mistaken for parser defects.

**Gate:** controlled reference/reftest pages with fixed fonts, viewport and scale,
actual App pixels plus geometry/hit targets, all three modes and representative
multilingual text. Compare spec-defined behavior and allowed differences, not a
promise of arbitrary Firefox screenshot equality. Full CSS/fonts/SVG/MathML
conformance remains a separate program.

### F14 — Foundation closure and product re-entry

**Deliverables and gate**

1. Every required row has implemented source, applicable passing evidence and
   no unresolved or hidden required dependency. Complete all required entry
   paths; a standalone parser pass cannot close the loaded-browser path.
2. Run the frozen package-level aggregate cohorts once on the integration
   revision, accounting for every selected case/mode and diagnostic requirement.
   Required failed, skipped, blocked, crashed, timed-out or unrun rows block
   closure. Separately report out-of-foundation/tentative cohorts.
3. Review new upstream normative deltas without moving the denominator
   retrospectively. Correct or explicitly retain a source-bound open issue;
   don't relabel it as passed.
4. Run the connected Windows foundation journeys: malformed HTML; real XHTML;
   fragment/clone/import/adoption; controls/forms/editing; frames/navigation;
   lifecycle/scripts; native components; modes/default presentation.
5. Reconcile CAPABILITIES, the coverage register, compatibility source-review
   records and roadmap. Keep historical results historical. Record remaining
   whole-HTML, CSS, ECMAScript, media and platform work.
6. Record which YouTube shell/search/navigation/playback requirements the
   completed foundation satisfies and which need further CSS, JavaScript, data
   or media implementation. Site milestones and wider standards programs retain
   their separate scope and acceptance evidence.

The final report must say exactly which baseline was completed and which wider
areas remain. It must not say that all websites, all HTML, or YouTube now work
without the corresponding evidence.

## Testing protocol and progress accounting

### Use the right evidence for the claim

| Layer | What it establishes | What it cannot establish alone |
| :--- | :--- | :--- |
| Native corpus adapter | Original tokenizer/tree/encoding/URL output against pinned fixtures | Full browser WPT, native controls, layout or website success |
| Native DOM tests | Real identities, bindings, algorithms and ownership | Actual Windows UI input or pixels |
| Connected original App | Composed DOM, resources, controls, jobs, focus and rendering | A fresh live YouTube outcome |
| Unchanged browser WPT | The selected harness-complete assertions on exact source/platform | All requirements absent from that selection |
| Live site milestone | That exact journey/cohort/build actually worked | General standards conformance or a release |
| Release qualification | Exact-source delivery gates for an accepted milestone | A reason to rerun full release checks after each implementation PR |

WPT's reviewed parsing resources contain 62 data files, including four scripted
files. **File counts are not case counts.** The native adapter must enumerate
initial tokenizer states, scripting flags, fragment namespaces/contexts and exact
test IDs. Tree fixture cases and Blob/document.write/character-stream browser
wrappers are different evidence. Cases without an explicit scripting flag and
scripted fixtures need their correct separate treatment. Pending-spec/tentative
and special XML-infoset coercion fixtures need separate cohorts.

For every selected case/mode, record exactly one outcome: **pass, fail, timeout,
crash, harness-blocked, skipped/deferred, not-run**. Expected failures remain
failures. Unknown or failed enumeration makes the run incomplete. Do not discard
unsupported inputs, normalize away wrong trees, ignore diagnostic assertions
without disclosure, or count missing harness completion as a pass.

Each record binds the source/dirty state, executable/target/profile, adapter,
upstream/input hashes, expected-output hashes, mode/context, effective guards and
raw result. Compare identical cohorts; additions or upstream pin changes are
reported separately. The coverage register uses reviewed/planned/partial/blocked/
implemented/verified distinctions. No whole-project percentage follows from a
test total, API count, lines of code or PR count.

### Verification scope

- During an increment: run the reproduction or newly implemented contract,
  directly affected regressions and relevant negative/security checks.
- At a coherent package boundary: run its selected unchanged corpus and
  connected App integration. Broaden only for a concrete remaining risk.
- At foundation closure: aggregate the fixed baseline and required Windows
  journeys. Ordinary PRs do not gain a full release/whole-WPT gate.
- Development builds use build_dev and existing incremental caches without
  changing the source/configuration identity requirements.
- A new merge or passing test does not establish another live-site result.
  A diagnostic site probe needs a specific unresolved runtime question that
  source and original fixtures cannot answer.

## Migration invariants

1. **One production semantic path.** Introduce coherent modules behind existing
   call sites; temporary comparison code may be test-only. Remove superseded
   production paths when migrated. Do not choose parsers by website, test name
   or claimed HTML version.
2. **Parse versus activate.** A legal DOM node/attribute must be representable
   without automatically granting script, network, style, form or native resource
   authority. Conversely, ordinary required behavior cannot remain a blanket
   refusal once its real implementation is available.
3. **Stable identity.** Preserve node/Attr/collection handles, ownership, saved
   aliases, live ranges and observers through specified mutations. Keep recent
   clone/import joint reservations and late-failure cleanup intact.
4. **Prepublication failure containment.** Do not publish partial internal tree
   changes after failed validation/admission. Preserve aliases and native state
   belonging to that unpublished operation. Conversions and required callbacks
   can run author code: their observable effects and completed normative mutations
   remain in the specified order even if a later step throws. An innerHTML value
   conversion that mutates another node and then throws must retain that author
   mutation. Streaming/document.write retain specified partial/aborted state.
   Charge real allocations/work, never refund still-live backing, and reject
   stale generations or invalid cross-realm/native receivers.
5. **Standards errors versus operational failure.** HTML parse errors usually
   recover; XML well-formedness errors are fatal. Policy denial, unsupported
   capability, actual resource exhaustion and cancellation are different.
   Never turn truncated parsing, denied loads or exhausted work into success.
6. **Necessary capacity changes only.** Replace input-controlled recursion and
   invalid fixed representation ceilings when implementing the family. Retain
   checked arithmetic, actual backing/work admission and cancellation. Do not
   restart every deferred capacity category or replace limits with usize::MAX
   without the necessary storage/algorithm change.
7. **Policy preservation.** Keep TLS, CSP/nonce provenance, origin, credentials,
   sandbox, user-file and event-trust enforcement. Negative fixtures include
   late inline-handler/resource mutations, duplicate attributes, denied schemes,
   entity expansion, stale owners and post-failure recovery.
8. **Original expectations can be corrected; upstream assertions cannot be
   weakened.** Explain changed DOM expectations with a normative reference.
   Cross-browser comparisons help resolve ambiguity but do not supersede the
   standard or justify copying another implementation.

## Related programs

The foundation includes local architecture and capacity changes needed by its
required contracts. Broader architecture and capacity programs remain deferred.

XPath, XSLT, XML Schema, XML 1.1 and validating XML processing are an explicit
XML-tools program. XML 1.0 nonvalidating parsing and XHTML do not imply those
features. Existing intentional policy restrictions remain visible: if one
prevents an included conformance requirement, report it as blocked rather than
weaken the policy or claim a pass.

## Primary references

- [HTML contents](https://html.spec.whatwg.org/multipage/),
  [parsing](https://html.spec.whatwg.org/multipage/parsing.html),
  [elements/interfaces index](https://html.spec.whatwg.org/multipage/indices.html),
  [dynamic markup and serialization](https://html.spec.whatwg.org/multipage/dynamic-markup-insertion.html),
  [XML syntax](https://html.spec.whatwg.org/multipage/xhtml.html),
  [rendering](https://html.spec.whatwg.org/multipage/rendering.html) and
  [obsolete required behavior](https://html.spec.whatwg.org/multipage/obsolete.html).
- [DOM](https://dom.spec.whatwg.org/), [Web IDL](https://webidl.spec.whatwg.org/),
  [Infra](https://infra.spec.whatwg.org/), [Encoding](https://encoding.spec.whatwg.org/),
  [URL](https://url.spec.whatwg.org/), [MIME Sniffing](https://mimesniff.spec.whatwg.org/),
  [Quirks](https://quirks.spec.whatwg.org/).
- [XML 1.0](https://www.w3.org/TR/xml/),
  [Namespaces in XML](https://www.w3.org/TR/xml-names/),
  [current DOM Parsing/XML serialization](https://w3c.github.io/DOM-Parsing/).
- [WPT documentation](https://web-platform-tests.org/),
  [pinned parsing corpus format](https://github.com/web-platform-tests/wpt/blob/42e27408034f57afe4c726476d1a0bd96b873129/html/syntax/parsing/resources/README.md),
  [html5lib-tests](https://github.com/html5lib/html5lib-tests/tree/c5495c539ca8a79c3c5107f5f14010bcff9a35d5).

Requirement labels and inventory anchors derive from the identified standards.
No third-party implementation or test corpus is included by this planning change.

