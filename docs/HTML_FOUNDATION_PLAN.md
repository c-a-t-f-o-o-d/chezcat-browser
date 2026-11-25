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
The broader [CSS foundation program](CSS_FOUNDATION_PLAN.md) is planned after
F14 acceptance. CSS required to close an included HTML/default-presentation
contract remains a prerequisite here; product journeys retain their own
dependency closure and evidence.

F00 met its requirements, input-lock and baseline criteria and was delivered in [v0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4). The original
source-bound native results retain their failures and harness gaps. F01 now has
shared native document, node, namespace and UTF-16 operations; F02 has shared
decoders and initial document charset selection; F04 has named references,
script/text states and native comment recovery. Required integrations and
unchanged browser assertions remain open.
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
SVG raw-ID writes and foreign raw-ID imports, quirks-mode folding, XML ID typing and frozen
qualification remain separate. F01 stays partial.

Exact collection named-lookup increment: HTMLCollection.namedItem retains converted
UTF-16 keys across current native children/tag/image views. Prepaid native ID/name
comparisons preserve actual node identity and skip scalar projection aliases.
Native work/copy refusal, conversion ordering, origin recovery and App interaction
have original checks. Raw named-property keys, full named descriptors, raw-ID
imports and frozen qualification remain separate; F01 stays partial.

Exact visual-ID consumer increment: native anchors read authoritative scalar IDs,
omitting raw IDs rather than publishing replacement-character aliases. Table
anchor spans and fixed state follow actual admission. Label association compares
borrowed exact native ID/for records while preserving first tree-order matches.
Component and App pointer/navigation fixtures qualify raw records; production
probes cover scalar controls. Raw for producers and foreign raw-ID imports, complete fragment/label
behavior and frozen qualification remain separate; F01 stays partial.

Exact form-name consumer increment: scalar form properties and supported document
form names compare authoritative native ID/name code units. Replacement-character
keys skip raw aliases before named-field prototype routing and returned lookup.
Shared borrowed control matching prepays native reads without copying keys.
Original component and native App checks cover live names, prototype fallback,
duplicate refusal, work exhaustion/recovery and paint/editor/link/page identity.
Raw property keys, raw name producers and foreign raw-ID imports, RadioNodeList, past-names maps,
complete document named access and frozen qualification remain separate. F01
stays partial.

Canonical Element.id increment: the shared inherited getter reads exact native
code units and the setter uses the existing attribute transaction. Native main
records, inert created/parsed elements and supported SVG share the accessor,
while brand/origin guards precede conversion. Form named-field precedence and
author prototype overrides remain ordinary lookup. Reads prepay attribute/value
work and copy limits. SVG raw-ID writes, child writes and raw materialization
retain their existing refusal; frozen qualification and complete ID integration
remain separate. F01 stays partial.

Public HTML ID producer increment: genuine HTML null-namespace id writes retain
exact UTF-16 through the shared native attribute transaction. Element.id,
setAttribute/NS, attached Attr aliases and setAttributeNode/NS preserve lookup,
observer and attribute identity. Public clone/move/removal and exact
selector/collection consumers are covered with resource refusal/recovery and
actual App pointer/paint/editor/link checks. SVG raw-ID writes, child writes, raw-
ID materialization, dynamic form-owner resetting and frozen browser qualification
remain separate. F01 stays partial.

Exact HTML ID materialization increment: imported and parked HTML nodes retain raw
ID code units through native copies, source/target owners, stable Attr identities
and template-content promotion. Promotion uses the shared prepaid exact form-key
guard, so raw aliases cannot trigger literal-key refusals. Literal explicit-
association changes, SVG/foreign raw IDs and other unsupported raw attributes
retain their guards. Original public-producer, native control, resource
refusal/recovery and App input/paint/editor/link checks qualify this slice; full
owner resetting and frozen browser qualification remain separate. F01 stays
partial.

Exact form-owner ID increment: native initialization borrows authoritative ID/form
DOMStrings for exact owner matching, preserving first-match nonform blockers and
explicit misses. ID writes and tree moves share prepaid borrowed comparisons for
the existing explicit-association refusal. Raw projection aliases can change or
move without touching a literal owner; projection-equal identity changes still
refuse atomically. Original component and App fixtures qualify native records.
Raw form producers and foreign raw-ID imports, general dynamic owner resetting, parser form-owner
magic and frozen qualification remain separate. F01 stays partial.

Exact live-name filter increment: displayed-document getElementsByName retains
converted UTF-16 filters in admitted native NodeList storage. Live reads compare
authoritative HTML name attributes rather than scalar projections and preserve
real tree order through moves and scalar name changes. Conversion occurs once;
brand/Symbol/abrupt behavior and native copy/string/storage/read-work refusal
retain recovery. Main raw-name records are component fixtures; App pointer-driven
native queries retain paint/editor/link/page identity, and production controls
cover scalar names and unmatched raw queries. Main raw-name producers,
inert/child Document expansion and frozen qualification remain separate.
The canonical Document.prototype.getElementsByName operation now has inherited
metadata and ordinary override/deletion/restoration. Saved functions retain the
exact displayed-document live backend, genuine Document branding and origin
checks before conversion. Owner-bound live lists now retain inert HTML/XML/generic
and supported child Document roots, traverse their actual native links and match
only HTML elements' null-namespace name attributes by exact code units. Moves,
adoption and name changes update results; template content remains a separate
root. Child lists use their real owner realm and preserve origin checks on saved
reads and iteration. Traversal has paid native lookups, bounded iterative order,
fallible combined scratch admission including replacement peaks, and refusal/recovery.
Original native/App checks and production probes qualify this scoped integration;
control/slot/frame/resource raw-name producers, raw-name imports, child writes
and frozen browser qualification remain open.
F01 stays partial.

Ordinary HTML name producer increment: supported ordinary containers now retain
raw UTF-16 null-namespace name values through setAttribute/NS, attached Attr
value/nodeValue/textContent and setAttributeNode/NS. The existing admitted native
attribute transaction serves exact live-name and named-collection consumers,
clone/move/removal and actual observation old values. Retained backing and
work/copy/string/storage refusal preserve recovery. Original native/App and
production checks qualify this scope; native controls, slots, frames, resources,
foreign/namespace-qualified raw names and inert raw-name imports stay guarded.
F01 stays partial.

Ordinary HTML name materialization now shares the namespace, local-tag and null-namespace attribute policy with public producers. Native import copies exact raw names without changing source owners; parked and cloned template contents preserve Attr identity through promotion and update exact live queries. Work, copy and retained-storage refusal preserve source and destination state with fresh recovery. Native controls, slots, frames, resource elements and foreign or namespace-qualified raw names remain guarded. Original native/App and production checks qualify this slice; child writes and frozen browser qualification remain separate. F01 stays partial.

Common HTML metadata materialization now admits exact null-namespace class, title, lang
and dir values on supported HTML elements. Imports retain independent attributes and
source ownership; parked and cloned template contents preserve exact metadata through
promotion. Class tokens, live class queries and CSS alias exclusion use actual code
units; title/lang retain raw values and dir keeps invalid-value reflection semantics.
Imported native controls retain real editor values. Work, copy and retained-storage
refusal preserve source/destination state and fresh recovery. Original native/App and
production checks qualify this slice, including actual class-based painting. Foreign or
namespace-qualified raw metadata and unsupported active raw attributes retain their
guards; frozen browser qualification remains separate. F01 stays partial.


Canonical Element.className now reflects the actual class DOMString through inherited
getter/setter metadata, saved accessors and ordinary prototype
override/deletion/restoration. Genuine receiver and captured-origin checks precede
conversion; inert HTML property writes now update native attributes, stable Attr
identities and exact observation values. Main and inert public producers retain raw
units through import and template promotion, with class tokens, live queries and CSS
alias exclusion using the actual record. Paid reads and work/copy/string/storage refusal
retain fresh recovery. Original native/App and production checks qualify this base
reflection, including class-based pixels and preserved editor/link/page identity.
Initial child writes, main raw SVG writes, SVGAnimatedString-specific interfaces, inert
classList and frozen browser qualification remain separate. F01 stays partial.

Canonical Element.classList now has inherited getter/setter metadata, stable supported main HTML/SVG token-list identity, saved accessors and ordinary prototype changes. Its PutForwards setter validates the genuine receiver and captured origin before reading the current classList property, then forwards the original value to the selected object value setter. Author getters, setters, proxies, abrupt effects and late native DOMString conversion retain their order; false Set results are ignored even for strict callers. Native attribute identity, observations and resource refusal/recovery remain shared with DOMTokenList.value. Original native/App and production checks qualify this slice, including class-based paint with preserved editor/link/page identity. Default inert, child and other token-list owner storage and frozen browser qualification remain separate. F01 stays partial.

Inert HTML classList now binds stable native token-list handles to actual owner attributes, including elements without a main-document slot. Exact value, indexed and membership reads, iteration, add/remove/replace/toggle and PutForwards writes share the existing native readers, serializers and attribute transactions. Scoped immutable node views copy no attribute payload or token cache; token metadata and selected output share admitted scratch headroom. Real Attr identities, observation old values, independent import/clone state and saved token lists survive supported template promotion. Original native/App and production checks cover raw-versus-replacement identity, conversion/validation ordering, work/copy/string/storage refusal, cancellation and fresh recovery, with actual class-based paint and preserved editor/link/page identity. Child and inert foreign token-list owners, broader SVG mutation and frozen browser qualification remain separate. F01 stays partial.

Genuine inert XML and other namespace Elements now use the existing bounded native ordinary-property store. Exact UTF-16 and Symbol keys, descriptors and author accessors preserve real Element identity and native attribute state; clone/import omit author properties, while supported adoption preserves them. Canonical native accessors bypass author shadows, and classList PutForwards can select an author getter without prematurely converting the original value. Native quota refusal/recovery and captured child-origin checks remain enforced. Original native/App and production checks qualify this slice, including HTML paint driven through genuine XML author accessors with preserved editor/link/page identity. Default inert foreign token-list storage, broader foreign materialization and frozen browser qualification remain separate. F01 stays partial.

