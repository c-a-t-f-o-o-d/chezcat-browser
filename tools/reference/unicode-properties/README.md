# Unicode RegExp character-property data

The generated static ranges use Unicode 17.0.0, matching the identifier-data version. `source.json` pins the official UCD archive and the size and SHA-256 of each input file. The generator validates every input identity before producing output; it makes no network requests.

Download and extract the pinned official archive into a local directory, then run from the repository root:

```text
python tools/generate_regexp_property_data.py UCD_DIRECTORY --manifest tools/reference/unicode-properties/source.json --output src/regexp_unicode_property_data.rs
```

Add `--check` to compare regeneration with the existing file. The generator is development tooling; production uses original Rust lookup, parsing and matching over static ranges. No external Unicode library or runtime is required.

General-category aliases come from PropertyValueAliases.txt. Script extensions override Script only for explicitly listed code points and otherwise inherit Script. Binary property names and aliases are restricted to ECMAScript Table 66; synthetic ASCII, Any and Assigned use their specified definitions. Unsupported emoji string properties remain separate. Category coverage includes unassigned code points, private use and surrogate categories.

Normative sources: [Unicode 17.0.0 UCD](https://www.unicode.org/Public/17.0.0/ucd/) and [ECMAScript Unicode property tables](https://tc39.es/ecma262/multipage/text-processing.html#table-binary-unicode-properties). Retain the source attribution and [Unicode License V3](../../../notices/Unicode-data-license.txt); see [third-party notices](../../../THIRD_PARTY_NOTICES.md). Generated data is derived from normative property files; the generation and browser algorithms are original.
