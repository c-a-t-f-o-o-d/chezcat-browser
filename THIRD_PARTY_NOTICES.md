# Third-party and provenance scope

CSAPUL applies only to Covered Material designated in LICENSE_SCOPE.md. It does
not override separate terms, prior grants or independent rights. Preserve the
actual notices below; source visibility alone does not establish exclusive title.

| Material | Treatment |
| --- | --- |
| Public Suffix List snapshot/generated rules | Retain data/public_suffix/LICENSE-AND-NOTICE.md and provenance/source files; MPL-2.0 terms remain separate |
| Selected Test262 tests/harness | Retain tools/reference/test262/LICENSE and hash manifest; upstream BSD notice remains intact |
| Rust standard library/compiler runtime notices | notices/rust/COPYRIGHT-library.html and the accompanying license collection are refreshed from pinned Rust1.99.1; broader historical notices remain retained |
| Historical GCC/MinGW runtime notices | Preserve notices/ files; these do not establish current MSVC redistribution rights |
| Windows APIs/static CRT | APIs/toolchain are dependencies; verify applicable current Microsoft redistribution terms before binary release |
| Bundled public trust certificates | Retain fingerprints/provenance; redistribution basis and current trust-data policy require review |
| Original local fixtures and generated artwork | Retain generator/provenance records. Generated output does not prove exclusive copyright; individual asset and reference provenance still requires confirmation |

The original icon source retains C2PA/service provenance metadata, including an
embedded service badge; normalized runtime icon files do not carry that metadata.
Its provenance is not a blanket license to third-party marks or content. Runtime
bundles must include approved notices for every redistributed component. Optional
development tools are not production Cargo dependencies and are not bundled as
browser runtimes. No third-party engine/player/OS decoding service is included.