Inert Element classList now binds the shared native token backend across null, XML, MathML and SVG namespaces, including owners without a main-document slot. Exact reads, iteration, mutations and PutForwards use actual attributes and preserve Attr identity, observation and saved handles through supported adoption; clone/import token state remains independent. Inert className and token writes select the null-namespace class attribute without overwriting a same-spelling namespaced attribute. Original native/App and production checks cover namespace/case separation, raw UTF-16 identity, conversion ordering, captured origin, scratch headroom and resource/cancellation refusal with fresh recovery. Actual App paint consumes XML token values through supported HTML materialization while preserving editor/link/page state. Initial child token storage, broader foreign materialization, main SVG raw writes and frozen browser qualification remain separate. F01 stays partial.

Inert Element.id and common HTMLElement reflection setters now select null-namespace content attributes using the shared native namespace-aware transaction. Same-spelling namespaced attributes remain independent for id, title, lang, dir, hidden and tabindex; native Attr identities, exact ID lookup, observation and clone state follow the selected attribute. Original native/App and production checks cover raw UTF-16 values, native conversion reentry, owners without a main-document slot, captured origin and resource/cancellation refusal with fresh recovery. Actual App paint uses the reflected native values through supported HTML consumers with preserved editor/link/page identity. Generic setAttribute/removeAttribute retain their qualified-name behavior; broader foreign materialization, child mutation and frozen browser qualification remain separate. F01 stays partial.

Nullable ARIA string reflection now reads exact native DOMString values through the shared optional attribute reader, preserving absent-null versus present-empty state and raw UTF-16 identity. Genuine inert Element setters retain nullable conversion, actual null-namespace Attr identity, observations and independent clone/import state through supported adoption; same-spelling namespaced attributes remain separate. Original native/App and production checks cover registered mappings, native author-shadow bypass, conversion reentry, receiver/Symbol/origin ordering, bounded raw reads and resource/cancellation refusal with fresh recovery. Actual App paint uses reflected inert values through supported HTML consumers with preserved editor/link/page state. Displayed raw ARIA writes and raw ARIA materialization, child writes, accessibility-tree integration and notifications retain their separate boundaries. F01 stays partial.

Displayed HTML now preserves exact registered ARIAMixin and role DOMStrings through canonical nullable setters, generic attribute producers and attached Attr value/nodeValue/textContent writes. The same HTML/null-namespace qualification admits existing independent import and template promotion consumers, preserving actual Attr identity, owners, observations and literal CSS separation. Native control value state remains independent of ARIA metadata. Original native and in-process App checks cover registered mappings, transfer isolation, genuine paint/editor/link state and atomic resource/cancellation refusal with recovery. Foreign or namespace-qualified raw metadata, unknown ARIA attributes, child writes, accessibility-tree integration and notifications retain their existing boundaries. F01 stays partial.

Public HTML label for values now retain exact DOMString units through generic attribute producers, attached Attr aliases and native attribute-node replacement. The shared HTML-label/null-namespace qualification also admits independent label imports and template promotion while preserving native Attr identity, owners and observations. Existing native label association compares the actual for and ID records, retaining first tree-order noncontrol blockers, explicit misses and disconnected-node boundaries. Original native checks exercise exact versus replacement-character targets, conversion effects, origin validation, quota/cancellation refusal and transfer recovery. In-process App pointer input activates the actual exact target after public writes, import and promotion while preserving neighboring editing, link hits and page identity. Raw for values on other or foreign elements, qualified raw attributes, label IDL APIs and wider form-owner resetting remain separate. F01 stays partial.

Canonical HTMLLabelElement prototypes now expose enumerable/configurable htmlFor accessors over actual HTML-label attributes. Main and inert labels retain exact DOMString conversion of null and undefined, absent-empty reads, null-namespace Attr identity, original observations and independent clone/import/adoption state. Saved native accessors preserve real brands and captured origin checks before author conversion; ordinary own/prototype shadows and deletion keep the underlying native attribute separate. Bounded reads, writes, cancellation and no-main-slot creation retain refusal/recovery evidence. Original in-process App pointer checks use htmlFor writes through actual native label targeting, import and template promotion with checkbox paint, neighboring editor/link state and page identity. Direct construction, child writes, label control/form IDL and wider form-owner resetting retain their existing boundaries. F01 stays partial.

HTMLLabelElement.control now exposes a canonical readonly accessor over actual main and inert HTML label trees. Exact null-namespace for and ID records select the first tree-order ID match only when it is labelable; absent for selects the first labelable descendant, with hidden inputs and foreign elements excluded. Built-in meter, output and progress elements participate even without native widget state. Detached, cloned, imported, adopted and promoted roots retain actual element identity; author properties do not become association authority. Native origin validation, bounded attribute comparisons, admitted traversal scratch, cancellation and fresh recovery remain checked. Original in-process App checks compare the accessor with actual pointer activation while retaining checkbox paint, neighboring editing, link hits and page identity. Child-tree association, form-associated custom elements, the label form getter, native input-type changes, native-control template promotion, wider form-owner resetting and full label activation semantics remain incomplete. F01 stays partial.

HTMLLabelElement.form now exposes a canonical readonly accessor for the actual associated control owner. Existing displayed native controls retain their stored form owner; pure inert and ordinary controls without native widget state use bounded native connected or detached tree association. Connected explicit form values compare exact null-namespace IDs, require the first match to be an HTML form, and retain nonform and replacement-character blockers. Detached explicit references fall back to the nearest actual HTML form ancestor. Meter and progress remain labelable without a form owner; output participates in form association. Native reads ignore author control/form getters and retain descriptor shadows, clone/import/adoption and supported label-promotion identity, origin, resource and cancellation refusal/recovery. Original in-process App checks compare form and control accessors with native pointer activation, checkbox paint, adjacent editing and link state. Child trees, form-associated custom elements, broader parser/form-owner resetting, native input-type changes, native-control template promotion and full activation semantics remain incomplete. F01 stays partial.

HTMLInputElement.labels now returns a canonical readonly, stable-identity live NodeList over actual same-root HTML-label associations. Exact for and ID values, first nonlabelable blockers, implicit descendants, current tree order and disconnected roots use native association records. Hidden input reads return null while an already retained list updates to empty, then recovers its original identity when the inert type changes back. Native collection item, keys, iteration, callbacks and argument-conversion reentry read current membership; author properties never become association authority. Identity creation and refresh retain origin checks, metadata quotas, bounded native work/copies, simultaneous outer and nested traversal scratch admission, cancellation and fresh recovery. Clones, actual adoption, independent import and supported label promotion retain native identity, with in-process App pointer/paint/editor/link evidence. Other built-in labels interfaces, child and unsupported parser backings, form-associated custom elements, native input-type changes, native-control template promotion and full label activation remain required unfinished work. F01 stays partial.

HTMLButtonElement.labels and HTMLTextAreaElement.labels now expose distinct canonical readonly accessors over the shared bounded live label-list runtime. Their native NodeLists retain stable identity, exact same-root associations, current tree order, ordinary author descriptor precedence and native collection iteration. A hidden type content attribute does not suppress either non-input list. Clone/adoption/import and supported label promotion retain actual source authority; captured-origin, metadata, work/copy, simultaneous traversal storage and cancellation refusals retain fresh recovery. In-process App checks retain native pointer, paint, editor, link and page identity evidence. Meter, output, progress and select labels interfaces, child and unsupported parser backings, form-associated custom elements, native input-type changes, native-control template promotion and full label activation remain unfinished. F01 stays partial.

Meter, output, progress and select now have canonical native HTML interfaces and distinct readonly labels getters, completing the seven ordinary built-in labelable interfaces in the supported main and inert trees. Shared operation metadata keeps per-realm descriptor installation and genuine receiver validation aligned. Stable live NodeLists retain exact native associations, current tree order, author descriptor precedence, clone/adoption/import identity, supported label promotion and collection iteration. Non-input hidden content never suppresses these lists. Native origin, metadata, bounded work/copies, combined traversal storage and cancellation retain refusal/recovery checks. App evidence retains actual select state and popup interaction alongside label updates, paint, editor, link and page identity. Whole-control main import for meter, output and progress remains guarded with atomic refusal; label imports and supported select imports retain actual associations. Other interface IDL, child and unsupported parser backings, custom-element internals, native input-type changes, native-control template promotion and full label activation remain required unfinished work. F01 stays partial.

Public HTML OUTPUT for attributes now admit exact DOMString values through setAttribute, explicit null-namespace writes and attached Attr value/nodeValue/textContent aliases. Native attribute identity, exact observer old values, namespace collisions, author conversion effects and CSS replacement-character exclusion retain the original UTF-16 units. Output input-reference tokens remain independent of the output element's own labels, labeled control and form owner. Work/copy/storage/cancellation and captured-origin refusals preserve native records and fresh recovery. In-process App evidence follows actual button writes through exact attribute-based paint while preserving neighboring editing, link hits and page identity. The canonical output htmlFor DOMTokenList binding, other output value/default-value IDL and whole-output main import/template promotion remain required unfinished work. F01 stays partial.

HTMLOutputElement.htmlFor now exposes a canonical SameObject DOMTokenList over its actual null-namespace for attribute. Exact UTF-16 value and token reads, mutation, iteration and observation share the existing bounded native token runtime. Its PutForwards setter performs ordinary Get before forwarding the original value to the selected object value setter, preserving author overrides, proxy behavior, conversion effects, receiver brands and captured-origin checks. Stable handles survive inert cloning and adoption without consuming displayed-node slots; the list remains independent of classList and the output element own label association. Focused native and in-process App checks cover exact attribute-based paint and interaction. Other output IDL, whole-output main import and promotion, child lifecycle and complete browser qualification remain required; F01 remains partial.

HTMLOutputElement.form now exposes a canonical readonly accessor over the shared native form-owner lookup. Native widget owners remain authoritative; ordinary and inert output elements resolve exact null-namespace form/ID records, first-match blockers, connected explicit owners and detached nearest-form ancestry without consulting author getters or htmlFor tokens. Inert clones, adoption and HTML-namespace output elements in XML documents retain independent owner trees without displayed-node slots. Work, copy, traversal storage, cancellation and captured-origin refusals retain recovery. In-process App checks follow actual output tree moves through form and label getters, exact CSS paint and nearby editing/link behavior. Native Reflect.set retains its existing explicit refusal. Other output IDL, whole-output main import/promotion, child lifecycle and complete browser qualification remain required; F01 stays partial.

