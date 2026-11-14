# Original parser-continuation AST witnesses

`ast-witnesses.tsv` contains 1431 locally authored/generated expressions and
normalized syntax trees produced by independent Acorn 8.15.0. Node 22.16.0 also
checks accepted syntax. No external implementation code is copied. The 0.47 Rust
unit test compares these expectations to ChezCat; it has not run in the authoring
environment. The tests need only this checked-in text, not Acorn or Node.

Regeneration is optional development work, not part of the browser or normal
build. Install/locate Acorn independently, then run:

```text
node tools/reference/parser-047/generate.cjs <path-to-acorn> <output-directory>
```

The generator records its fixed seed, versions, source references, rejected
candidate expressions and output SHA-256 in provenance.json. Its original inputs
cover operator/assignment precedence, grouping, calls, spread/trailing commas,
computed keys, constructors, optional-chain boundaries and seeded combinations.
No Google or other captured site script is present.

The Python transition model is under results/verification/pass-02l/20261112/.
It models the authored algorithm, not the Rust compiler, tokenizer or full JS
language, and cannot replace native compilation, stack-safety or semantic tests.
