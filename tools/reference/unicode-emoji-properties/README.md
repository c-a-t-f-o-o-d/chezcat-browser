# Unicode emoji string properties

The original development-only generator reads the two official Unicode 17.0
emoji sequence files identified by `source.json`. Download those exact files
into a local directory, then run from the repository root:

```text
python tools/generate_regexp_emoji_data.py SOURCE_DIRECTORY --manifest tools/reference/unicode-emoji-properties/source.json --output src/regexp_unicode_emoji_data.rs --check
```

Omit `--check` to regenerate. The generator verifies both complete file hashes,
sizes and versions before parsing. It expands Basic_Emoji scalar ranges,
retains exact code-point sequences for the six source properties and forms
RGI_Emoji as their union. Every sequence is stored once; sorted property tables
reference those members. No downloaded implementation or runtime dependency is
used. The generated tables are shipped with the original Rust matcher.

The files are Unicode data under Unicode License V3, reproduced in
`THIRD_PARTY_NOTICES.md`. Normative semantics are described in
[Unicode Emoji](https://www.unicode.org/reports/tr51/) and the
[ECMAScript RegExp specification](https://tc39.es/ecma262/multipage/text-processing.html).