HTMLOutputElement now exposes canonical exact DOMString name accessors and readonly type metadata returning output independently of the type content attribute. OUTPUT name writes through IDL, public attributes, explicit null-namespace writes and attached Attr aliases share the existing paid native transactions and exact live getElementsByName lookup. Author descriptors, conversion reentry, actual Attr identity, observer old values and CSS replacement-character exclusion retain native semantics; name does not select the output form owner, labels or htmlFor list. Inert clone/adoption and HTML-namespace metadata in XML owners require no displayed-node slots. Native work/copy/storage/string/cancellation and captured-origin refusals retain recovery; in-process App evidence follows actual name-based paint and stable nearby interaction. Output value/defaultValue/reset, constraint validation, complete form.elements participation, whole-output main import/promotion, child lifecycle and full browser qualification remain required; F01 stays partial.

HTML OUTPUT textContent now uses the native whole-child replacement transaction for connected and detached main-document elements. Exact DOMStrings create a fresh Text node, empty values remove all children, and saved descendants, live NodeLists, Range boundaries and childList records retain native identity. Ordinary parent replacement can retire a passive OUTPUT subtree. Shared replacement traversal admits stack, descendant and caption backing with input text and overlapping growth before publication, charging copies and cancellation. Native control/resource removal and whole-output import/promotion retain their existing guards. Focused native and in-process App checks cover actual replacement, paint and neighboring interaction. Output value/defaultValue/reset and complete descendant-text reading remain required; F01 stays partial.

Native main-tree textContent now concatenates actual Text/CDATA descendants in tree order, including hidden and script/style/object/noscript source text, independently of paint activity. HTML template contents remain in their separate fragment; iframe fallback reads retain represented-topology guards and never walk child documents. Main and inert readers admit traversal/part backing, overlapping growth, returned text and temporary UTF-16 join units together, with paid copies and cancellation. Exact split surrogate units, saved children, native observers and resource ownership remain unchanged by reads. Focused native and in-process App checks cover actual source-derived text paint and neighboring interaction. Output value/defaultValue/reset and remaining unsupported child/parser lifecycles stay required; F01 remains partial.

HTMLOutputElement now exposes canonical exact DOMString value/defaultValue accessors. value reads actual descendant text and replaces real children while preserving the native default-value override; defaultValue replaces children only before an override exists, then updates the independent default without child mutation records. Empty overrides remain distinct from absence, inert clones start with independent default state, and adoption preserves existing native authority. Conversion reentry, genuine brands, captured-origin checks, bounded storage/work/copies, cancellation and failed child transactions preserve refusal/recovery behavior. Focused native and in-process App pointer/paint checks cover current/default text and independent form/label/token associations. Form reset, whole-output main import/promotion, child lifecycle, validation and full form.elements participation remain required; F01 stays partial.

Native reset-button actions now restore associated HTML OUTPUT elements from their actual default value and clear each native override only after successful child replacement. Selection uses current exact form ownership after uncanceled reset listeners, including external associations, with bounded traversal and retained scratch. Reset shares genuine child-list/Range transactions, preserves protected resource/control refusals, and does not emit input/change events. Observer delivery follows widget reset within the same action allowance. Scripting-disabled actions use the same bounded native text transaction without an interpreter. Focused native and in-process App pointer/paint checks cover default/empty modes, cancellation, ownership, quotas, recovery and independent editor/link/page state. Full form.reset IDL, form.elements, constraint validation, whole-output main import/promotion and child lifecycle remain required; F01 stays partial.

Public HTML OUTPUT form content attributes now admit exact DOMStrings through setAttribute, explicit null-namespace writes and attached Attr value/nodeValue/textContent aliases. The shared paid transaction preserves actual attribute identity, conversion reentry and observer old values; existing exact native owner lookup, label.form and reset selection consume the current attribute without a second authority. HTML name normalization, namespace isolation, sparse native state, resource/cancellation refusal and fresh recovery remain bounded. Focused native and in-process App pointer/paint checks cover current ownership, matching versus unrelated reset buttons and independent editor/link/page state. The output.form IDL property stays readonly, and native widget reassociation, whole-output main import/promotion, full form.reset/form.elements/validation and child lifecycle remain guarded or unfinished; F01 stays partial.

HTML OUTPUT now exposes canonical readonly willValidate and validationMessage getters and canonical checkValidity/reportValidity methods. OUTPUT is not submittable and therefore is not a constraint-validation candidate: these getters return false/empty and the methods return true without invalid events, focus, scrolling, child records or default-value changes. Genuine native branding, captured origin authority, work/deadline admission, normal author shadow/delete/restore behavior and inert clone/adoption remain enforced. Unobserved methods retain bounded private data-slot definitions; first observation creates a stable callable without changing ordinary descriptors or the function cap. Focused native and in-process App pointer/paint/editor/link checks cover refusals and recovery. Mutable custom-validity error state, ValidityState, other controls, full form.reset/form.elements, whole-output main import/promotion and child lifecycle remain required; F01 stays partial.

HTML OUTPUT now has native custom-validity error storage, canonical setCustomValidity(DOMString) and a live readonly ValidityState with stable native identity. Custom errors normalize CRLF/CR while preserving exact UTF-16; customError and valid reflect that one authority, and inapplicable OUTPUT constraints stay false. OUTPUT remains a noncandidate: validationMessage is empty and checkValidity/reportValidity return true without invalid events or UI effects. Required arguments, once-only conversion/reentry, abrupt completion, genuine brands, captured origin, author shadows and work/copy/string/storage/deadline refusals retain their native boundaries. Clone state is fresh; adoption preserves identity and custom error. Focused native and in-process App pointer/paint/editor/link evidence remains scoped. Other controls, form.reset/form.elements, whole-output main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLInputElement, HTMLButtonElement, HTMLSelectElement and HTMLTextAreaElement now expose canonical readonly form getters with concrete native brands. They share the existing exact owner lookup: admitted native widgets retain their actual owner authority, while inert/ordinary controls use current connected explicit IDs and detached ancestor rules without an owner cache. Readonly assignment, inert author shadow/delete/restore, native widget form-descriptor refusal, namespace isolation, captured origin and bounded work/deadline refusal/recovery remain enforced. Focused native and in-process App pointer/paint/editor/link checks cover real owner reads and unrelated state preservation. Native widget reassociation, child lifecycle, broader form APIs and whole-control import/promotion remain guarded or unfinished; F01 stays partial.

HTMLSelectElement and HTMLTextAreaElement now expose canonical readonly type getters. Select returns select-one or select-multiple from the exact null-namespace lowercase multiple attribute presence, independently of its value, size or widget admission; textarea returns textarea. Native and inert concrete brands, namespace isolation, captured origin, author override/delete/restore and work/copy/string/deadline refusal/recovery apply without copying attribute values. Focused native and in-process App pointer/paint/editor/link checks remain scoped. Multiple/listbox widgets, broader select/textarea APIs, whole-control import/promotion and child lifecycle remain unfinished; F01 stays partial.

HTMLFormElement now exposes a canonical readonly length getter that counts actual listed built-in controls with the form as owner, including fieldset/object/output and unsupported widget states, excluding image-button inputs. It traverses the actual root, respects template parking and native widget ownership, and uses exact connected IDs or detached ancestors for controls without widget state. No count cache or author property supplies authority. The outer traversal uses a bounded parent/sibling walk without scratch allocation; nested exact-owner lookup retains its own paid scratch boundary. Concrete brands, captured origin, existing native named-property precedence and work/copy/storage/deadline refusal/recovery remain enforced. Native and in-process App pointer/paint/editor/link evidence stays scoped. HTMLFormControlsCollection, RadioNodeList, broader form names/lifecycle, form-associated custom elements, main import/promotion and child lifecycle remain unfinished; F01 stays partial.

HTMLFieldSetElement now has a canonical native/inert interface and readonly form/type getters. Form ownership uses the existing exact current association helper with connected explicit IDs and detached ancestors; type returns fieldset. Concrete namespace/local-name brands, illegal direct construction, author override/delete/restore, captured origin and bounded work/copy/string/storage/deadline refusal/recovery retain native authority. Existing disabled-fieldset/first-legend widget behavior remains separate from metadata reads. Focused native and in-process App pointer/paint/editor/link checks are scoped evidence. Fieldset collections and broader form lifecycle, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes canonical name DOMString reflection over the actual null-namespace attribute. Native and inert setters convert once, preserve exact UTF-16, use current Attr identity after conversion and retain native observer, live-name and selector behavior. Author overrides remain ordinary descriptors; canonical borrowed accessors keep concrete brand and captured-origin checks. Reads and writes preserve work/copy/string/storage/deadline refusal and recovery, with scoped native and App pointer/paint/editor/link evidence. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes canonical disabled Boolean reflection over its own actual null-namespace attribute. Assignment uses Boolean truth without author primitive conversion; native writes reuse the existing paid descendant-control refresh, including nested fieldsets and the first-legend exception. Inert reflection, actual Attr identity and observer old values, concrete brands, author overrides and captured origin share the existing attribute transaction. Presence reads inspect paid attribute names without copying values or requiring string/storage allowance; work/copy/storage/deadline refusals preserve attributes and native control state before recovery. Native and App pointer/paint/editor/link evidence stays scoped. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes canonical readonly willValidate and validationMessage getters plus ordinary checkValidity/reportValidity methods for its noncandidate validation branches. They return false, empty text and true respectively, without invalid dispatch, focus/scroll changes, author argument conversion or descendant-control validation. Native and inert concrete brands, captured origin, author override/delete/restore and work/deadline refusal/recovery remain enforced. Empty/scalar results require no value copying or additional string/retained storage; native and App pointer/paint/editor/link evidence stays scoped. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes a stable live ValidityState and canonical setCustomValidity using the shared native custom-error index. Exact DOMString conversion, newline normalization and joint input/payload/index storage admission match OUTPUT; each concrete interface retains its own borrowed-accessor brand checks. Only customError and valid vary for these noncandidate controls; all other validity flags stay false, validationMessage stays empty and check/reportValidity remain true without invalid events or UI changes. Clones start with fresh errors, adoption preserves identity, and native form reset preserves existing custom errors independently for FIELDSET and OUTPUT. Readonly flags, author overrides, captured origin and work/copy/string/storage/deadline refusal/recovery retain native authority. Native and App pointer/reset/paint/editor/link evidence stays scoped. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement form content attributes now admit native public/null-namespace and attached Attr writers through the shared attribute transaction, including exact UTF-16 values. Current exact IDs determine the fieldset owner independently of author properties; changing the fieldset owner does not reassociate descendant controls or redirect their native reset. Conversion reentry, actual Attr identity, observer old values, namespace/raw-value guards, captured origin and bounded work/copy/string/storage/deadline refusal/recovery retain existing authority. Inert clone/adoption and native App pointer/reset/paint/editor/link checks remain scoped evidence. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLLegendElement now has a canonical native/inert interface and readonly form getter. Only an actual direct HTML fieldset parent supplies ownership, using the shared current fieldset association helper; other parents return null. Exact namespace/local-name brands, illegal direct construction, descriptor and enumeration behavior, author override/delete/restore, captured origin and bounded work/copy/storage/deadline refusal/recovery retain native authority. Exact owner IDs, inert clone/adoption and native App pointer/reset/paint/editor/link evidence remain scoped. Broader form collections/lifecycle, main import/promotion and initial child lifecycle remain required; F01 stays partial.

