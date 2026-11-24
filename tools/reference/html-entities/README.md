# Pinned HTML named-character-reference data

This directory retains the normative data used by ChezCat's original HTML
named-reference matcher. No browser engine or third-party parser implementation
is included.

## Source and provenance

The exact upstream inputs are
[`entities/out/entities.json`](https://github.com/whatwg/html-build/blob/283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70/entities/out/entities.json)
and [`LICENSE`](https://github.com/whatwg/html-build/blob/283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70/LICENSE)
from WHATWG HTML-build revision
`283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70`. The JSON is retained here as
`entities.json`, without changing its bytes.

[`manifest.json`](manifest.json) records local and upstream paths, repository,
revision, byte lengths, Git blob SHA-1 identifiers and SHA-256 hashes. These are
the existing `html_build_entities` foundation pin and the exact two input records
in `tools/reference/html-foundation/inputs.json`. The separate normative HTML
algorithm pin remains `0e292f402d0c3dd17fe00c1bb2ef3a3c94cd8bce`; no input pin or
required foundation selection changes with this generated table.

The offline generator verifies the manifest against those exact identities and
checks both input files before use. It rejects duplicate JSON members, unexpected
paths and fields, symlinks, altered content, malformed names, invalid scalar
sequences and inconsistent character strings. Updating this data requires an
explicit pin change, review and regeneration.

## Selection and representation

All 2,231 spellings are retained, including 93 two-scalar mappings and 106 legacy
spellings without semicolons. Names are case-sensitive ASCII. The generated
`ENTITIES` slice is sorted by the exact spelling after removing only the initial
ampersand; semicolons remain part of the keys. Values preserve complete Unicode
scalar sequences using ASCII Rust Unicode escapes. `MAX_NAME_BYTES` is derived
from those keys and is 32 for the pinned input.

The data includes supplementary scalars, combining characters and two-character
ASCII output. It does not normalize, compose, case-fold or recursively decode
the replacements. `&nGt;` and `&nLt;` each expand five input bytes to six UTF-8
output bytes. The caller owns output admission and the HTML longest-match,
attribute-context and tokenizer-state rules.

Only the generated static Rust data and original matching code are used at
runtime. This source JSON, manifest and Python tooling are development inputs;
generation requires no network access or additional Python/Rust dependencies.

## Reproduce and check

From the repository root, using Python 3 and its standard library:

```console
python tools/generate_html_entities.py
python tools/generate_html_entities.py --check
python tools/check_html_entities.py
```

Generation is deterministic. `--check` compares the exact bytes of
`src/html_named_references_data.rs` without rewriting it and verifies the
distribution notice. The focused checks cover pin identities, schema and Unicode
rejection, damaged or linked paths, stale generated output and retained
attribution. These are data/provenance checks, not browser or tokenizer
conformance results. Full tokenizer states, streaming, parse diagnostics and
foundation package acceptance retain their separate requirements.

## License and attribution

Copyright © WHATWG (Apple, Google, Mozilla, Microsoft).

The complete upstream [`LICENSE`](LICENSE) is retained unchanged. It applies
CC BY 4.0 to the work and BSD 3-Clause to portions incorporated into source code.
The generated source attributes the normative data and its exact revision; the
generator and matching algorithms are original ChezCat code.

`notices/WHATWG-HTML-Entities-LICENSE.txt` is an exact copy for source and binary
distributions, and `THIRD_PARTY_NOTICES.md` records the separate provenance. The
existing release packaging includes tracked `notices/` files in runtime bundles;
the retained source inputs and generator belong in source archives.
