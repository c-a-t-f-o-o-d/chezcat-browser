<div align="center">

# Capability and dependency matrix

*Implemented behavior, explicit limits, and the evidence behind each claim.*

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Documentation](README.md) · [Compatibility](COMPATIBILITY.md)

</div>

---

## Read the matrix correctly

> [!IMPORTANT]
> **Every broad platform area remains partial.** “Implemented” describes a finite
> contract, not complete standards or website support. Local tests, App journeys,
> physical devices, and live services are different evidence scopes.

**Published prerelease 0.82.0-alpha.1** is tagged at `f2e1823` and includes the integrated features measured through `6089546`. The preceding prerelease is 0.81.0-alpha.3 at `57cc1d5`. [Release downloads](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.1) · [Source comparison](https://github.com/c-a-t-f-o-o-d/chezcat-browser/compare/v0.81.0-alpha.3...v0.82.0-alpha.1)

**Earlier source snapshot:** public `main` at `a65d37c` (November 17, 2026). This includes
PRs [#29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29),
[#30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30),
[#31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31),
[#32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32), and
[#34](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/34).
The fixed [alpha.2 downloads](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2)
remain at `36adf20`; these later changes do not update those binaries.

[Browser & platform](#browser-and-platform) · [DOM & JavaScript](#dom-and-javascript)
· [Rendering](#rendering-and-resources) · [Media](#media)
· [Evidence](#evidence-boundaries) · [Implementation map](#implementation-map)

## Browser and platform

| Capability | Implemented slice | Important limits |
| :--- | :--- | :--- |
| **Async functions and arrows** | Calls return a fresh Promise. Direct `await` statements, `return await`, concise arrows and single-binding initializers suspend and resume through the Promise job queue. Blocks, conditional arms and try bodies with synchronous catch handlers retain local variables, receiver identity and rejection reasons. | Other await expression positions, loops containing await, suspending catch handlers, and try/finally containing await are outside this slice. Dynamic compilation uses the existing script policy and resource limits. |
| **Async iterator loops** | Async functions can use `for await…of` with a lexical binding and an authored `Symbol.asyncIterator`. They await iterator results and early closes through the same Promise jobs used by other async code. | Synchronous iterable fallback, async generator syntax and other binding forms remain outside this slice. |
| **Native shell** | Tabs/windows, tab-pinned inspectors, guarded primary/middle links and foreground/background/new-window context actions | Named contexts, script-opened windows, wider accessibility, and locked-desktop presentation remain gaps |
| **Navigation** | HTTP(S)/files, explicit URL/file intent, encoded address searches, Google/Bing/DuckDuckGo/None providers, local result/destination/one-Back/second-query journeys | Live outcomes are response/session-dependent; failed navigation is distinct from the retained page |
| **Profile and storage** | Settings, bookmarks, optional history, staged downloads, guarded local roots, memory-only cookies | No general site storage, service workers, or persistent cookies; local files are not encrypted vaults |
| **About / identity** | Full descriptive version/source/state/kind, offline license access, allowlisted Copy data | Metadata is not signing/authentication; App copy capture alone does not prove OS clipboard delivery |
| **License acceptance** | Full offline terms, explicit personal/organization acceptance, strict minimal external receipts, same-text reuse, changed-terms/decline/write-failure refusal | Records a local choice—not verified identity, organizational authority, or an online account |
| **Build and delivery** | Zero third-party production Cargo dependencies, pinned tools, generated resources, portable ZIP and offline gates | Installer and update discovery/upgrade remain planned; releases require owner requests |
| **Isolation** | Checked ownership, worker retirement/cancellation, explicit FFI boundaries and watchdogs | One process; no OS renderer sandbox or whole-process memory sandbox |

## DOM and JavaScript

| Capability | Implemented slice | Important limits |
| :--- | :--- | :--- |
| **String reverse search** | `String.prototype.lastIndexOf` finds the last matching substring at or before the given start position. Positions and results count UTF-16 code units, including both units of supplementary characters. | Receiver and argument conversions follow the existing native access checks. Search runs within the script's step and string-work budgets. |
| **Number fixed-point text** | `Number.prototype.toFixed` accepts 0–100 fraction digits on numbers and boxed numbers. It rounds the exact binary value, including ties and negative values that round to zero. | Magnitudes of 10²¹ or greater use the existing shortest decimal representation. String size, copy work and instruction limits apply. |
| **Function frame properties** | The fixed-point formatter has no own `caller` or `arguments` property. | The inherited throwing accessors for these names on `Function.prototype` remain unsupported. |
| **JavaScript identifiers** | Bindings, references and property names accept Unicode 17 identifier characters and valid ECMAScript Unicode escapes. Equivalent literal and escaped spellings address the same property. | Names retain their code points without normalization. Reserved names can appear after a dot and existing property-size limits apply to the decoded spelling. |
| **Attribute removal** | `removeAttribute` on a known HTML or SVG element returns immediately if the named attribute is absent. HTML names use ASCII case folding, SVG names are exact, and `removeAttributeNS` uses namespace and local name. | A present attribute still goes through the element's checked native mutation path. |
| **WeakMap** | The constructor accepts supported iterable pairs. `get`, `set`, `has` and `delete` use object, function or unregistered symbol identity. Native object keys retain their document access checks. | During callable collection, an entry retains its value while both the map and its key are reachable. Native and symbol identities may remain live for the document. The existing per-collection and document entry limits apply. |
| **WeakSet** | The constructor accepts a supported iterable. `add`, `has` and `delete` track object, function and unregistered symbol identity. Each realm supplies its own constructor and prototype. | Membership alone does not keep a callable alive. Native and symbol identities may remain live for the document. The same entry limits and native access checks apply as for WeakMap. |
| **Reflect.get** | Each realm supplies an ordinary `Reflect` namespace. Native `get(target, key, receiver)` converts string and symbol keys and preserves inherited accessor receivers, native getter brands and work limits. | Native targets support an omitted or identical receiver through existing access checks. Canonical prototypes can invoke genuine branded getters with a supported native receiver. |
| **Reflect.set** | Native `set(target, key, value, receiver)` returns the Boolean assignment result for ordinary object and function properties. String and symbol writes preserve receiver identity, descriptor flags and setter exceptions. | Native targets, native data receivers, arrays, typed arrays, boxed strings, arguments and regular expression targets or data receivers remain outside this assignment slice. |
| **Proxy** | `new Proxy(target, handler)` supports ordinary non-callable objects. Property reads, writes, `in` and prototype queries resolve live handler methods, forward absent traps and check target invariants. When the handler's `getOwnPropertyDescriptor` is null or undefined, own-property checks and descriptor reads use the target's string and Symbol properties. A proxy has its own identity as a WeakMap key. | Callable, native, array and nested proxy endpoints, revocation, callable `getOwnPropertyDescriptor` traps, own-key enumeration, public property definition, deletion and integrity changes remain outside this slice. |
| **DOM class inheritance** | A script class can extend the canonical Node, Element, SVGElement, HTMLElement, HTMLBodyElement, HTMLButtonElement, HTMLDivElement, HTMLHtmlElement and HTMLUListElement interfaces. Its constructor and prototype inherit from the selected realm’s actual interface objects. | Calling `super()` for an unregistered HTML element or for SVGElement throws the interface’s TypeError. Custom element registration and other native class bases remain outside this slice. |
| **Computed class method names** | Public instance and static methods, getters and setters accept computed string and symbol keys. Keys evaluate once in class order, and the resulting method descriptors are nonenumerable. | Computed fields, private elements and suspension inside a computed key remain outside this slice. |
| **SVG element identity** | Balanced inline SVG containing supported groups and basic shapes exposes the SVG namespace, case-preserving element names and `SVGElement` ancestry. HTML elements with the same local name keep their HTML identity. | The parser recognizes a bounded set of inert markup. Resource-bearing or ambiguous foreign content retains unknown namespace provenance. SVG-specific element interfaces and programmatic SVG creation remain incomplete. |
| **DOM identity and collections** | Branded `Node.prototype` getters expose native parent, child and sibling relationships and stable live `childNodes` lists for supported nodes. The same prototype exposes the checked append, insert, remove and replace methods. Checked native array-like apply/slice, Attr identity, text replacement and main-document Comment construction are supported. The CDATASection interface inherits Text and HTML documents reject CDATA creation. The native ProcessingInstruction constructor and HTML Document factory expose retained target and data. | ProcessingInstruction pseudo-attribute APIs, XML processing, parsed comments, full CharacterData, main-document doctype representation and child-frame collections remain outside this scope. |
| **Ordinary HTML properties** | Verified main-document HTML elements, including parsed iframe wrappers, use ordinary string and symbol properties with native prototypes and descriptors. Author-defined members can fill absent shadow-tree and newer markup API names. | Form named lookup, select indices, iframe navigation and child-context access, and other native members retain their own routing. |
| **Mutation observation** | MutationObserver batches native CharacterData writes and checked append/insert/remove/replace operations. Child-list records carry variant, sibling and node snapshots across source and destination, and subtree registrations follow current main-document ancestry. Checked setAttribute and removeAttribute, including namespace-aware forms, produce filtered attribute records and optional old values. | Reflected property writes, whole-content replacement and transient registrations after removal remain outside this scope. |
| **Tree traversal** | `Document.createTreeWalker` and `NodeFilter` masks traverse live main-document Nodes with forward and backward movement and a writable cursor. | The traversal subset accepts the default null filter. Callback filters and child-realm traversal remain pending. |
| **Document readiness** | The resource-completion transition to complete fires a native document readystatechange before window load. DOMContentLoaded still runs at interactive. `Document.currentScript` exposes the genuine element during classic-script execution and its immediate microtask checkpoint. | Initial classic scripts run after parsing in this pipeline. |
| **Window interface** | The displayed document and admitted child Windows have their own Window interface and EventTarget ancestry. Same-origin code may inspect the canonical prototype. | The named-properties object currently provides prototype ancestry and its own-property rules. Named element and frame discovery are still outside this scope. |
| **HTML template contents** | A real HTMLTemplateElement exposes a stable, inert DocumentFragment. The fragment and parsed descendants use a separate HTML Document owner. Native traversal and childNodes preserve nested template boundaries. Shallow and deep clones retain the inert owner. Checked template innerHTML works on inert content and retains the fragment identity on replacement. Supported ordinary HTML content can be imported as detached displayed-document nodes. | Direct fragment mutation, adoption and import of protected resources or native controls remain outside this scope. |
| **Inert HTML documents** | `document.implementation.createHTMLDocument` creates an independent HTML tree with a doctype, head, body, and optional literal title. Basic element and text factories preserve its node ownership. | Inert documents have no Window, renderer, or resource loader. Tree mutation and adoption into the displayed document remain pending. |
| **Document interface** | The main document and accessible active blank-frame documents inherit their own realm’s `Document.prototype` and Node ancestry. `HTMLDocument` is an alias of that realm’s interface. The `documentElement` getter reads the receiving document’s actual root element. | Creating an independent document with `new Document()` remains unsupported. Blank-frame writes and origin checks retain their existing limits. |
| **Node roots** | Genuine `Node.getRootNode()` for ordinary accessible connected/detached trees, Attr and supported same-origin child trees; ordered options conversion and owner/work checks | No Shadow DOM or fabricated child scope |
| **Node text content** | The realm's `Node.prototype.textContent` descriptor can be inspected and wrapped. Saved accessors use the receiver's native node and the existing checked mutation path. | Existing replacement limits and blank-frame write restrictions apply. |
| **Node cloning** | `cloneNode()` makes a detached copy in the main document. Passing a truthy argument copies children in order. Ordinary HTML containers retain their attributes. Text, Comment and DocumentFragment retain their node kinds. | The original ordinary-node slice excludes native controls, resources, handlers, foreign elements and child documents. The later textarea and radio clone behavior is described below. |
| **Element markup** | Reading `outerHTML` on supported main-document HTML elements serializes the element, its ordered attributes and its current children. Void elements serialize their start tag. | Template contents, foreign markup and `outerHTML` replacement are outside this reader. |
| **Native INPUT and IFRAME interfaces** | Parsed and created inputs expose the real `HTMLInputElement` prototype and saved value, checked and defaultChecked accessors. Accessible HTML iframe elements expose their owner realm’s `HTMLIFrameElement` identity. | Form operations retain their control-specific limits. Frame access follows the existing origin and activation checks. Parsed inactive frames keep their restricted instance storage. |
| **Canvas dimensions** | Genuine HTML canvases expose realm-owned `HTMLCanvasElement` width/height accessors, unsigned-long conversion, raw attribute/Attr reflection and live natural dimensions. A scripted canvas occupies a transparent replaced box until something is drawn. Supported CSS sizing preserves its natural ratio before finite layout caps. | Native construction is illegal. Attribute, access and work limits apply. With scripting disabled, canvas fallback content remains visible. |
| **Canvas 2D rectangles** | `getContext('2d')` returns a stable context for a supported canvas. `fillRect` paints with its solid `fillStyle`, initially black. `clearRect` reveals the background. Setting either dimension clears the pixels and restores black on the same context. | Each transparent sRGB backing stores up to 4,194,304 pixels, with a maximum side of 16,384 pixels and a shared 64 MiB document allowance. Other drawing state, transforms, paths and encoded image export remain open. |
| **Canvas fill colors** | The context reads and writes supported named colors, hexadecimal RGB/RGBA, `rgb()`, `rgba()` and `transparent`. It resolves `currentColor` from the canvas foreground at assignment time. Invalid colors preserve the previous value. | The parser accepts up to 2,048 ASCII bytes and stores eight bits per channel. Gradients, patterns, additional color spaces and the full CSS color grammar remain open. |
| **Canvas pixel readback** | `getImageData` copies the requested rectangle into an independent `ImageData` with a mutable `Uint8ClampedArray`. The array holds straight RGBA bytes in row order. Out-of-bounds pixels are transparent black. `new ImageData` accepts dimensions or a compatible clamped view. | Readback covers the canvas's own sRGB RGBA8 backing and uses the shared retained-storage admission. Image imports, float16 readback and writing an ImageData back to a canvas remain open. |
| **Native controls in fragments and clones** | Conventional checkbox and textarea fragments create real controls. Fresh inputs can become checkboxes or radios. Textarea clones preserve live text independently of their deep or shallow defaults. Radio clones copy current checkedness and join the appropriate group on connection. | Active control type transitions, explicit form association and protected subtrees retain their existing admission rules. |
| **Node containment** | Integrated [PR #76](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/76) adds genuine inclusive `Node.contains`, physical ancestry, Attr distinction and checked receiver/argument ownership. | Ordinary trees and supported same-origin child Nodes only; no shadow-tree or cross-origin access grant. |
| **DOM object stringification** | BUTTON, UL, DIV, BODY and HTML inherit `Object.prototype.toString` through their own interfaces. MAIN uses `HTMLElement.prototype`. [PR #64](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/64) · [PR #68](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/68) | In supported child documents, HTML and BODY use prototypes from their own realm. |
| **Unregistered custom-element names** | HTML elements with an admitted autonomous custom-element name inherit their owning realm’s `HTMLElement.prototype`. A parsed or script-created node retains its own properties, text and identity when inserted. | A custom-element registry, upgrades and lifecycle callbacks are separate capabilities. |
| **Element selectors, siblings, Slottable and NodeLists** | Scripts can borrow Element selectors. `querySelectorAll` returns a static snapshot. Element/CharacterData sibling getters skip Text/Comment nodes. [PR #67](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/67) adds branded Element/Text `assignedSlot` getters returning null in supported ordinary trees. Main-document `HTMLSlotElement.name` reflects the checked string attribute. Main-document NodeLists have bounded indexed iteration. | Existing grammar, work and document access checks apply. Shadow DOM, child Element queries and complete NodeList APIs remain outside this scope. |
| **Select fragments** | Conventional select/option/optgroup fragments; whole-subtree removal/reconnection retains identities, edited values, owner/order/disabled state and retires stale focus/hits | Rich select content, existing-select interior edits, explicit new select associations, and createElement(select) remain unsupported |
| **Empty tables in HTML fragments** | In ordinary flow containers, an immediately paired table start and end tag produces an empty HTML table alongside its original text and element siblings. | Whitespace or other content inside the table, implicit endings and table-context insertion modes remain unsupported. |
| **Bare links in HTML fragments** | Checked ordinary fragments admit attribute-less HTML LINK nodes, with normal void serialization, document identity and later checked `rel`/`href` mutation. Replacing an old bare LINK requires untracked or pristine empty stylesheet state. | Attributed LINK fragments remain unsupported. Active/retired stylesheet generations, pending requests and removal notifications still block replacement; native checkbox fragment admission and frame/resource boundaries remain unchanged. |
| **Hidden inputs** | Fresh hidden-input admission, finite name/value/default reflection and whole-tree moves through native ownership; no box or focus target | Arbitrary existing input-type transitions and unrelated unsupported input kinds remain refused |
| **Checkbox and radio values** | Native HTML checkbox and radio inputs share one content attribute across `value`, `defaultValue`, attached Attr writes and form submission. An absent attribute yields `"on"` for `value` and an empty string for `defaultValue`. An explicit empty attribute yields an empty string for both. | Value mutations preserve checkedness, radio groups and form ownership. Native reflection applies to the HTML input's null-namespace `value` attribute. |
| **Text-input fragments** | Default/explicit text INPUT fragments through staged native transactions; dirty/default values, caret/selection, Reset and focus retirement; finite BUTTON.type reads | Unsupported kinds, foreign/inert ancestry, new explicit form associations and exhausted limits reject atomically. This is not unrestricted innerHTML/form support |
| **Anchor URLs** | Scoped live HTTP(S) `href` reflection and related URL values through shared URL/DOM behavior | Wider URL semantics remain contract-bounded; reflection does not grant navigation authority |
| **Language/runtime** | Original interpreter, selected built-ins/promises/jobs, owner-isolated intrinsic roots, RegExp input ToString and generic ordered `String.prototype.concat` | Classes, broader prototype access, UTF-16 code-unit semantics, generic RegExpExec/custom exec, object-valued lastIndex, modules, fetch/XHR and many APIs remain incomplete |
| **UTF-16 strings and RegExp** | Quoted strings, indexed reads, concatenation and classic RegExp preserve individual UTF-16 units. Case-sensitive `u` supports BMP and supplementary literals, braced code-point escapes, positive ranges and ordinary grouping and repetition. Unicode searches and zero-width iteration advance by code point while reporting UTF-16 offsets. | Property keys and native operations requiring scalar text reject unpaired units. Unicode dot, complemented sets, explicit surrogate literals, Unicode case folding, assertion groups, backreferences, property escapes, `v` and `d` remain outside this slice. |
| **Literal prototypes** | Finite static-colon object-literal prototype setters with real null/Object/Function links and ignored primitive RHS | Ordinary own/legacy `__proto__` operations and wider native-host prototype chains remain guarded |
| **Events** | Genuine owner-realm Event/UIEvent/MouseEvent/CustomEvent construction and branded state; real Event/UIEvent/CustomEvent dispatch/cancellation; finite trusted anchor click/auxclick; object listeners resolve their current `handleEvent` at delivery; dispatched base Events use genuine ordinary string/symbol property lookup with metered own storage and protected isTrusted | Synthetic Mouse dispatch, general pointer delivery, specialized keyboard interfaces and shadow retargeting remain gaps |
| **Animation frames** | Replaceable/deletable/restorable main-window scheduling and cancellation through real callbacks and budgets | Child-frame schedulers remain unsupported |

**Framework App rechecks.** On `067ac5e`, Vue 3.5.45 completes all 17 actions and jQuery 4.0.0 handles trusted input and its timer. At both widths, Bootstrap 5.3.9 completes its dropdown interaction and Vue 2.7.16 completes its reactive update. Vue 3.5.45 also passes all 17 actions on `45f9491`. On a development build of tagged source `f2e1823`, jQuery 1.12.4 stops at its raw `onfocusin` write at 4743:4, and ReactDOM 18.3.1 stops at `activeElement.contentEditable` at 58:228 before mounting. [Measured sources and boundaries](COMPATIBILITY.md)

**Earlier integration results.** PR #34's exact optimized candidate
passes the unchanged pinned jQuery 4.0.0 offline app through startup, genuine
trusted input, and an asynchronous timer text update. This is one workflow, not
AJAX/Deferred or full-framework support. Legacy jQuery and Vue retain their earlier
recorded fragment and language boundaries. Bootstrap initializes at both widths.
After PR #67/#68, measured candidate `13d7dae` visibly paints the open menu and
runs its trusted amber-choice callback at 1280/640. The menu stays open. The first
microtask fault is computed `position` at 1990:5, and dropdown input remains failed.
Candidate and integrated `main` `5309a68` CI each pass 2,695 normal tests and 25
journeys. The App captures identify the candidate source.
Integrated PR #69/#71 add computed `position` and layout-backed `offsetParent`.
Measured `c74503c` reaches `offsetParent` at 1994:12, then `96666d3` reaches
`scrollLeft` at 3154:19. Both runs open the menu and execute its trusted
Amber-choice callback at 1280/640. The menu remains visible after selection,
and dropdown input remains failed.
Those observations remain bound to their earlier builds. On integrated
`ec7db2f61cddb78c457b2966b72e1e362b89e66a`, PR #74/#75/#76 complete the pinned
Bootstrap dropdown: trusted open, Amber selection and true close pass with
paired DOM/pixels and responsive cards at 1280/640, with zero script errors.
On `acd5703`, the original WeakMap App probe reaches count 4 after a trusted
click and displays its completion title after one timer. On that source the
Vue 3.5.45 runtime stops at its first use of `WeakSet` before mounting.
See [compatibility outcomes](COMPATIBILITY.md) for exact versions and stages.

## Rendering and resources

| Capability | Implemented slice | Important limits |
| :--- | :--- | :--- |
| **CSS/layout** | Loaded styles; presence, exact, token, dash-prefix and substring attribute selectors shared with DOM queries; finite viewport/hover media groups; measured supported geometry/scrollHeight; clipping and software painting | Attribute comparisons retain the supported HTML case rules and ASCII `i`/`s` flags. Selector-string escapes, namespaces, wider media/selector grammar, pseudo-element generated painting and broad layout remain incomplete. |
| **Default grid rows** | A single implicit column lays out in-flow items in successive rows with actual row gaps. On `067ac5e` the Bootstrap action stack matches its literal and flex controls at 640 px for 8 and 16 px gaps. | Authored tracks, placement and per-item inline alignment are outside this source baseline. |
| **Text and box sizing** | Inherited custom properties can supply supported heights and line heights. An absolutely positioned caption sizes from the same word widths and spaces used to lay out its text. | The progress-meter comparison covers a 240×16 track and its 25% and 75% fills at 640 and 1280 px. |
| **Responsive foundation** | Initial-font em media, explicit box-sizing inheritance and fractional flex fitting; Foundation fixture stacks at 640 and forms two columns at 1280 | Does not establish framework initialization or complete component styling |
| **Document root selector** | Stylesheets and supported DOM queries match `:root` against the document element. Grouped theme rules can define custom properties that descendants inherit. | Detached elements and document fragments have no document element to match. |
| **Inline style enumeration** | `length`, `item()` and numeric indices read live declarations. `Object.keys()` returns their indices. A `for...in` loop also sees supported CSS property names and declaration methods. Custom declaration names retain their case. | Margin, padding, inset, gap and flex expand to longhands. Indexed inspection reports an error for shorthands whose longhands the scripting bridge cannot represent. |
| **HTMLElement inline style** | Main-document HTML elements with ordinary instance properties can shadow the inherited `style` accessor. A saved native declaration continues to update that element. Deleting a configurable shadow restores the inherited getter. | Whole-declaration `cssText` replacement and inline declaration handles for child documents remain unsupported. |
| **Inline easing reflection** | `animationTimingFunction` and `animation-timing-function` read and write checked specified declarations, including bounded Level 1 keywords, cubic-bezier functions, step functions and comma lists. Stored finite numbers retain precision independently of canonical readback. Equivalent values with unchanged priority preserve raw bytes. Invalid values preserve the previous declaration; an empty value removes it. | Declaration metadata only. No native animation timeline, interpolation or compositor is supplied. Values and serialized text are limited to 2 KiB, lists to 32 items and step counts to positive signed 32-bit integers. Easing `linear()`, `calc()` and custom-property substitution remain unsupported. |
| **Stylesheets** | Ordinary unmarked parser/dynamic HTTP(S) sheets, guarded redirects, final URL ownership and bounded accounting | No general stylesheet CORS, imports or CSSOM source access. Marked modes retain their documented subset |
| **Cold computed style** | The default host job allowance includes the parsing charge for up to 64 MiB of active stylesheets, with 768 Mi work units per job and four times that across startup. | Script-driven CSS checks the current job deadline during admission, parsing and cascade. The 8 MiB per-sheet and selector-work limits still apply. Work units do not reserve memory. |
| **Stylesheet selector matching** | Cached stylesheets can skip rules whose selector alternatives all require an absent rightmost class. Remaining rules keep their cascade order and run the full matcher. | Candidate collection, matching and computed values share the existing work limits. |
| **Stylesheet parsing during reflow** | Reflow and computed-style reads can reuse parsed rules when the connected sheet owners, order, source bytes, limits and media environment still match. Selector matching, form state, variables and computed values are evaluated again after a change. | Reuse covers one generation of fully admitted sheets in the current document. Source comparisons and new style computations consume the existing work budgets. |
| **Script-created HTML style elements** | Detached `textContent` replacement creates a Text node, or removes the children for an empty value. Direct insertion, movement and removal update the cascade in document order and preserve saved Text handles and live child lists. The `media` attribute responds to screen, all, print, empty and removal. | The complete sheet passes the document’s byte and rule limits. The browser checks type and inline style CSP on connection. Complex element media queries, connected text replacement, fragment splicing of STYLE nodes and CSSOM editing remain unsupported. |
| **CSS backgrounds** | One URL layer on ordinary block/inline-block boxes with ownership, cascade, repeat/position/size and shared resource lifecycle | Inline-fragment/replaced backgrounds and multiple layers remain incomplete |
| **Fonts and images** | Selected original decoders and bounded resource/presentation paths | Format subsets and cross-origin font loading require specific contracts; conditional/dynamic/resize font fetching remains limited |
| **Custom properties** | PR #31 integrates bounded, case-sensitive inherited variables for supported padding/border values, nested fallbacks, cycles, token boundaries and invalid-computed-value handling; original App pixels/hits/reflow match a literal control | Alpha colors, calc(), CSSOM and full Bootstrap component styling remain outside this qualification |
| **Margin and padding calculations** | Physical margins and padding accept inherited custom properties and a single `calc()` product of a length or percentage and a number in either order. Margins may be negative. Negative calculated padding resolves to zero. | Supported units are px, pt, pc, em, rem and percent. Sums, division and nested calculations remain outside this subset. |
| **Computed custom styles** | [PR #62](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/62), integrated at `5c21400`, provides live read-only custom-property values through the renderer. | Full CSSOM, computed pseudo-element declarations and child Element renderer bindings remain outside this slice. |
| **Opacity** | Live computed `opacity` and `getPropertyValue('opacity')` default to 1 and follow the author cascade, including variables and explicit inheritance. Ordinary HTML paint groups combine overlapping descendants before applying opacity, including nested groups, text and native controls. Opacity zero preserves layout and hit testing. | Computed values clamp to 0–1. The normal source-over painter bounds region snapshot work and simultaneous memory before painting. An unsupported computed opacity preserves that element's prior painting and unavailable readback. A group resource refusal withholds document paint and reports the limit while preserving chrome and inspector. Root/body group opacity is deferred because page-background propagation is outside this slice, with prior painting retained. Activity reports these limits. Blend modes, CSS math and general compositing are outside this slice. |
| **Computed overflow** | [PR #65](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/65) supplies live `overflow`, `overflowX` and `overflowY` reads from the rendering cascade, including `getPropertyValue`. Rectangular axis clips govern painting and hit bounds. | Clipping is qualified at initial element scroll positions. Element scrolling and scrollbar widgets remain unimplemented. |
| **Computed position and offset parents** | Integrated [PR #69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/69) supplies live computed `position` through named reads and `getPropertyValue`. [PR #71](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/71) returns supported HTMLElement `offsetParent` identities from the actual layout tree. | Existing document access and work checks apply. General child-element geometry and full CSSOM remain outside this slice. |
| **Viewport scroll offsets** | [PR #72](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/72), integrated at `13f5cd4`, adds standard-root vertical scroll reads/writes and zero offsets for supported non-scrollable axes. Qualified feature `e7be7cd` passes 2,732 normal tests and 25 journeys locally and in CI. | Nested element scrolling and scrollbar widgets remain outside this slice. On the measured feature at both Bootstrap widths, the chosen menu remains visible with first fault `offsetLeft` at 1927:10. |
| **Offset coordinates** | Integrated [PR #74](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/74) derives supported `offsetLeft`/`offsetTop` from first border boxes and actual offset-parent padding edges, preserving scroll independence. | Unmeasured fragments/parent boxes refuse explicitly; offsets retain untransformed geometry. |
| **Translations** | Integrated [PR #75](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/75) adds bounded 2D translations, nested composition, visual DOMRects, consistent paint/hits/clips and transformed absolute/fixed containing blocks, including identity translation. `getBoundingClientRect()` includes measured text-only inline border fragments across wrapped lines. | Wider transforms, nested inline border fragments and unmeasured reference geometry remain outside this slice. |
| **Functional background colors** | Stylesheet and inline `background-color` declarations and color-only `background` shorthands paint opaque `rgb()` and `rgba()` values, including values supplied by custom properties. Channels and alpha are clamped before painting. | Fractional-alpha backgrounds still require compositing. |
| **Rounded backgrounds and borders** | Block, flex and inline-block boxes support four equal circular corners on a background color. An opaque background also supports a uniform solid border, including a translucent border color. Radii support px, em, rem and custom properties. Overlapping curves shrink to fit the final box. | Background images, bordered boxes without an opaque color background, unequal border sides, elliptical corners and percentage radii retain square painting. Descendant clipping and pointer hit regions remain rectangular. |
| **Background color clipping** | Ordinary block, flex and inline-block color backgrounds support single-layer `border-box` (default), `padding-box` and `content-box`, using actual used border/padding insets. Equal circular corners with uniform insets share the rounded border composite. Existing color/none and inherit/initial/unset `background` shorthand forms reset clipping; custom properties and explicit inheritance participate in the existing cascade. | Image layers, text clipping, multiple layers, native controls and inline/table decorations retain existing painting. Unequal insets on rounded boxes retain their prior background fallback. Compound and rollback background shorthands remain unsupported. Color clipping does not change box geometry, descendant clips or pointer regions. |
| **Native button padding** | Button, submit and reset controls apply supported CSS padding to their boxes and caption placement. [PR #54](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/54) is integrated at `25654fa`. | Finite button-like control scope. Complete native appearance and authored border behavior remain separate. |
| **GPU acceleration** | Not implemented | Planned measured graphics backend with software fallback; not a replacement decoder grant |

<details>
<summary><strong>📐 Viewport and responsive behavior</strong></summary>

Finite em media lengths use the browser's initial 16 CSS-pixel font basis, not
an author root/body font or physical zoom. Recognized terminal ::before/::after
stylesheet targets preserve valid atomic selector lists without matching ordinary
elements or painting generated content. Explicit box-sizing inherit uses the
actual parent; initial/unset remain content-box. Fractional flex main sizes are
retained until cumulative pixel allocation so two 50% items fit odd widths while
real padding/minimum overflow still wraps.

After the initial load settles, viewport and zoom changes can select a new
source for a parser-discovered responsive image. A successful decode replaces
the displayed bitmap, while a failed replacement leaves the previous image
visible. A density change for the same URL reuses the decoded pixels. Repeated
selection uses bounded work and the existing image request policy.

Nested matching media rules use the real CSS viewport and an explicit desktop
mouse profile. Tests distinguish full viewport width from available line width,
retain fractional zoom thresholds, and verify geometry/background pixels/link
ownership. Admission counts inactive descendants; whole-sheet refusal cannot
publish a prefix. Initial font selection uses the same context, but nested font
faces and dynamic/resize font fetching are not added. Offscreen results are not
physical desktop or complete live-page results.

</details>

## Networking, trust, and diagnostics

| Area | Current behavior | Boundary |
| :--- | :--- | :--- |
| **Transport** | Original HTTP/TLS/certificate handling, redirects, decompression, workers and timings | Protocol, parser, chain and admission gaps remain |
| **Parser-loaded images** | Distinct image admission uses decoded backing bytes. Per-load reuse metadata has a separate byte eviction budget; evicting reuse entries preserves delivered pixels. Image batches retain three concurrent fetch slots and share the download allowance. | Decoder format/dimension/pixel checks, resource time and download allowances remain. DOM reference and dynamic-image queue restrictions are separate. |
| **Stylesheet requests** | Parser-loaded sheets use source/rule admission without a sheet-count ceiling. Dynamic requests share immutable policy snapshots and a byte reservation that survives queue transfers and clones until the final owner drops. Worker queue capacity and completed source/event payloads have byte admission and reserve failure-message storage before accepting a job. External replacements and inline detach/reinsert admit against current node-owned sources; removal releases source bytes/rules. Ownership-map capacity is charged to host retained storage. Parsed LINK mutations use current CSP checks and real completion events. | Complete stylesheet bytes and rules/declarations remain bounded. General parsed STYLE text mutation and CSSOM source editing remain incomplete. |
| **Local HTML source** | Opens regular local UTF-8 HTML files up to 32 MiB and retains the decoded source for inspection. | Plain-text viewing retains its separate 1 MiB bound. HTML tree node and depth bounds still apply. |
| **Loaded HTML tree** | Local and network documents can parse up to 262,144 nodes, including the synthetic root. | Crossing the node bound or the 128-level open-element bound reports a load failure. Script-inserted fragments use their separate checks. |
| **Classic script sources** | Parser-loaded and dynamic external scripts admit up to 32 MiB per source and 256 MiB of source per document. The loader bounds decompression and UTF-8 conversion. | Token records grow on demand within 256 MiB. Delimiter links use the token records. Compilation also shares a 768 MiB accounting budget for source copies, token payloads, and syntax storage, and checks the current job deadline. |
| **Script syntax and retention** | A script can compile up to 6,291,456 syntax nodes, with up to 16,777,216 retained across a document. | These limits apply alongside the compilation budget and bounded native stack. They describe charged storage and work. JavaScript strings retain their separate 8 MiB limit. |
| **Script call stack and scratch** | On Windows, guarded call entries check the current thread's actual stack reservation. They reserve one third of that stack, up to 1 MiB, as headroom for the remaining guarded evaluator, parser and unwind. | This replaces the fixed production call count. Argument and transient root scratch grow on demand under the retained-value byte budget, with actual argument slots and root capacity charged against it. Parser and evaluator recursion check the same Windows headroom. Explicit regression depth/count limits supplement physical checks; unmeasured platforms retain fallback bounds. Native Array conversion checks each active array without a production regression-count ceiling. |
| **JSON traversal scratch** | Parsing, reviver traversal and serialization grow heap frames on demand. Frame capacity and retained key and source snapshots use the retained-value byte budget, including during a callback's nested JSON call. | Document object, string and work budgets also apply. Callbacks still use the shared native stack checks. Serialization detects cycles on the active path and allows repeated objects in separate branches. |
| **Expression evaluation scratch** | The ordinary expression driver grows its heap continuations on demand. Frame, argument and transient root buffers and their retained operand data share a byte budget, including while a callback reenters evaluation. | Suspended async and generator state still have separate limits. |
| **Expression parser storage** | Groups, operators, computed keys, and call parsing grow heap continuations under the compilation storage budget. Optional-chain steps, call arguments, and template spans also charge their allocated storage. | Compilation deadlines, AST admission and genuinely recursive parser entry still have separate guards. |
| **Deferred function bodies** | Function templates and the deferred body queue grow under the compiler’s storage budget. Block bodies parse iteratively across lexical nesting. | Every deferred body must validate before any of the program executes. |
| **Recursive parser entry** | The Windows parser checks actual native stack headroom for recursive syntax, including compilation entered by a running script. | Small regression profiles and platforms without measured stack reservation retain a depth guard. The evaluator and individual syntax data structures have separate limits. |
| **Recursive evaluator entry** | Windows statement, expression, pattern and resumed evaluation check actual native stack headroom. | Small regression profiles and platforms without measured stack reservation retain a depth guard. |
| **Conditional continuations** | Flat else-if parsing uses the compiler's syntax storage admission. Selected branches retain target IDs in an allocated-capacity byte reservation, shared through callbacks and refunded on completion or failure. | Recursive branch bodies retain native stack protection. Small test profiles select a separate pending-branch allowance. |
| **Suspended function storage** | Async and generator lexical/control stacks and async assignment, call and comparison stacks grow by allocated capacity under the retained-value byte budget. Active async storage and retained values stay charged through reentry and transfer to Promise storage on suspension. Resumed call chains run iteratively with accounted suffix storage. | Environment identities, argument payloads, compilation and execution work carry their own budgets. Small test profiles can select a separate pending-operand allowance. |
| **Binary backing storage** | ArrayBuffer, typed arrays, DataView, ImageData and canvas readback share retained-value storage admission. Actual backing capacity is charged alongside other retained values, with checked ranges and fallible allocation. Failed unpublished ImageData creation refunds its backing. | Object identities and work have separate allowances. Buffers remain owned for the document lifetime; resizable, shared and detachable storage are outside the implemented slice. |
| **Dual-stack fallback** | Up to 256 resolver entries, per-family retention, eight bounded serial attempts, opposite-family time reservation and redacted diagnostics | Blocking OS DNS, up to one two-second connect before cancellation, and no concurrent Happy Eyeballs |
| **Trust** | Reviewed conservative 98-authority Mozilla NSS subset; explicit PEM replacement with shared 1,024-root capacity | No OS-store discovery, automatic root updates, revocation/CT, full Firefox policy or security certification |
| **Policy** | Original hostname for TLS/Host/origin; per-hop CSP, cookies, credentials, mixed-content, MIME and response-policy checks | A successful response is not a usable website or challenge clearance |
| **Diagnostics** | Document-only `--diagnose` with optional exact-host restrictions before DNS, phase/timing evidence and escaped bounded metadata | No page scripts/subresources, profile or usability assessment; raw reports may contain private data |

See [explicit HTTPS trust](../trust/README.md) and the [T1 contract](FEATURE_CONTRACTS.md#t1--shared-tlstrust-and-data-coverage-baseline).
The selected roots exclude unsupported forms and restrictions rather than
silently broadening trust. Custom stores replace the default and remain strict.

### Remaining capacity correction

The storage changes above do not complete the browser-wide capacity correction.
These confirmed production restrictions still require implementation review,
including their App defaults and callback consumers. They are not classified as
unavoidable safety boundaries merely because they currently have fixed values.

| Family and source | Current units and restrictions | Remaining correction |
| :--- | :--- | :--- |
| Function and environment arenas, `function_arena.rs` | Production slot growth uses shared retained-value bytes for parallel function metadata, environments and pre-reserved reusable IDs. Joint fallible reservations precede publication; one complete slot can fit without paying for a full geometric increment. Bound captures remain charged across observable getters. First backing refusal refunds dropped script/bound/Promise captures; inline records share arena backing. Collection follows backing/byte pressure and amortizes scans of fully live graphs. | Binding/index payload, later unpublished metadata retention, other identity arenas and reachability scratch have separate remaining policies. Explicit regression profiles retain count guards. |
| Promise state and metadata, `promises.rs`, `async_promise_reactions.rs` | Promise state, stable pair/capability/group/element stores, pending reaction capacity and suspended-await payloads use shared retained-value admission. Settlement retains the old reaction backing through transfer and refunds abandoned frames on failure; cancellation releases pending reaction storage. Queued jobs admit actual deque capacity plus owned payload; active callbacks retain their payload charge and await frames transfer to the active driver. Production checkpoints share execution work/deadline admission instead of a job-count cutoff. Cancellation refunds queue capacity and pending payload. | Native host-ticket cancellation and obsolete-record reclamation remain to be reconciled. Explicit regression profiles retain small identity/reaction/record/queue/checkpoint-count guards. |
| Runtime profiles, `execution_limits.rs` | 65,536 arguments; 4,096 programs; 1,048,576 bindings, object identities and properties per object; 16,777,216 document properties and array length. Source/string and compilation/retention byte budgets apply separately. | Reconcile remaining identity, metadata and sparse versus materialized storage admission and every derived lower limit. |
| Host callbacks and queues, `BROWSER_HOST` and consumers | Examples include 4,096 listeners and 1,024 timers; events, Promise jobs, MessagePorts, image queues and host retained bytes have additional policies. | Account pending records, payload and queue capacity together across dispatch, cancellation and navigation. |
| Collections, `collections.rs` | Production Map/Set/WeakMap/WeakSet admission uses shared retained-value bytes for payload and owned lookup/order capacity. Fallible replacement reservations precede publication. Compact live order storage preserves monotonic iterator sequence; deletion refunds payload and clear releases backing capacity. | Weak graph traversal, entry copy/compaction work, identity arenas and insertion sequence overflow checks remain. Explicit regression profiles retain small count guards. |
| Regular expressions, `regexp.rs` | 32 KiB patterns, 32,768 code entries, 32 nested groups/assertions, 16 captures, 1,000 repetitions and 1,024 backtracking choices; other repetition/name limits remain. | Replace fixed representations and recursive dependencies where required; keep checked allocation, work and cancellation. |
| DOM and layout, `html.rs`, `layout.rs` | 128 open HTML nodes; 20,000 internal versus 262,144 loaded nodes. Layout has depth 128, 2,048 flex/positioned entries, item/run/work and coordinate bounds. | Follow fragment/inert/frame entry defaults and recursive traversals through layout, paint and hit testing. |
| CSS, `css.rs`, `css_custom_properties.rs` | 512-byte individual selectors, nesting eight, list admission, 4,096 declarations and query results; custom-property names, map/output bytes, nesting and references are separately bounded. | Reconcile actual selector/value storage, recursive matching and aggregate style work. |
| Execution work, `execution_limits.rs` | Production jobs use 30 seconds, one billion steps and 16 GiB cumulative copy work; startup uses 120 seconds, four billion steps and 64 GiB copy work. Host and RegExp work have separate counters. | Distinguish cooperative scheduling/cancellation from retained resources; do not describe cumulative work as physical memory. |
| Resource loading, `loader.rs`, `http.rs` | 256 MiB aggregate resource bodies; resource time 45 seconds; HTTP header/line/chunk/redirect/coding and body policies remain. | Follow parser/dynamic/background consumers, cumulative source-use accounting and the distinction between protocol rules, concurrency, cache eviction and content admission. |
| Images, canvas and fonts, `image.rs`, `canvas_2d_bitmap.rs`, `font.rs`, `fonts.rs` | Examples include four-megapixel static output, 16,384 axes, 256 animation frames, 24 web faces, a 96-face merge bound and glyph structure/raster work limits. | Trace decoded backing and intermediate buffers; distinguish cache retention from refusal and recursive glyph safety. |
| Media and browser state | Container/decoder dimensions, frames, samples, tracks, media tasks, tabs, history, cookies and download storage have independent bounds. | Complete their representation/resource dispositions and effective startup/default consumer review. |

## Media

| Area | Implemented slice | Not established |
| :--- | :--- | :--- |
| **Formats** | Local IVF/VP8 visible fixed-canvas version-zero single-MV I/P, PCM and constrained Matroska A/V | SPLITMV, hidden/nonzero-version interframes, WebM/Opus/Vorbis or all codecs |
| **Authored playback** | One supported player, source/load lifecycle, native controls, script play/pause/seek, promises tied to real decoded output | Multiple players, MSE, streaming/ranges, muted Matroska or complete HTMLMediaElement |
| **Network playback** | Complete bounded HTTP(S) IVF/Matroska responses selected by MIME and explicit authored CORS/credential modes | Unmarked foreign playback, general Fetch/CORS, ordinary YouTube or service playback |
| **Interfaces** | Genuine owner-realm Media/Video roots/constants/accessors; saved canonical operations and Boolean controls reflection | VIDEO dimensions, dynamic crossOrigin, extra/child players and broader APIs |
| **Controls lifecycle** | Hide/restore bar without resetting source, clock, frames, seek or play promises; real hit/focus retirement | Physical input or audible output inferred from offscreen tests |

<details>
<summary><strong>🎞️ Decoding and playback evidence</strong></summary>

Original authored journeys check trusted play, event/Promise order, actual clock
advancement, independently expected frame pixels, pause/seek/freeze, native
controls, EOF/replay, Stop and navigation. Loopback HTTP/TLS journeys retain
those assertions and response identity. Policy, MIME/status/partial-response,
cancellation, replacement, and stale-completion negatives remain independent.
Issuer/hostname refusal checks prevent media HTTP/player installation and render
a real error event.

Interframe journeys visit original frames across two keyframe groups in IVF and
Matroska. Native YUV and centered-bilinear BT.601 display references separate
codec and presentation results; the recorded display profile bounds mean RGB
error below 0.5 and maximum channel error at 2. Entropy/reference updates,
segment retention, rollback, cancellation and bounded replay are tested.

Routine verification keeps audio devices closed. Opt-in WinMM checks establish
acknowledged PCM advancement, pause/seek/drain/replay and alignment for the selected
profile—not physical audibility or arbitrary-device compatibility.

</details>

## Evidence boundaries

| Evidence | Establishes | Does not establish |
| :--- | :--- | :--- |
| **Model/unit checks** | Specific conversion, owner, queue, rollback and lifetime rules | Full standards conformance or visible page success |
| **App/input/offscreen** | Real application input, state, layout, hit targets and decoded pixels | Physical desktop, speakers, or public-service compatibility |
| **Pinned library fixtures** | Exact versions and separately observed startup/input/async/layout results | Every library version or an unrelated website |
| **Reference-browser capture** | The actually captured comparison stages | Unrun reference interactions or successful ChezCat execution |
| **Hosted offline CI** | The recorded commit/configuration on its runner | Later source, release assets, devices or live services |
| **Published release** | Actual source/tag, payload, extracted checks and downloaded bytes | Signing, exhaustive certification, reproduction or universal support |
| **Physical/live check** | Explicit observed session and outcome | Results inferred from HTTP 200, titles, elapsed time or zero script errors |

For 0.82.0-alpha.1, the downloaded runtime and source ZIPs, `publication.json` and `SHA256SUMS` passed verification. The extracted executable passed startup and its original App journey. Its version, HTTP User-Agent and About identity were also checked. [Publication workflow](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/runs/44400222193)

The alpha.2 release has its own normal/optimized/hosted and scoped desktop evidence.
Later source changes retain separate checks and do not retroactively change those
released bytes. Google Search/Images/Maps are not fully supported; ordinary
YouTube playback remains unsupported.

## Implementation map

Paths are repository-root-relative references, not additional runtime payloads.

| Concern | Primary implementation / checks |
| :--- | :--- |
| DOM and native forms | `src/script_dom_tree.rs`, `src/script_node_root.rs`, `src/script_hidden_inputs.rs`, `src/script_select_fragments.rs`, native form modules and original Page/App controls |
| Language/realms | `src/javascript.rs`, `src/string_concat.rs`, `src/object_literals.rs`, `src/prototypes.rs`, `src/intrinsic_realms.rs`, `src/promises.rs` |
| Events | `src/event_interface.rs`, `src/script_dispatch.rs`, `src/script_created_events.rs`, `src/script_anchor_mouse.rs` |
| Network/trust | `src/http.rs`, `src/tls.rs`, `src/x509.rs`, `src/trust.rs`, `src/loader.rs`, original signed-path/data/negative checks |
| Layout/resources | `src/css.rs`, `src/css_media.rs`, `src/layout.rs`, `src/script_geometry.rs`, stylesheet/render journeys |
| Media | `src/media.rs`, `src/audio.rs`, `src/matroska.rs`, `src/script_media.rs`, authored/interface/source-replacement journeys |
| Browser shell | `src/app*.rs`, `src/platform*.rs`, `src/ui.rs`, `src/build_identity.rs`, session/native-owner controls |
| Delivery | `Build.cmd`, `tools/verify_offline.py`, [release guide](RELEASING.md) |

For full lasting acceptance requirements, consult [feature contracts](FEATURE_CONTRACTS.md).
For task order, use the [roadmap](ROADMAP.md); this matrix is not a release schedule.