HTMLProgressElement now exposes canonical value/max numeric reflection and readonly position over actual native/inert attributes. Shared original decimal-prefix scanning supplies HTML ASCII-whitespace, finite-result and positive-zero parsing without changing ECMAScript number grammar. Missing value is indeterminate, invalid/negative values fall back to zero, current values clamp to the actual positive maximum, and nonpositive max assignments leave attributes unchanged. Finite-double conversion runs once before the shared attribute transaction, preserving exact Attr identities, observer old values, author effects, concrete brands, captured origin and bounded refusal/recovery. Inert clone/adoption, actual App pointer/attribute-driven CSS paint/editor/link checks and scoped production evidence retain existing labels and control authority. Native progress-bar painting, broader form collections/lifecycle, identity-preserving main adoption and initial child lifecycle remain required; F01 stays partial.

HTMLMeterElement now exposes six canonical numeric properties over actual native/inert attributes. Shared HTML floating-point parsing supplies finite defaults and ordered minimum/maximum, actual-value, low/high and optimum clamping; midpoint calculation remains finite across extreme ranges. Setters retain finite unclamped attribute values after one numeric conversion through the shared attribute transaction. Exact Attr identities and UTF-16 values, observer old values, concrete brands, captured origin, author effects and bounded refusal/recovery retain native authority. Inert clone/adoption, actual App pointer/attribute-driven CSS paint/editor/link checks and scoped production evidence retain existing labels and control state. Native gauge painting, broader form collections/lifecycle, identity-preserving main adoption and initial child lifecycle remain required; F01 stays partial.

Passive progress/meter cloning and main-document import now share the checked ordinary-element transaction. Shallow and deep copies retain exact actual attributes, independent Attr identities, fallback children and canonical numeric metadata; connected fallback text replacement retains real child/list/observer behavior. Existing dynamic connection retains source identities, labels and current metadata through shared promotion. Template parking, captured-origin checks and bounded work/copy/storage/node/deadline refusal/recovery preserve native state. Actual App pointer/attribute-driven CSS paint/editor/link evidence stays scoped; active-content, unrelated raw-name and native-control guards remain enforced. Native gauge painting, identity-preserving main adoption, broader form collections and initial child lifecycle remain required; F01 stays partial.

HTMLTimeElement now exposes a canonical dateTime DOMString accessor over actual native/inert datetime attributes. Empty defaults, exact UTF-16 values, once-only author conversion, concrete brands, current Attr identity, namespace distinctions and observer old values use shared bounded attribute transactions. Refused reads and writes preserve native state; captured-origin checks precede conversion. Ordinary clone/import and identity-preserving native adoption retain actual metadata and child handles. Actual App pointer actions update attribute-driven CSS paint while preserving fallback text, editor selection and link hits; scoped production probes exercise the built executable. Date parsing and visible-text formatting are outside this reflection behavior; broader HTML interfaces and initial child lifecycle remain required, and F01 stays partial.

HTMLDataElement now exposes a canonical value DOMString accessor over actual native/inert attributes. Exact UTF-16 values, empty defaults, once-only conversion, current Attr identity and observer old values share the bounded attribute transaction without creating form-control state. Checked ordinary-element clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve native records and existing active-content, namespace, raw-name and origin guards. Actual App pointer actions update attribute-driven CSS paint while preserving fallback text, editor selection and link hits; scoped production probes exercise the built executable. Broader HTML interfaces and initial child lifecycle remain required; F01 stays partial.

HTMLQuoteElement now provides one canonical constructor and prototype for both q and blockquote in each owner realm, with exact HTML namespace/local-name brands and ordinary native/inert element lifecycle. Clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement preserve actual attributes and children. Shared interface aliases allocate one constructor callable per owner; NamedNodeMap.getNamedItem uses the existing deferred data-method mechanism without changing descriptors, identity, arity or brands. Work/copy/storage/node/deadline refusal and recovery preserve existing origin, raw-name and active-content guards. Actual App pointer actions verify attribute-driven CSS paint, editor selection and link hits; scoped production probes exercise the built executable. Quotation cite URL reflection, user-agent quotation rendering, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHeadingElement now provides one canonical constructor and prototype shared by h1 through h6 in each owner realm. Exact HTML namespace/local-name brands preserve each rank through native/inert clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement. All six aliases allocate one constructor callable per owner and retain existing exact raw-name attributes and live name queries. NamedNodeMap.getNamedItemNS uses the existing deferred data-method mechanism while preserving namespace matching, ordinary descriptors, stable identity, arity and receiver checks. Work/copy/storage/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions verify heading attribute-driven CSS paint, editor selection and link hits; scoped production probes exercise the built executable. Document outline/accessibility integration, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHeadingElement now exposes its required align DOMString accessor over actual native/inert attributes for all six heading ranks. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement retain actual align metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve existing origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native align presentation hints, outline/accessibility integration, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLDivElement now exposes its required align DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Concrete DIV and heading accessor brands remain distinct. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement retain actual align metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve existing origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLLegendElement now exposes its required align DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Concrete LEGEND, DIV and heading accessor brands remain distinct. Inert cloning preserves independent attributes; alignment writes preserve actual fieldset ownership and first-legend control state. Work/copy/storage/string/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native LEGEND connection lifecycle, align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

Document.importNode now imports HTML LEGEND sources into inert documents through the existing bounded snapshot transaction. Native, pure inert and parked parsed sources retain exact attributes, independent Attr/child identities and the actual destination owner. Imported form metadata follows the destination direct parent; source fieldset ownership and native control state remain unchanged. Dictionary conversion and saved-child origin checks precede snapshot publication. Work/copy/storage/deadline refusal and no-slot recovery preserve source records and publish no partial identities. Other native control/resource families and focused native sources keep their existing guards. Actual App pointer actions use imported metadata to update real selector paint while preserving editor selection and link hits; scoped production probes exercise the built executable. Native LEGEND connection, align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

Native HTML LEGEND cloneNode and main-document import now publish detached copies through the existing bounded copy transaction. Native shallow/deep and inert/parked imports retain exact metadata, independent Attr/child identities and the actual main-document owner. Copied form metadata starts null while original fieldset ownership and first-legend native control state remain unchanged. Work/copy/storage/node/deadline refusal preserves source records and commits with fresh-job recovery. Active handler/resource guards and the existing native connection/inert promotion boundaries remain enforced; the shared ordinary-element admission is unchanged. Actual App pointer actions exercise both native clones and main imports through real attribute-selector paint while preserving editor selection and link hits; scoped production probes exercise the built executable. LEGEND connection, template transfer, align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLParagraphElement now provides the canonical owner-realm interface for exact HTML p elements and exposes its required align DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Paragraph, DIV, heading and LEGEND accessor brands remain distinct. The existing EventTarget.removeEventListener data method uses bounded deferred callable allocation while retaining stable identity, descriptors, receiver checks and actual listener removal. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback replacement retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLUListElement now exposes its required type DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. UL type, paragraph align and native control type authority remain distinct. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback replacement retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update type-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLUListElement now exposes its required compact boolean accessor over actual native/inert attribute presence. Boolean conversion sets an empty null-namespace attribute or removes it without invoking author conversion hooks; current Attr identity, exact UTF-16 backing, observer old values and independent type metadata use the bounded attribute transaction. Exact namespace/case and concrete receiver checks precede access. Getter work charges attribute-name scanning without string-copy or retained growth. Existing clone/import, identity-preserving adoption, dynamic connection and template parking retain actual attributes and children. Work/copy/storage/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLSpanElement now provides the canonical owner-realm interface for exact HTML span elements across native and inert creation. Constructor/prototype descriptors, saved bindings and concrete native identity share the existing HTML interface registry. Ordinary clones and imports preserve independent actual attributes and children; supported adoption, reserved-slot connection and shallow native template parking preserve actual owners and records. Deep native template parking and native host prototype mutation retain their existing guards. Work/copy/storage/node/deadline refusal and recovery preserve origin, namespace and active-content boundaries. The existing EventTarget.dispatchEvent data method uses bounded deferred callable allocation while retaining frozen data descriptors, stable identity, receiver checks and real cancelable dispatch. Actual App pointer actions update span attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLDListElement now provides the canonical owner-realm interface for exact HTML dl elements and exposes its required compact boolean accessor over actual native/inert attribute presence. Boolean conversion sets an empty null-namespace attribute or removes it without invoking author conversion hooks; current Attr identity, exact UTF-16 backing, observer old values and independent metadata share the bounded attribute transaction. Exact namespace/case and concrete receiver checks precede access. Getter work charges attribute-name scanning without string-copy or retained growth. The existing EventTarget.addEventListener data method uses bounded deferred callable allocation while preserving frozen descriptors, stable identity, concrete receiver checks and actual listener registration/removal. Existing clone/import, identity-preserving adoption, reserved-slot connection and shallow native template parking retain actual owners, attributes and children. Work/copy/storage/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Deep native template parking, native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLLIElement now provides the canonical owner-realm interface for exact HTML li elements and exposes its required type DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. List-item, UL and native control type authority remain distinct. The existing Node.contains data method uses bounded deferred callable allocation while preserving frozen descriptors, stable identity, concrete receiver/argument checks and real inclusive descendant queries. Existing ordinary clone/import, identity-preserving adoption, reserved-slot connection and shallow native template parking retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update type-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLLIElement now exposes its required value signed-long accessor over actual native/inert attributes. Reads use allocation-free HTML integer-prefix parsing, returning zero for absent, invalid or out-of-range values without rewriting the source. Assignments perform once-only number conversion and reuse shared 32-bit wrapping before writing the shortest decimal integer through the bounded attribute transaction. Native type and form-control value authority remain distinct. Current Attr identity, exact raw UTF-16 backing, observer old values, independent clone/import state and supported adoption, connection and shallow template parking retain real metadata and owners. Paid attribute-name/payload scans, conversion ordering, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update value-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now provides the canonical owner-realm interface for exact HTML ol elements and exposes its required type DOMString accessor over actual native/inert attributes. Empty defaults, unrestricted strings, exact UTF-16 backing, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Ordered-list, unordered-list, list-item and native control type authority remain distinct. Existing Node.hasChildNodes, getRootNode and isSameNode data methods use bounded deferred callable allocation while retaining frozen descriptors, stable identity, concrete brands and actual tree/identity queries. Existing clone/import, identity-preserving adoption, reserved-slot connection and shallow native template parking retain actual metadata, owners and children. Work/copy/storage/string/node/deadline refusal and recovery preserve namespace, origin and active-content guards. Actual App pointer actions update type-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now exposes its required reversed boolean accessor over actual native/inert attribute presence. Boolean conversion sets an empty null-namespace attribute or removes it without author conversion hooks. Current Attr identity, exact UTF-16 records, observer old values and independent clone/import metadata use the bounded attribute transaction. Exact namespace/case and concrete receiver checks precede access; getters charge attribute-name scanning without string-copy or retained growth. Supported adoption, reserved-slot connection and shallow native template parking retain real owners, attributes and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions update reversed-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now exposes its required start signed-long accessor over actual native/inert attributes. Reads reuse allocation-free HTML integer-prefix parsing and return one for absent, invalid or out-of-range content without rewriting it. Assignments perform once-only number conversion and shared signed 32-bit wrapping before the shortest decimal write through the bounded attribute transaction. The reflected default remains one with reversed attribute presence; actual list numbering remains a separate presentation requirement. Current Attr identity, raw UTF-16 backing, observer old values, independent clone/import state and supported adoption, connection and shallow template parking retain actual metadata and owners. Paid name/payload scans, concrete brands, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update start-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now exposes its legacy compact boolean accessor over actual native/inert attribute presence. Boolean assignment sets an empty null-namespace attribute or removes it without author conversion hooks, preserving current Attr identity, exact UTF-16 backing and observer old values through the bounded transaction. Ordered-list type, start and reversed retain their independent actual attributes. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Getter name scanning needs no string copy or retained growth. Supported clone/import, adoption, reserved-slot connection and shallow native template parking retain real owners, metadata and children. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLPreElement now provides the canonical owner-realm interface for exact HTML pre elements and exposes its legacy width signed-long accessor over actual native/inert attributes. Reads reuse allocation-free HTML integer-prefix parsing with zero defaults for absent, invalid or out-of-range values without rewriting content. Assignments perform once-only number conversion and shared signed 32-bit wrapping through the bounded attribute transaction. PRE width and native image/canvas/control width authority remain distinct. Current Attr identity, raw UTF-16 backing, observer old values and independent clone/import metadata retain actual owners and children through supported adoption, connection and shallow native template parking. Paid name/payload scans, concrete brands, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update width-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native preformatted presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

