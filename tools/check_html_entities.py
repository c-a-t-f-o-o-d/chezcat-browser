#!/usr/bin/env python3
"""Check pinned HTML entity generation and reject damaged offline inputs.

This check does not build or execute ChezCat. Runtime parser behavior and
unchanged browser assertions retain their separate evidence selections.
"""

import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import generate_html_entities as generator


def encoded(value):
    return json.dumps(value).encode("ascii")


class HtmlEntityDataChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = generator.read_inputs()
        cls.manifest = json.loads((generator.DATA / "manifest.json").read_bytes())

    def test_pinned_inputs_reproduce_ascii_static_module(self):
        result = generator.render(self.inputs).encode("ascii")
        generator.verify_generated(result)
        self.assertIn(b"pub(super) const MAX_NAME_BYTES: usize = 32;", result)
        self.assertIn(b"pub(super) static ENTITIES: &[(&str, &str)] = &[", result)
        entries, maximum = generator.model(self.inputs)
        self.assertEqual((len(entries), maximum), (2231, 32))
        self.assertEqual(entries, sorted(entries))
        self.assertEqual(len({name for name, _ in entries}), 2231)

    def test_existing_foundation_pin_and_file_locks_are_retained(self):
        coverage = json.loads((generator.ROOT / "docs/HTML_FOUNDATION_COVERAGE.json").read_bytes())
        pin = coverage["input_pins"][generator.FOUNDATION["coverage_pin"]]
        self.assertEqual(coverage["input_pins"]["html"]["revision"], generator.HTML_REVISION)
        self.assertEqual(pin["revision"], generator.REVISION)
        self.assertEqual(pin["path"], generator.FILES["entities.json"]["upstream_path"])
        self.assertEqual(pin["git_blob"], generator.FILES["entities.json"]["git_blob"])
        for field in ("name_count", "multi_codepoint_count", "semicolonless_legacy_count"):
            self.assertEqual(pin[field], generator.INVENTORY[field])
        foundation = json.loads((generator.ROOT / generator.FOUNDATION["input_manifest"]).read_bytes())
        for expected in generator.FILES.values():
            matches = [row for row in foundation["inputs"] if row["repository"] == "html-build"
                       and row["path"] == expected["upstream_path"]]
            self.assertEqual(len(matches), 1)
            self.assertEqual(matches[0]["revision"], generator.REVISION)
            for field in ("bytes", "git_blob", "sha256"):
                self.assertEqual(matches[0][field], expected[field])

    def test_generated_attribution_and_distribution_license_are_retained(self):
        generator.verify_notice(self.inputs)
        rendered = generator.render(self.inputs)
        for text in (generator.REVISION, "WHATWG (Apple, Google, Mozilla, Microsoft)",
                     "BSD-3-Clause", "notices/WHATWG-HTML-Entities-LICENSE.txt",
                     "tools/reference/html-entities/"):
            self.assertIn(text, rendered)
        notices = (generator.ROOT / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8")
        self.assertIn("WHATWG HTML named-character-reference data", notices)
        self.assertIn("notices/WHATWG-HTML-Entities-LICENSE.txt", notices)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "LICENSE"
            path.write_bytes(self.inputs["LICENSE"] + b"\n")
            with self.assertRaisesRegex(ValueError, "Distribution notice differs"):
                generator.verify_notice(self.inputs, path)

    def test_each_source_rejects_changed_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            for name, original in self.inputs.items():
                for damaged in (original + b"\n", bytes([original[0] ^ 1]) + original[1:]):
                    with self.subTest(name=name, size=len(damaged)):
                        (folder / name).write_bytes(damaged)
                        with self.assertRaisesRegex(ValueError, "Pinned entity input mismatch"):
                            generator.read_inputs(folder)
                (folder / name).write_bytes(original)

    def test_manifest_revision_schema_and_every_file_identity_are_checked(self):
        variants = []
        for field, value in (("schema_version", True), ("schema_version", 2),
                             ("repository", "https://example.invalid"), ("revision", "0" * 40),
                             ("foundation", {}), ("inventory", {}), ("files", {})):
            manifest = copy.deepcopy(self.manifest)
            manifest[field] = value
            variants.append((field, manifest))
        for file_index in range(2):
            for field, value in (("bytes", 0), ("bytes", float(self.manifest["files"][file_index]["bytes"])),
                                 ("git_blob", "0" * 40), ("sha256", "0" * 64),
                                 ("upstream_path", "elsewhere.json")):
                manifest = copy.deepcopy(self.manifest)
                manifest["files"][file_index][field] = value
                variants.append((f"{file_index}:{field}", manifest))
        manifest = copy.deepcopy(self.manifest)
        manifest["inventory"]["name_count"] = 2231.0
        variants.append(("noninteger inventory", manifest))
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            for label, manifest in variants:
                with self.subTest(label=label):
                    (folder / "manifest.json").write_bytes(encoded(manifest))
                    with self.assertRaises(ValueError):
                        generator.read_inputs(folder)

    def test_manifest_rejects_path_escape_duplicate_missing_and_extra_fields(self):
        variants = []
        for path in ("../entities.json", "/tmp/entities.json", "dir/entities.json", "dir\\entities.json"):
            manifest = copy.deepcopy(self.manifest)
            manifest["files"][0]["path"] = path
            variants.append(manifest)
        manifest = copy.deepcopy(self.manifest)
        manifest["files"].append(copy.deepcopy(manifest["files"][0]))
        variants.append(manifest)
        manifest = copy.deepcopy(self.manifest)
        manifest["files"].pop()
        variants.append(manifest)
        manifest = copy.deepcopy(self.manifest)
        manifest["extra"] = "unselected"
        variants.append(manifest)
        manifest = copy.deepcopy(self.manifest)
        del manifest["files"][0]["sha256"]
        variants.append(manifest)
        manifest = copy.deepcopy(self.manifest)
        manifest["files"][0]["extra"] = "unselected"
        variants.append(manifest)
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            for index, manifest in enumerate(variants):
                with self.subTest(variant=index):
                    (folder / "manifest.json").write_bytes(encoded(manifest))
                    with self.assertRaises(ValueError):
                        generator.read_inputs(folder)

    def test_duplicate_json_members_and_non_json_constants_are_rejected(self):
        for source in (
                b'{"&amp;":{"codepoints":[38],"characters":"&"},"&amp;":{"codepoints":[38],"characters":"&"}}',
                b'{"&amp;":{"codepoints":[38],"codepoints":[38],"characters":"&"}}',
                b'{"schema_version":1,"schema_version":1}',
                b'{"value":NaN}', b'{"value":Infinity}', b'{} trailing'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                generator.parse_json(source)

    def test_entity_schema_and_ascii_names_are_checked(self):
        mapping = {"codepoints": [38], "characters": "&"}
        invalid = [[], {}, {"&amp;": None}, {"&amp;": [38]},
                   {"&amp;": {"codepoints": [38]}},
                   {"&amp;": {**mapping, "extra": True}}]
        invalid.extend({name: mapping} for name in (
            "amp;", "&", "&;", "&amp;;", "&am;p;", "&&amp;", "&amp_", "&é;", "&amp\n", "&1;"))
        for value in invalid:
            with self.subTest(value=value), self.assertRaises(ValueError):
                generator.parse_entities(encoded(value))

    def test_entity_scalars_and_characters_must_match_exactly(self):
        for scalars in ([], [38, 38, 38], [True], [38.0], ["38"], [-1],
                        [0x110000], [0xd800], [0xdfff], None):
            with self.subTest(scalars=scalars), self.assertRaisesRegex(ValueError, "scalar sequence"):
                generator.parse_entities(encoded({"&amp;": {"codepoints": scalars, "characters": "&"}}))
        for characters in (None, "", "&&", "A", "\ud800"):
            with self.subTest(characters=characters), self.assertRaisesRegex(ValueError, "characters/codepoints"):
                generator.parse_entities(encoded({"&amp;": {"codepoints": [38], "characters": characters}}))
        source = {"&Afr;": {"codepoints": [0x1d504], "characters": "\U0001d504"},
                  "&fjlig;": {"codepoints": [102, 106], "characters": "fj"}}
        self.assertEqual(generator.parse_entities(encoded(source)), [("Afr;", "\U0001d504"), ("fjlig;", "fj")])

    def test_complete_inventory_and_mapping_edges_are_preserved(self):
        entries, maximum = generator.model(self.inputs)
        names = dict(entries)
        self.assertEqual(sum(len(value) == 2 for value in names.values()), 93)
        self.assertEqual(sum(not name.endswith(";") for name in names), 106)
        self.assertEqual(len("CounterClockwiseContourIntegral;"), maximum)
        for name, expected in {"amp": "&", "amp;": "&", "AMP;": "&", "fjlig;": "fj",
                               "NotEqualTilde;": "\u2242\u0338", "ThickSpace;": "\u205f\u200a",
                               "Afr;": "\U0001d504", "nGt;": "\u226b\u20d2"}.items():
            self.assertEqual(names[name], expected)
        self.assertNotIn("Amp;", names)
        self.assertNotIn("apos", names)
        source = json.loads(self.inputs["entities.json"])
        del source["&amp;"]
        with self.assertRaisesRegex(ValueError, "inventory changed"):
            generator.model({**self.inputs, "entities.json": encoded(source)})

    def test_missing_files_and_stale_generated_output_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            for name in ("entities.json", "LICENSE", "manifest.json"):
                path = folder / name
                original = path.read_bytes()
                path.unlink()
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "Missing or linked"):
                    generator.read_inputs(folder)
                path.write_bytes(original)
            output = Path(temporary) / "output.rs"
            expected = generator.render(self.inputs).encode("ascii")
            output.write_bytes(expected)
            generator.verify_generated(expected, output)
            output.write_bytes(expected + b"\n")
            with self.assertRaisesRegex(ValueError, "module is stale"):
                generator.verify_generated(expected, output)

    def test_linked_input_files_directories_notices_and_output_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            linked = Path(temporary) / "linked-inputs"
            try:
                linked.symlink_to(folder, target_is_directory=True)
            except (OSError, NotImplementedError) as error:
                self.skipTest("Platform cannot create symlinks: " + str(error))
            with self.assertRaisesRegex(ValueError, "linked entity input directory"):
                generator.read_inputs(linked)
            for name in ("entities.json", "LICENSE", "manifest.json"):
                path = folder / name
                original = path.read_bytes()
                path.unlink()
                path.symlink_to(generator.DATA / name)
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "Missing or linked"):
                    generator.read_inputs(folder)
                path.unlink()
                path.write_bytes(original)
            output = Path(temporary) / "output.rs"
            output.symlink_to(generator.OUTPUT)
            with self.assertRaisesRegex(ValueError, "Missing or linked"):
                generator.verify_generated(generator.render(self.inputs).encode("ascii"), output)
            notice = Path(temporary) / "notice.txt"
            notice.symlink_to(generator.NOTICE)
            with self.assertRaisesRegex(ValueError, "Missing or linked"):
                generator.verify_notice(self.inputs, notice)


if __name__ == "__main__":
    unittest.main(verbosity=2)
