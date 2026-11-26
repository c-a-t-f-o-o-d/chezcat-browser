# Unicode simple case folding

The original development generator reads the pinned Unicode 17.0.0
`CaseFolding.txt` identified in `source.json`. Its bytes can be obtained from the
official file URL or the pinned UCD archive. From the repository root:

```text
python tools/generate_regexp_folding_data.py SOURCE_DIRECTORY --manifest tools/reference/unicode-case-folding/source.json --output src/regexp_unicode_folding_data.rs --check
```

Omit `--check` to regenerate. The generator verifies the complete input hash,
size and version. Only common (`C`) and simple (`S`) one-code-point mappings
enter the static table; full (`F`) and Turkic (`T`) mappings are excluded.
Canonical targets are idempotent and preserve UTF-16 width in this pinned data.
No downloaded implementation or runtime dependency is used.

The data is copyrighted Unicode, Inc. and distributed under Unicode License V3,
retained in `notices/Unicode-data-license.txt` and `THIRD_PARTY_NOTICES.md`.
See [ECMAScript Canonicalize](https://tc39.es/ecma262/multipage/text-processing.html#sec-runtime-semantics-canonicalize-ch)
for the Unicode RegExp mapping semantics. Classic non-Unicode case conversion
is a separate algorithm.