The listing and xmp HTML local names now share the existing HTMLPreElement owner-realm interface with pre. Exact factories, saved constructors and aliases retain actual local names while using the same canonical width accessor and signed-long conversion. Actual Attr handles, observer old values, independent copies, supported adoption/imports, text descendants and shallow native template parking preserve metadata and owners. Work/copy/storage/string/node/deadline refusal retains real state with fresh recovery. Actual App pointer actions update width-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise both aliases in the built executable. Native preformatted presentation, initial child lifecycle, checked parser coverage and broader HTML interfaces remain required; F01 stays partial.

HTMLUnknownElement now supplies the canonical nonconstructible interface for the fixed historical HTML names applet, bgsound, blink, isindex, keygen, multicol, nextid and spacer. Native and inert factories retain actual names, namespaces and owners while sharing the proper HTMLElement prototype chain. Saved constructors, ordinary inherited attribute handles and observation, concrete brands and captured-origin refusal retain real identity and authority. The interface does not admit unsupported native resource copies. Actual App pointer actions update class-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise all eight names in the built executable. Native presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

The fixed historical HTML names acronym, basefont, big, center, nobr, noembed, noframes, plaintext, rb, rtc, strike and tt now expose HTMLElement directly. Exact native and inert HTML names retain the canonical prototype, inherited actual attributes and namespace identity without acquiring another concrete interface. Saved globals and captured-origin access preserve real authority; author prototypes do not grant native brands. An actual center App fixture verifies pointer-driven class-attribute paint, neighboring editor selection and link hits, while built-executable probes cover all twelve factory names. This interface selection does not implement obsolete presentation, parser recovery or initial child lifecycles; broader foundation acceptance remains required, with F01 partial.

Standard HTML elements whose interface is HTMLElement now expose that canonical prototype directly: abbr, address, article, aside, b, bdi, bdo, cite, code, dd, dfn, dt, em, figcaption, figure, footer, header, hgroup, i, kbd, main, mark, nav, noscript, rp, rt, ruby, s, samp, search, section, small, strong, sub, summary, sup, u, var, wbr. Exact native and inert HTML factories preserve names, namespaces, owners and inherited actual attributes; unrelated concrete interfaces retain their brands. Saved constructors and captured-origin access preserve native authority, and author prototypes grant none. Concrete HTML classification now requires the HTML namespace; an explicit native SVG fixture rejects HTMLFontElement color access before author conversion. SVG factory probes retain SVG identity. Parser namespace admission remains a separate requirement. An actual section App fixture covers pointer-driven class-attribute paint, neighboring editor selection and link hits; built-executable probes cover each factory name. This interface selection does not complete presentation, parsing or child lifecycles for those elements. Broader foundation acceptance remains required; F01 stays partial.

HTML names without an applicable built-in interface now use the canonical nonconstructible HTMLUnknownElement interface. Exact native and inert factories preserve names, namespaces, owners and actual inherited attributes. Valid custom names retain HTMLElement; uppercase HTML-namespace local names and the reserved hyphenated names follow exact-name fallback. Known specialized names with partial interfaces, including frame, frameset and marquee, keep their existing native routing. Foreign namespaces, real brands, saved constructors, captured-origin refusal and unsupported native-copy boundaries remain intact. An actual mystery-element App fixture covers pointer-driven class-attribute paint, neighboring editor selection and link hits; scoped built-executable probes cover fallback and namespace distinctions. Presentation, parsing, child lifecycles and the remaining concrete interfaces are still partial; F01 remains partial.

HTMLModElement now supplies one canonical interface for ins and del, with a dateTime DOMString accessor over their actual datetime attributes. Native and inert values retain exact UTF-16, null-namespace identity, attached Attr aliases, observer old values and once-only conversion. Real brands and captured-origin checks precede author conversion; work, copy, string, retained-storage and deadline refusal preserve state with fresh-job recovery. Ordinary bounded text, clone/import and identity-preserving adoption retain real metadata and child handles. Native App pointer actions for both elements verify attribute-driven paint with preserved fallback text, editor selection and link hits; scoped executable probes cover each alias and the shared interface. Node.normalize uses deferred ordinary-method storage to preserve the existing small-profile child-realm limit and canonical method behavior. The cite URL reflector and its shared URL/document-encoding dependency, initial child lifecycle and broader foundation acceptance remain required; F01 stays partial.

HTMLMapElement now exposes a canonical name DOMString accessor over actual map attributes. Exact native/inert UTF-16 values, attached Attr aliases, null-namespace distinctions, once-only conversion and observer old values use bounded attribute transactions. Real brands and captured-origin checks precede author conversion; refusal preserves state with fresh-job recovery. Ordinary bounded text, clone/import and identity-preserving adoption retain native metadata and child handles. Live collection named lookup observes actual changed map names without replacing lone surrogates. Actual App pointer actions verify attribute-driven paint and preserved fallback text, editor selection and link hits; scoped executable probes cover the built source. Node.cloneNode uses deferred ordinary-method storage to preserve the existing small-profile child-realm limit and canonical method behavior. Image-map association and interaction, initial child lifecycle and broader foundation acceptance remain required; F01 stays partial.

HTMLMapElement.areas now exposes a canonical readonly SameObject HTMLCollection over actual descendant HTML area elements. Native and inert roots preserve tree order and live insertion, movement, removal and named lookup, including exact UTF-16 native IDs and inert names. Namespace filtering excludes foreign and uppercase HTML local names. The same collection follows inert adoption and retained node identities. Canonical descriptors, author overrides and saved constructors preserve real authority; captured-origin checks precede cached access. Charged first-touch filter creation refuses work, copy, string, retained-storage and deadline exhaustion without publishing a collection, while cached reads need no new filter allocation. Actual App pointer actions mutate native area descendants and verify the live collection through attribute-driven paint, preserved text, editor selection and link hits; scoped executable probes cover the built source. Existing unsupported native area-copy and name-write boundaries remain intact. Image-map association, geometry and interaction, native area lifecycle expansion, initial child lifecycles and broader foundation acceptance remain required; F01 stays partial.

HTMLAreaElement now has a canonical nonconstructible interface and alt DOMString accessor over actual native and inert attributes. Exact UTF-16 values, once-only conversion, attached Attr aliases, observer old values and null-namespace distinctions share bounded attribute transactions. Concrete brands and captured-origin checks precede conversion; work, copy, string, retained-storage and deadline refusal preserve state with fresh-job recovery. Inert cloning preserves independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use alt readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Node.insertBefore uses deferred ordinary-method storage to preserve the existing small-profile child-realm limit and canonical mutation behavior. Other area attributes, image-map association, geometry and activation, native lifecycle expansion and initial child lifecycles remain required; F01 stays partial.

HTMLAreaElement.coords now reflects the actual native and inert coords attribute as a plain DOMString, preserving exact UTF-16 units and the original spelling without coordinate parsing or normalization. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use coords readback for adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Other area attributes, coordinate interpretation, image-map association and activation, native lifecycle expansion and initial child lifecycles remain required; F01 stays partial.

