<div align="center">

# Retained development references

*Frozen inputs and independent expectations—not disposable run output.*

[Home](../../README.md) · [Build guide](../../docs/BUILDING.md) · [Compatibility](../../docs/COMPATIBILITY.md)

</div>

---

These are active inputs for the regression tools, not old verification runs.

- `test262/`: pinned upstream tests, their harness, license, and hash manifest.
- `review-040/` and `review-041/`: original frozen Node outcomes, specification
  expectations, and boundary adjudications used by the independent checkers.
- `url-041/`: original URL programs and corrected boundary inputs.

The version labels identify when a fixture was introduced. Keep the input
programs and expected outcomes unchanged when moving files. Test262 bytes must
match its manifest; do not run bulk text conversion over reference files.

Reviewers use the maintained `../check_javascript.py` and `../check_promises.py`
runners. Historical `helper_sha256` fields in frozen reference JSON describe the
original runner used to establish those outcomes. New reports hash the current
runner separately, and comparisons still assert each frozen Node outcome. The
0.41.2 cleanup verified equivalent outcomes for all 170 older independent Node
programs; it did not regenerate their expected results.

Current run reports belong in `results/verification/`, outside this directory.
