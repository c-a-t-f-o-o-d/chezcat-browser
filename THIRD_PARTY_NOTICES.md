<div align="center">

# Third-party and provenance scope

*Separate materials keep their separate rights and notices.*

[Home](README.md) · [License scope](LICENSE_SCOPE.md) · [Trust data](trust/README.md) · [Public-suffix data](data/public_suffix/README.md)

</div>

---

CSAPUL applies only to Covered Material designated in LICENSE_SCOPE.md. It does
not override separate terms, prior grants or independent rights. Preserve the
actual notices below; source visibility alone does not establish exclusive title.

| Material | Treatment |
| --- | --- |
| Public Suffix List snapshot/generated rules | Retain data/public_suffix/LICENSE-AND-NOTICE.md and provenance/source files; MPL-2.0 terms remain separate |
| Selected Test262 tests/harness | Retain tools/reference/test262/LICENSE and hash manifest; upstream BSD notice remains intact |
| Rust standard library/compiler runtime notices | notices/rust/COPYRIGHT-library.html and the accompanying license collection are refreshed from pinned Rust1.99.1; broader historical notices remain retained |
| Historical GCC/MinGW runtime notices | Preserve notices/ files; these do not establish current MSVC redistribution rights |
| Unicode identifier and RegExp character properties | The generated ranges in src/unicode_identifier_ranges.rs and src/regexp_unicode_property_data.rs derive from Unicode 17.0.0 normative data, © 2025 Unicode, Inc. Identifier data pins DerivedCoreProperties.txt; RegExp categories, scripts/script extensions and binary properties pin their official input files and archive in tools/reference/unicode-properties/source.json. Emoji string-property sequences in src/regexp_unicode_emoji_data.rs additionally pin official emoji-sequences.txt and emoji-zwj-sequences.txt in tools/reference/unicode-emoji-properties/source.json. The generators, parser and matcher are original project code. Retain the Unicode License V3 in notices/Unicode-data-license.txt in source and binary distributions. |
| WHATWG Encoding indexes, ranges and labels | Generated data in src/text_encoding_single_byte.rs and src/text_encoding_multibyte_data.rs derives from WHATWG Encoding revision 7d2e3e08e11f57ba1b080390b490d53648c6f69c, © WHATWG (Apple, Google, Mozilla, Microsoft). Retain notices/WHATWG-Encoding-LICENSE.txt in source and binary distributions and tools/reference/text-encodings/ and tools/reference/multibyte-encodings/ provenance in source. The upstream license applies CC BY 4.0 to the work and BSD 3-Clause to portions incorporated into source code; the decoder and generator logic are original. Multibyte data includes GB18030/EUC-KR, Big5 (including supplementary mappings), JIS0208 and JIS0212 indexes; decoding algorithms and state machines are independently written. |
| WHATWG HTML named-character-reference data | Generated data in src/html_named_references_data.rs derives from entities/out/entities.json at WHATWG HTML-build revision 283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70, © WHATWG (Apple, Google, Mozilla, Microsoft). Retain notices/WHATWG-HTML-Entities-LICENSE.txt in source and binary distributions and tools/reference/html-entities/ provenance in source. The upstream license applies CC BY 4.0 to the work and BSD 3-Clause to portions incorporated into source code. All 2,231 spellings and complete one- or two-scalar mappings are retained; the generator and matching algorithms are original ChezCat code. |
| Windows APIs/static CRT | APIs/toolchain are dependencies; verify applicable current Microsoft redistribution terms before binary release |
| Mozilla NSS certificate data and generated trust subset | Retain trust/mozilla/certdata.txt notices, LICENSE-MPL-2.0.txt, PROVENANCE.json, policy.json and selection.json; upstream MPL-2.0 terms remain separate. The original verifier and conservative data subset do not reproduce the complete Firefox trust policy |
| Local fixtures and artwork | Retain applicable provenance, ownership and credit records. Individual asset and reference rights require confirmation; inclusion does not establish exclusive title |
| Original VP8 I/P and PCM fixtures | tools/make_interframe_fixture.py authors scene/sample inputs; FFmpeg/libvpx are development encoding/reference tools only and are not redistributed. Preserve tools/fixtures/scene-inter-PROVENANCE.md and static payload/reference identities |

Project artwork does not grant a blanket license to third-party marks or content. Runtime
bundles must include approved notices for every redistributed component. Optional
development tools are not production Cargo dependencies and are not bundled as
browser runtimes. No third-party engine/player/OS decoding service is included.