HTMLAreaElement.shape now reflects the actual native and inert shape attribute as a plain DOMString. Original spelling, unknown values and exact UTF-16 units remain metadata rather than interpreted image regions. Canonical descriptors, real brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use shape readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Other area attributes, image-map region interpretation, association and activation, native lifecycle expansion and initial child lifecycles remain required; F01 stays partial.

HTMLAreaElement.download now reflects the actual native and inert download attribute as a DOMString, preserving the original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use download readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Download selection, requests, file handling and activation remain separate unfinished consumers, alongside other area attributes, image-map association, native lifecycle expansion and initial child lifecycles; F01 stays partial.

HTMLAreaElement.rel now reflects the actual native and inert rel attribute as a DOMString, preserving original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use rel readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. The relList token list, link-relation processing, other area attributes, image-map association, native lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.ping now reflects native and inert ping metadata through a USVString boundary. Property assignment replaces unpaired surrogates while preserving valid pairs; property readback normalizes raw attribute units without rewriting the underlying Attr or emitting records. Raw attribute APIs retain exact DOMString values. Canonical descriptors, concrete brands, once-only conversion, namespace distinctions, observer old values and captured-origin checks use bounded attribute transactions with atomic refusal and fresh-job recovery. Actual App pointer checks preserve adjacent map paint, fallback text, editor selection and link hits; scoped executable probes cover the built source. This does not implement hyperlink auditing, ping requests, URL-list processing, image-map association, remaining area consumers or native area lifecycle expansion; F01 stays partial.

HTMLAreaElement.relList now exposes a SameObject DOMTokenList over the actual native and inert rel attribute, with independent typed handles and the existing exact token operations. Ordinary PutForwards reads the current author-visible relList before forwarding the original value to its value property; concrete brands and captured-origin checks precede author effects. Attached Attr aliases, exact UTF-16 token identity, live iteration, namespace distinctions, inert clone/adoption isolation, mutation records and bounded refusal/recovery use the existing attribute transactions. Actual App pointer checks preserve adjacent map paint, fallback text, editor selection and link hits; scoped executable probes cover the built source. The supported-token set is empty because area link-relation processing is unfinished: noreferrer, noopener and opener are not advertised before their processing models exist. Inherited native area-instance enumeration remains refused. Image-map association, other area consumers and native area lifecycle expansion remain unfinished; F01 stays partial.

HTMLAreaElement.target now reflects the actual native and inert target attribute as a DOMString, preserving original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy, inherited-enumeration and unrelated raw-attribute refusals remain intact. Actual App pointer actions use target readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Browsing-context selection, area hyperlink activation, image-map association, remaining URL and policy consumers, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.hreflang now reflects the actual native and inert hreflang attribute as a DOMString, preserving original spelling and exact UTF-16 units as language metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy, inherited-enumeration and unrelated raw-attribute refusals remain intact. Actual App pointer actions use hreflang readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Language selection, area hyperlink activation, image-map association, remaining URL and policy consumers, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.type now reflects the actual native and inert type attribute as a DOMString, preserving original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy, inherited-enumeration and unrelated raw-attribute refusals remain intact. Actual App pointer actions use type readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Resource-type processing, area hyperlink activation, image-map association, remaining URL and policy consumers, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.referrerPolicy now reflects the actual native and inert referrerpolicy attribute with ASCII-insensitive known-value matching and canonical lowercase readback. Missing, invalid, whitespace-padded and unpaired-surrogate values read as the empty string while the raw DOMString and attached Attr spelling remain exact. Canonical descriptors, concrete brands, once-only conversion, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions; reading never rewrites the source or emits records. Work, copy, string, retained-storage and deadline refusal preserve state with fresh-job recovery. Inert clones retain independent metadata; existing native area lifecycle and inherited-enumeration refusals remain intact. Actual App pointer actions use policy readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Request policy selection, Referer header processing, area hyperlink activation, image-map association, remaining URL consumers and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.href now exposes a canonical native and inert USVString accessor over actual href attributes using the shared original URL parser and current document base. IDL writes replace unpaired surrogates; raw attribute producers preserve exact DOMString records, while reads serialize the projected URL without changing source spelling or emitting records. Area and Anchor descriptors retain separate concrete brands, captured-origin checks and once-only conversion. Work, copy, string, retained-storage and deadline refusal remain atomic with fresh-job recovery. Inert clones retain independent URL metadata; active-handler copy and initial child lifecycle refusals remain intact. Actual App pointer actions update neighboring map paint from href readback while preserving fallback text, editor selection and link hits; executable probes cover the built source. Hyperlink activation, image-map association, request policy processing and native area lifecycle expansion remain unfinished; F01 stays partial.

HTMLAreaElement URL components now share the original HTTP(S) hyperlink parser and current document base: readonly origin plus protocol, username, password, host, hostname, port, pathname, search and hash. Canonical Area accessors retain concrete brands separate from Anchor descriptors, captured-origin checks and once-only USVString conversion. Component writes serialize into the actual href attribute with stable Attr identity and observer old values; invalid and missing URL setters retain the shared no-op rules; non-HTTP(S) records retain explicit atomic capability refusals. Work, copy, string, retained-storage and deadline refusals remain atomic with fresh-job recovery, and reads do not mutate source records. Native and inert paths share these bounds. Actual App pointer actions update neighboring map paint from component writes while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Non-HTTP(S) URL records, hyperlink activation, image-map association, request policy processing, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement and HTMLAnchorElement now expose canonical native toString methods over their internal href getters. Direct calls, String conversion and ordinary concatenation use current native/inert attribute and base authority without consulting an author href getter. The two methods retain separate concrete brands, per-realm identity and captured-origin checks; ordinary own-method, prototype and Symbol.toPrimitive overrides retain their normal precedence. Work, copy, string and deadline refusals preserve source records with fresh-job recovery, and reads require no retained-storage growth. Actual App pointer actions use stringification readback to update neighboring map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Existing URL parser capability limits remain explicit, including non-HTTP(S) records. Hyperlink activation, image-map association, request policy processing, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.noHref now exposes the legacy boolean reflector over actual native and inert nohref attributes. Reads test exact unnamespaced attribute presence without copying attribute values; truthy writes retain or create the actual empty attribute and falsey writes remove it. Boolean conversion invokes no author string or primitive hooks. Canonical camel-case descriptors, concrete Area brands, current Attr identity, observer old values, raw UTF-16 aliases, exact namespace and captured-origin checks share bounded attribute transactions. Work, copy, storage and deadline refusals preserve state with fresh-job recovery; presence reads need no string-copy or retained-storage growth. Actual App pointer actions update adjacent map paint from boolean readback while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. This obsolete API reflects metadata only. Image-map association, hyperlink activation, request policy processing, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement now supports detached native cloneNode and inert-to-main importNode through the shared bounded copy planner. Copies retain independent attributes, exact supported UTF-16 metadata, live token-list identity, destination-document URL authority and copied nonce-unsafety state. Deep copies preserve actual text/comment topology and owners without connecting the result. Active handlers and unsupported imports retain explicit refusals; existing unrelated raw-attribute guards remain intact. Work, copy, retained-storage, node and deadline refusals publish no partial nodes or source changes and recover with fresh allowances. Saved child-realm methods enforce captured origin before copying. Actual App pointer actions use detached-copy readback to update adjacent map paint while preserving source attributes, fallback text, editor selection and link hits; scoped executable probes cover the built source. This admission does not broaden native area connection/adoption or create image-map association, hyperlink activation or request-policy behavior. Initial child lifecycles and remaining area consumers stay unfinished; F01 remains partial.

HTMLAreaElement now uses reserved native identities for supported author-created inert leaves, inert clones and parsed template leaves. Direct main-document insertion and template-fragment round trips retain the same node, Attr and token-list handles while rebinding the actual owner and resolving href against the current destination. Native detached leaf parking preserves the concrete interface and real Map.areas order on reconnection. Existing active-handler, namespace and resource admission remains checked before activation. Work, copy, storage and deadline refusals preserve trees, owners and observations with fresh-job recovery; saved child-realm methods recheck captured origin. Actual App pointer checks cover create, append, reorder and removal through live collections and adjacent paint, editor selection and link hits. Completed template leaf parking now advances the shared DOM commit generation so ordinary and Area leaves publish genuine child-list observer records. Native main-document author-created connection was already supported and remains a control. Image-map geometry, area activation/navigation, request policy and initial child lifecycles remain separate; F01 stays partial.

HTMLParamElement now exposes a canonical name DOMString accessor over actual parsed, native-created and author-created inert attributes. Exact UTF-16 values, attached Attr aliases, null-namespace distinctions, once-only conversion and observer old values use bounded shared transactions. Genuine brands and captured-origin checks precede author conversion; work, copy, storage and deadline refusals preserve records with fresh-job recovery. Its canonical prototype is registered in each owner realm, while the constructor data identity is allocated on observation through charged native metadata; prototype/global identity, descriptors, mutation and deletion retain ordinary behavior without changing the frozen small-profile limits. Actual App pointer actions cover IDL, Attr and removal changes with adjacent paint, editor selection and link hits, and scoped executable probes cover the built source. Other Param fields, native copy/connection lifecycles and embedded-object behavior remain separate; F01 stays partial.

