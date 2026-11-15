# Bundled public-suffix data

ChezCat's original lookup in `src/public_suffix.rs` embeds `rules.rs` at compile
time. No file beside the executable, network service, third-party Rust crate,
Java runtime, or downloaded feature pack is required. This directory contains
**data**, not copied browser or library implementation code.

## Provenance and freshness

The dataset is pinned to upstream Public Suffix List revision
`823beb1425def931c8b0b170a6bc33b424c3f9b0`, dated **2025-05-12**. This historical
snapshot is not a current list and can miss newer public/private hosting
boundaries. Review and refresh it for current hosting boundaries; do not describe
it as comprehensive current cookie-isolation data.

The rule set was recovered from Debian's OpenJDK distribution data, whose separate
MPL notice identifies the upstream revision. The exact distribution archive is
retained in `tools/reference/public-suffix/openjdk-public-suffix.dat` (relative
to the project root). No Java implementation was copied. `source.dat` reconstructs
its rule lines and ICANN/PRIVATE section membership; it is not a byte-identical
copy of upstream's commented source. SHA-256 values and distribution version are
recorded in `provenance.json`. `LICENSE-AND-NOTICE.md` retains the full notice and
Mozilla Public License 2.0. The rule data and generated representation remain
under that license; the original lookup is separate code.

This snapshot includes 6,870 ICANN and 2,892 PRIVATE rules, without dropping
Unicode entries. Offline generation converts normalized Unicode labels to their
ASCII Punycode spellings using Python's standard library. This does not add
runtime IDNA conversion: the existing URL parser still requires ASCII/punycode.

## Algorithm and policy

Lookup uses bounded label iteration and binary searches over sorted arrays for
exact, wildcard, and exception rules. It has no allocation or recursive walk.
Both PSL sections affect cookies and schemeful site equality. Unknown suffixes
use the implicit one-label `*` rule. IP addresses are handled separately by the
cookie code. SameSite ignores ports but distinguishes HTTP from HTTPS.

The formal PSL algorithm has a documented wildcard-base ambiguity among libraries.
ChezCat treats a wildcard's base itself as public, matching libpsl's conservative
convention. In addition, a Domain cookie cannot widen beyond its originating
host's registrable site. A tenant cannot skip a nested private suffix by choosing
a higher parent that is not itself a listed suffix. These checks deliberately
prefer rejecting an unsafe broad scope to sharing it across registrations.

## Deliberate offline maintenance

To verify the checked-in representation:

```text
python tools/generate_public_suffix.py --check
python tools/check_cookie_suffix_data.py
```

The second command compares a Python model with original witnesses and, when
installed, a system libpsl development oracle loaded with the **same** data. It
is not execution of ChezCat's Rust. Normal Cargo tests exercise the real lookup.
Neither development tool is used by the browser or required by ordinary builds.

For a later refresh, obtain a complete pinned `public_suffix_list.dat` from the
PSL project, preserve its license/comments, replace `source.dat`, and update
`provenance.json` with its revision/date/source hashes. Then regenerate offline,
review rule additions/removals and run the suffix and cookie tests. Update the
historical-distribution reconstruction check in `check_cookie_suffix_data.py` as
part of that intentional data migration; do not relabel the old archive as a new
snapshot. No startup/background updater is introduced.

Primary data/format sources:
- https://publicsuffix.org/list/
- https://github.com/publicsuffix/list/wiki/Format
- https://github.com/publicsuffix/list/commit/823beb1425def931c8b0b170a6bc33b424c3f9b0
