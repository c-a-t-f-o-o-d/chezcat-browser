#!/usr/bin/env python3
"""Check pinned single-byte generation and reject damaged development inputs.

This offline check does not build or execute ChezCat. The Rust decoder tests
independently compare runtime output with the checked-in normative index files.
"""

import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import generate_single_byte_encodings as generator


class SingleByteDataChecks(unittest.TestCase):
    def test_pinned_inputs_reproduce_checked_in_module(self):
        inputs = generator.read_inputs()
        self.assertEqual(len(inputs), 29)
        self.assertEqual(generator.OUTPUT.read_bytes(), generator.render(inputs).encode("utf-8"))

    def test_distribution_notice_preserves_upstream_license(self):
        notice = generator.ROOT / "notices/WHATWG-Encoding-LICENSE.txt"
        self.assertEqual(notice.read_bytes(), generator.read_inputs()["LICENSE"])

    def test_changed_source_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            path = folder / "index-windows-1252.txt"
            path.write_bytes(path.read_bytes() + b"\n")
            with self.assertRaisesRegex(ValueError, "Pinned encoding input mismatch"):
                generator.read_inputs(folder)

    def test_changed_revision_or_digest_is_rejected(self):
        original = json.loads((generator.DATA / "manifest.json").read_bytes())
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            path = folder / "manifest.json"
            manifest = copy.deepcopy(original)
            manifest["revision"] = "0" * 40
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "manifest or revision"):
                generator.read_inputs(folder)
            for field, value in [("bytes", 0), ("git_blob", "0" * 40), ("sha256", "0" * 64)]:
                with self.subTest(field=field):
                    manifest = copy.deepcopy(original)
                    manifest["files"][0][field] = value
                    path.write_text(json.dumps(manifest), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "Pinned encoding input mismatch"):
                        generator.read_inputs(folder)

    def test_manifest_cannot_escape_input_folder_or_repeat_files(self):
        original = json.loads((generator.DATA / "manifest.json").read_bytes())
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            path = folder / "manifest.json"
            for name in ["../index-windows-1252.txt", "/tmp/encodings.json", "dir/index-ibm866.txt"]:
                with self.subTest(path=name):
                    manifest = copy.deepcopy(original)
                    manifest["files"][0]["path"] = name
                    path.write_text(json.dumps(manifest), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "Unexpected encoding input path"):
                        generator.read_inputs(folder)
            manifest = copy.deepcopy(original)
            manifest["files"].append(copy.deepcopy(manifest["files"][0]))
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Duplicate encoding input path"):
                generator.read_inputs(folder)

    def test_index_preserves_holes_and_literal_c1_description(self):
        index = generator.parse_index("# example\n0\t0x0085\t\u0085 (<control>)\n2\t0x20AC\t\u20ac\n".encode("utf-8"))
        self.assertEqual(index[0], 0x85)
        self.assertEqual(index[1], generator.UNDEFINED)
        self.assertEqual(index[2], 0x20AC)
        self.assertEqual(index[127], generator.UNDEFINED)

    def test_index_rejects_invalid_rows(self):
        invalid = [
            b"0\t0x20AC\tx\n0\t0x20AC\tx\n",
            b"128\t0x20AC\tx\n",
            b"-1\t0x20AC\tx\n",
            b"0\t0xD800\tx\n",
            b"0\t0xFFFF\tx\n",
            b"0\t0x10000\tx\n",
            b"0 0x20AC x\n",
            b"# empty table\n",
        ]
        for data in invalid:
            with self.subTest(data=data), self.assertRaises(ValueError):
                generator.parse_index(data)

    def test_pinned_alias_identity_and_mapping_edges(self):
        entries, tables = generator.model(generator.read_inputs())
        by_name = {name: (aliases, table) for _, name, aliases, table in entries}
        self.assertIn("logical", by_name["iso-8859-8-i"][0])
        self.assertIn("visual", by_name["iso-8859-8"][0])
        self.assertEqual(by_name["iso-8859-8"][1], by_name["iso-8859-8-i"][1])
        self.assertIn("ansi_x3.4-1968", by_name["windows-1252"][0])
        self.assertEqual(tables["INDEX_WINDOWS_1252"][0], 0x20AC)
        self.assertEqual(tables["INDEX_WINDOWS_1252"][1], 0x81)
        self.assertEqual(tables["INDEX_ISO_8859_6"][0xA1 - 0x80], generator.UNDEFINED)


if __name__ == "__main__":
    unittest.main(verbosity=2)