HTMLHRElement now provides the canonical owner-realm interface for exact HTML hr elements and its legacy align DOMString accessor over actual native/inert attributes. Assignments perform once-only DOMString conversion through the bounded attribute transaction. Exact UTF-16 backing, current Attr identity, observer old values and independent clone/import metadata retain actual owners through supported adoption, connection and shallow native template parking. Parsed void leaves retain null children; metadata does not fabricate separator content or presentation. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update align-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now reflects its legacy color DOMString independently of align over actual native/inert attributes. Arbitrary strings and exact UTF-16 backing are reflected without color parsing or normalization. Once-only conversion, actual Attr identity, observer old values, independent copies and supported adoption, connection and native template parking retain real metadata and owners. Align and its actual Attr handle remain unchanged by color writes or removal. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update color-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now exposes its legacy noShade boolean accessor over actual lowercase noshade attribute presence. Boolean assignments create an empty null-namespace attribute or remove it without invoking author conversion hooks; the camel-case property remains distinct from lowercase author keys and differently cased inert attributes. Current Attr identity, raw UTF-16 backing, observer old values, independent copies and supported adoption, connection and native template parking retain actual owners. Align, color and their actual Attr handles remain unchanged by noShade writes and removal. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update noshade-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now exposes its legacy size DOMString accessor over actual native/inert attributes. Arbitrary strings and exact UTF-16 backing reflect without numeric parsing, clamping or normalization; numeric assignment uses ordinary string conversion. Once-only conversion, current Attr identity, observer old values, independent copies and supported adoption, connection and native template parking retain actual owners. Align, color, noShade and their actual Attr handles remain independent of size writes or removal, including retained author effects during conversion. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update size-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now exposes its legacy width DOMString accessor over actual native/inert attributes. Arbitrary strings and exact UTF-16 backing reflect without numeric parsing, clamping or normalization; numeric assignment uses ordinary string conversion. Once-only conversion, current Attr identity, observer old values, independent copies and supported adoption, connection and native template parking retain actual owners. Align, color, noShade and size keep independent actual Attr handles across width writes and removal. Borrowed accessors reject image, canvas, input, select and other wrong brands before conversion; exact namespace/case, captured origin and resource refusal preserve state with fresh recovery. Actual App pointer actions update width-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLBRElement now provides the canonical owner-realm interface for exact HTML br elements and its legacy clear DOMString accessor over actual native/inert attributes. Once-only string conversion, exact UTF-16 backing, current Attr identity, observer old values and independent supported copies, adoption, connection and shallow native template parking retain actual owners. Parsed void leaves keep null children; metadata does not manufacture line-break contents. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update clear-attribute-driven adjacent CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Node equality and document-position methods use bounded deferred data slots while retaining native identity and data descriptors. Legacy clear presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLMenuElement now provides the canonical owner-realm interface for exact HTML menu elements and its legacy compact boolean accessor over actual native/inert attribute presence. Boolean assignment sets an empty null-namespace attribute or removes it without author conversion hooks. Exact UTF-16 backing, current Attr identity, observer old values and independent supported copies, adoption, connection and shallow native template parking retain actual owners and children. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Getter scanning needs no string copy or retained growth. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, neighboring editor selection and link hits; scoped production probes exercise the built executable. Native menu presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLDirectoryElement now provides the canonical owner-realm interface for exact HTML dir elements and its legacy compact boolean accessor over actual native/inert attribute presence. Boolean assignment sets an empty null-namespace attribute or removes it without author conversion hooks. Exact UTF-16 backing, current Attr identity, observer old values and independent supported copies, adoption, connection and shallow native template parking retain actual owners and children. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Getter scanning needs no string copy or retained growth. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, neighboring editor selection and link hits; scoped production probes exercise the built executable. Node namespace-query methods use bounded deferred data slots while retaining native identities, data descriptors and real namespace contexts. Native directory/list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLFontElement now provides its canonical interface and legacy color reflection over actual native/inert attributes. The LegacyNullToEmptyString setter maps original null to an empty string while undefined and object conversion retain ordinary DOMString behavior. Exact UTF-16 backing, once-only conversion, concrete brands, captured origin, actual Attr identity and observer old values remain bounded. Real text descendants, independent deep copies, supported imports/adoption and shallow native template parking preserve actual owners and metadata. Work/copy/storage/string/node/deadline refusal retains state with fresh recovery. Actual App pointer actions update attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native presentational hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLFontElement now reflects its legacy face DOMString independently of color over actual native/inert attributes. Null and undefined use ordinary string conversion; the separate color member keeps its legacy null-to-empty behavior. Once-only conversion, exact UTF-16 backing, actual Attr handles and observer old values, concrete brands, captured origin and bounded refusal/recovery retain actual state. Real text descendants, independent copies, supported imports/adoption and shallow native template parking preserve owners and metadata. Actual App pointer actions update face-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native font presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLFontElement now reflects its legacy size DOMString independently of color and face over actual native/inert attributes. Size preserves unrestricted strings and ordinary null/undefined conversion rather than applying integer parsing or clamping. Once-only conversion, exact UTF-16 backing, actual Attr handles and observer old values, concrete brands, captured origin and bounded refusal/recovery retain real state. Real text descendants, independent copies, supported imports/adoption and shallow native template parking preserve owners and metadata. Actual App pointer actions update size-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native font presentational hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.


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

**Named-reference increment:** the generated named-reference table covers all 2,231
pinned spellings, including two-scalar values and legacy attribute ambiguity.
The fixed supplementary Linux native catalog passes all 13,699 cases, with all
13 new parser regressions and 58 existing HTML tests passing. The
[coverage register](HTML_FOUNDATION_COVERAGE.json) records the comparison and its
scope. Full tokenizer diagnostics, streaming, surrounding parser resource and
cancellation accounting, Windows App integration and the required frozen corpus
gates remain open; this increment does not complete F04.

**Script-data and PLAINTEXT increment:** the original
[native script scanner](../src/html_script_data.rs) handles escaped and
double-escaped states, appropriate end tags and unfinished end-tag EOF. Native HTML SCRIPT
and STYLE retain literal references, replace source nulls and use fallible text
copying. Permissive closing tags recover an initial `=` without ending at a quoted
`>`. Loaded and inert HTML PLAINTEXT consumes through EOF with native character
insertion, paragraph closure and formatting reconstruction, while retaining
foreign parentage.

The fixed supplementary comparison improves from 76/110 to 110/110 retained-tree
matches: 52 unchanged WPT trees (all 26 script-data sources in loaded and inert
paths), 44 adapted script controls, six PLAINTEXT controls and eight RAWTEXT
controls. All 20 frozen new native tests pass, up from eight, and all 71 existing
HTML tests remain passing. Three additional review checks also pass: the initial
`=` regression has a separate baseline failure; two boundary checks have candidate
evidence only. These 94 native checks and tree comparisons qualify this source
increment, not complete upstream cases with unavailable diagnostics. The frozen
inputs, selections and witnesses are unchanged. Streaming, whole-parser
allocation/cancellation, Windows App and required browser/corpus acceptance
remain open; F04 remains partial.

**Comment recovery increment:** unrecognized declarations and invalid end-tag
openings now retain real bogus Comment nodes in document, inert-document and
supported checked-fragment entries. HTML-context CDATA declarations become
literal comments. Empty `</>` emits nothing without disturbing a pending initial
newline; `</` at EOF remains text. Normal and bogus scans, exact NUL-expanded
sizing and fallible copying support cancellation. Existing table/formatting,
document-relative placement, processing instructions and SVG provenance remain.

All 24 frozen new native checks pass, up from three; all 94 existing HTML checks
pass with two obsolete declaration refusals replaced by exact comment assertions.
Four separately identified scan/copy checks also pass. The fixed supplementary
tree comparison improves from 424/562 to 560/562, preserving all prior matches.
All 26 unchanged WPT comment trees remain matching; the separate adapted
Comment/Character cohort improves from 398/536 to 534/536. At that checkpoint,
the remaining loaded and inert pair retained a literal in-body NUL Text node
after a correctly parsed comment. The DATA character increment below resolves
that pair without changing its expected output or place in the selection.

At that checkpoint, checked DOCTYPE/generic NUL and foreign CDATA boundaries
remained. The later F07 increment below removes the checked DOCTYPE refusal. These are native
component results, not complete upstream cases with diagnostics or Windows App,
browser, streaming or full-corpus acceptance. F04 and its tokenizer contract
remain partial.


**DATA character increment:** loaded and inert HTML documents now apply the
supported structural and table transitions before ignoring literal DATA NUL.
Initial NUL still ends the doctype opportunity; head/after-head and document-tail
reprocessing puts subsequent comments and whitespace in their correct parents.
Column groups retain their original leading whitespace before reprocessing the
NUL. Pending table characters ignore NUL before nonspace classification, so
whitespace does not become fostered text. Ignored characters create neither
empty Text nodes nor reconstructed formatting elements. Later retained text
still reconstructs normally, including whitespace delegated from document-tail
modes without changing those modes prematurely.

The filter runs after tokenization and character-reference decoding. It cannot
turn malformed text into a tag or decode a reference twice; numeric `&#0;` and
raw-text/comment replacement characters remain distinct. Counting and fallible
exact-size copying poll cancellation, and text without NUL keeps its allocation.
All 25 frozen new native checks pass, up from two, and all 122 existing HTML
checks remain passing. Six separately identified helper/review checks pass.
The unchanged 562-case supplementary comparison improves from 560 to 562 matching
trees: 26/26 unchanged WPT comment comparisons and 536/536 separately adapted
Comment/Character comparisons. Every previous match and every expected tree is
preserved. These are retained-tree/component results, not complete tokenizer or
upstream-case acceptance.

Checked-fragment generic NUL guards remain. Legacy internal parsing,
scripting-enabled and nested-template noscript, unknown foreign/uncertain
recovery, full insertion modes,
diagnostics, streaming, whole-parser resource accounting and Windows App/browser/
full-corpus qualification remain open. The frozen foundation inputs, selections
and witnesses are unchanged; F04 and F05 remain partial.

**Tag and attribute increment:** the shared scanner now retains complete tag
names through the actual delimiter, folds ASCII case only, and replaces literal
NUL in tag names, attribute names and all attribute-value states. An initial `=`
belongs to the attribute name; quoted delimiters, slash, reconsume and EOF states
retain their real token boundaries. Malformed resource-like names no longer
alias native resource elements. Duplicate names compare after normalization,
retain the first value and keep the duplicate evidence used by nonce checks.

Checked STYLE end-tag recovery consumes its complete attribute tail without
publishing those attributes. Strict TEXTAREA/SVG syntax and the generic checked
fragment NUL guard remain separate. Scans, exact-size normalization and copies
poll cancellation; token storage uses fallible reservation.

All 23 frozen new native checks pass, up from eight, and all 180 existing HTML
checks remain passing, with the obsolete STYLE refusal replaced by positive
tree assertions. Seven separately selected optimized native parser scenarios
pass, up from one; the raw-text vertical-tab delimiter scenario was already a
passing guard. These are source-bound Linux component results. Structured
diagnostics, streaming, whole-parser admission, Windows App/JavaScript execution
and full browser/corpus qualification remain open. F04 and its tokenizer
contract stay partial; frozen foundation inputs and selections are unchanged.

The composed tag/declaration source passes all 214 selected native HTML checks.
One additional native assertion group verifies eight formerly refused spellings
and shares its exact assertions with the innerHTML fixture. That supplementary
group is separate from the frozen new-test selections; the Page portion of the
consumer fixture is not included in this component execution.

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

**Head noscript increment:** loaded and inert HTML documents now use an explicit
scripting-disabled head-noscript insertion mode. Allowed whitespace, comments,
processing instructions and metadata retain the noscript parent. A duplicate
html start tag merges missing root attributes; nested head/noscript starts and
unrelated end tags are ignored. The noscript end tag returns to head. Other
starts, non-whitespace characters and EOF reprocess through the appropriate
head/body path; the exceptional br end tag produces the attribute-free body br.
Initial whitespace stays inside noscript before later characters move to body,
and a literal NUL triggers that transition before the body DATA filter drops it.
Raw-text processing and duplicate-nonce provenance remain intact.

All 26 original focused checks pass, up from nine on the unchanged production
baseline, with all 153 existing native HTML checks still passing. The fixed
562 supplementary retained-tree comparisons remain matching with unchanged
expectations. These are Linux native parser/component results. Scripting-enabled
noscript tokenization, checked-fragment expansion, complete template/foreign
insertion modes, tokenizer diagnostics and Windows App/browser/full-corpus
acceptance remain separate. F05 and mode M05 remain partial.

**Paragraph block-start increment:** the fourteen previously missing ordinary
block names now close an in-scope HTML paragraph before insertion: `address`,
`aside`, `details`, `dialog`, `dir`, `dl`, `fieldset`, `figcaption`, `figure`,
`hgroup`, `menu`, `nav`, `search` and `summary`. The paragraph lookup uses native
HTML button-scope boundaries, checks the target before an older boundary and
preserves synthetic fragment roots. Unknown foreign integration is handled
conservatively. Active formatting reconstructs with distinct node identities
after the paragraph closes.

All 13 frozen new native checks pass, up from three, with all 180 existing HTML
controls remaining passing. Six separately selected optimized native parser
scenarios pass, up from one. The checks cover actual trees and attributes, scope,
formatting, capacity refusal, cancellation and recovery. Page/Windows/JavaScript
execution, full foreign/mode/browser acceptance, definition-list and heading
recovery, form/xmp/listing behavior and table quirks-mode rules remain separate.
F05 stays partial. Composed with the table-leaf increment and existing
tag/declaration source, all 242 selected native HTML checks pass with unchanged
expected outcomes.

**Definition and list-item start increment:** `dd`/`dt` starts scan backward for
an open definition item; `li` starts scan for an open list item. The matching
target is checked before special-element barriers, with `address`, `div` and `p`
as exceptions. The existing 83-name special category is unchanged. Paragraph
closure follows this scan; fragment roots, template and conservative foreign
boundaries, original children and formatting identities remain intact.

All 13 frozen DD/DT checks pass, up from one, and five separately counted LI
checks pass, up from two. All 242 existing HTML controls remain passing, for
260 native checks total. Six separately selected optimized native parser
scenarios pass, up from one. Old definition refusals have exact positive native
sibling/text assertions. Added connected/detached innerHTML assertions are not
executed by this native selection and are not counted as Page evidence.
Heading starts, explicit scoped end handling, full foreign/mode behavior and
Windows/browser/corpus acceptance remain separate. F05 stays partial.

**Heading-start increment:** after paragraph closure, a new `h1`–`h6` pops
exactly one current native HTML heading before insertion. A heading behind
another current node remains open, and the synthetic fragment root is retained.

All 13 frozen new native cases pass, up from one; all 242 existing HTML controls
remain passing on the component source. The cases cover all 36 rank combinations,
current-node boundaries, paragraph ordering, formatting, template/root and native
namespace guards, token completion, capacity, cancellation and resource metadata.
All six separately selected optimized native scenarios pass, after all six
reproduced missing behavior in the baseline. Two obsolete heading-refusal
assertions now verify positive sibling/name/text results; the Page-contained
assertion is not included in native execution. General heading end recovery and
full foreign/mode/Windows/browser acceptance remain open. F05 stays partial.

With definition/list starts and heading starts composed, all 273 selected native
HTML checks pass with the frozen expected outcomes unchanged. This combined
result covers the 242 prior controls, 18 definition/list checks and 13 heading
checks; it does not add Page or Windows execution evidence.

**Scoped list and heading end increment:** native LI ends use list-item scope,
including UL/OL boundaries. DD/DT ends require the exact requested name in
ordinary scope. Any H1-H6 end closes the nearest in-scope HTML heading regardless
of rank. Targets precede barriers; synthetic roots are excluded. Recognized ends
with missing or out-of-scope targets are ignored without a generic cross-boundary
close. The existing ordinary-scope predicate is reused unchanged.

Dispatch follows checked table/select restrictions and strict template SVG end
validation. Existing foreign-current fallback is retained; broader foreign
integration is not implied. The stack scan allocates no nodes and preserves
children and active formatting identities under existing depth and node limits.

All 24 frozen new parser checks pass, up from eight, plus two unchanged existing
scope-predicate controls. All 273 prior native HTML checks remain passing, for
299 total. Seven optimized native scenarios pass, up from one. Checks cover all
36 heading-rank pairs, exact scopes and tree identities, token completion,
formatting, capacity, cancellation and recovery. Frozen new expectations remain
unchanged. Added connected/detached Page assertions are not executed by this
native selection. Paragraph/BR synthesis, broader end rules and Windows/browser/
corpus acceptance remain separate; F05 stays partial.

**Paragraph/BR end recovery increment:** in the supported native body path, a
P end closes an in-button-scope paragraph or inserts and immediately closes a
real attribute-free P. A BR end follows ordinary attribute-free void BR
insertion. End-token id/href/attributes and duplicate-attribute evidence never
transfer to recovered nodes. Existing insertion, formatting, table placement,
node/depth admission and cancellation remain authoritative; empty paragraph
synthesis does not reconstruct formatting.

A bounded initial-template gate retains the required ignore behavior for these
ends before a template leaves its initial state. Text and markup leaves do not
change that state. The exact ten head-metadata starts retain it, including title;
other original starts exit it before structure/table helpers can consume them.
Void starts retain that transition, and nested template identities restore their
outer state on closure. Storage is fallible and bounded by the existing depth
limit. Exited-initial state does not imply a complete later-template mode.

All 23 frozen new native checks pass, up from three, with all 299 existing HTML
checks passing, for 322 total. All eight optimized native scenarios pass after
reproducing missing behavior in the baseline. Cases cover actual trees, scope,
metadata and consumed-token transitions, nested/root template state, formatting,
table placement, exact capacities and cancellation/recovery. Four older refusal
fixtures now have narrow positive parser assertions with other guards retained.
Direct parser assertions inside a mixed Page test are not executed by this
selection and do not establish Page behavior. Later template modes and full
foreign/mode/Windows/browser acceptance remain separate; F05 stays partial.

**Ordinary block-end increment:** the shared scoped-end helper recognizes the
exact 28-name in-body group, including button and select. An exact native HTML
target must be in ordinary scope and is tested before barriers; missing or
blocked targets are ignored. Only recognition changes. The existing scan, root
floor, LI extras, strict dispatch and generic form/table/foreign/marker behavior
remain intact.

All 19 frozen original cases and two separately frozen disabled-noscript/table
cases pass, up from seven of 21. All 322 prior native HTML checks remain passing,
for 343 total. Eight optimized native scenarios pass, up from three. The
supplement verifies disabled-noscript traversal and following fostered text,
alongside the actual table boundary and raw-iframe guards. Generic table fences
and modes remain unchanged. Original expected outcomes remain unchanged.
Added connected/detached Page assertions are not executed by the native
selection. Checked listing/select restrictions, enabled-noscript behavior and
broader form/marker/template/foreign/mode/Windows/browser requirements remain
separate; F05 stays partial.

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

**Fragment DOCTYPE increment:** supported checked HTML fragments now consume and
ignore DOCTYPE tokens without adding a DocumentType node or changing the fragment
mode. The shared declaration reader retains malformed-token and EOF boundaries.
Text, comment, select and supported template SVG ownership remain intact, while
raw/RCDATA contexts keep declaration spelling as text. Full documents retain their
separate initial DOCTYPE identity and mode behavior.

All 11 frozen new native parser checks pass, up from three; the same 64 existing
selected controls remain passing, for 75 affected checks. Four separately selected
optimized native parser scenarios pass, up from one. These are source-bound Linux
component results. Complete mode inheritance, unsupported contexts, markup/API
execution, serialization, Windows App/browser/corpus acceptance and XML/XHTML
remain separate. Checked generic NUL, unsupported structure and foreign-content
refusals remain. F07 and its contextual-fragment contract stay partial.

**Table leaf increment:** supported ordinary-flow fragments retain real table
children for decoded ASCII whitespace, comments and processing instructions.
Ignored DOCTYPE and empty end tokens allocate no node or split adjacent Text.
The shared token loop preserves source order and requires a clean explicit table
close. Active formatting stays out of table whitespace and resumes for later
ordinary text. Node/depth admission and bounded classification cancellation remain
part of this path.

All 14 frozen new native checks pass, up from four; all 191 existing HTML checks
on the component source remain passing, for 205 total. Five separately selected
optimized native parser scenarios pass, up from one. Connected/detached innerHTML
ownership assertions are added, but their Page execution is not part of these
measurements. Element children, non-whitespace and foster parenting, implicit
endings, nested/table-context construction and broader namespace/mode/API/browser
qualification remain open. F07 stays partial.

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
Its broader CSS work is mapped in the [CSS plan](CSS_FOUNDATION_PLAN.md) and
[coverage register](CSS_FOUNDATION_COVERAGE.json).

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

After this gate, activate C00 in the [CSS plan](CSS_FOUNDATION_PLAN.md): reconcile
new source progress, lock the selected standards and tests, and begin the next
remaining shared syntax/property contract. Preserve the saved YouTube milestones
as product consumers; unrelated CSS extensions do not become blanket site gates.

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

The [CSS foundation plan](CSS_FOUNDATION_PLAN.md),
[coverage register](CSS_FOUNDATION_COVERAGE.json) and
[standards inventory](CSS_STANDARDS_INVENTORY.json) supply the post-F14 CSS program.
They keep existing partial implementation, required integration dependencies,
specialized renderers and forward drafts distinct. No CSS completion claim
follows from HTML foundation closure.

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
