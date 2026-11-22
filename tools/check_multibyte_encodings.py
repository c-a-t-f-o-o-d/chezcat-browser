#!/usr/bin/env python3
"""Check pinned multibyte generation and reject damaged development inputs.

This offline check does not build or execute ChezCat. Decoder tests separately
compare runtime output with the retained normative files, including holes and
four-byte boundaries; these checks do not claim browser conformance.
"""

import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import generate_multibyte_encodings as generator


class MultibyteDataChecks(unittest.TestCase):
    def test_pinned_inputs_reproduce_checked_in_module(self):
        inputs = generator.read_inputs()
        self.assertEqual(len(inputs), 5)
        self.assertEqual(generator.OUTPUT.read_bytes(), generator.render(inputs).encode("utf-8"))

    def test_distribution_notice_preserves_upstream_license(self):
        notice = generator.ROOT / "notices/WHATWG-Encoding-LICENSE.txt"
        self.assertEqual(notice.read_bytes(), generator.read_inputs()["LICENSE"])

    def test_changed_source_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            path = folder / "index-gb18030.txt"
            path.write_bytes(path.read_bytes() + b"\n")
            with self.assertRaisesRegex(ValueError, "Pinned multibyte input mismatch"):
                generator.read_inputs(folder)

    def test_changed_revision_length_or_digest_is_rejected(self):
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
                    with self.assertRaisesRegex(ValueError, "Pinned multibyte input mismatch"):
                        generator.read_inputs(folder)

    def test_manifest_paths_repeated_and_missing_inputs_are_rejected(self):
        original = json.loads((generator.DATA / "manifest.json").read_bytes())
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            path = folder / "manifest.json"
            for name in ["../index-gb18030.txt", "/tmp/encodings.json", "dir/index-euc-kr.txt"]:
                with self.subTest(path=name):
                    manifest = copy.deepcopy(original)
                    manifest["files"][0]["path"] = name
                    path.write_text(json.dumps(manifest), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "Unexpected multibyte input path"):
                        generator.read_inputs(folder)
            manifest = copy.deepcopy(original)
            manifest["files"].append(copy.deepcopy(manifest["files"][0]))
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Duplicate multibyte input path"):
                generator.read_inputs(folder)
            manifest = copy.deepcopy(original)
            manifest["files"].pop()
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Incomplete multibyte input selection"):
                generator.read_inputs(folder)
            path.write_text(json.dumps(original), encoding="utf-8")
            (folder / "index-euc-kr.txt").unlink()
            with self.assertRaisesRegex(ValueError, "Missing or linked multibyte input"):
                generator.read_inputs(folder)

    def test_index_preserves_holes_literal_c1_and_final_pointer(self):
        data = "# example\n0\t0x0085\t\u0085 (<control>)\n2\t0x20AC\t\u20ac\n23939\t0xFFFE\tx\n"
        index = generator.parse_index(data.encode("utf-8"))
        self.assertEqual(len(index), 23940)
        self.assertEqual(index[0], 0x85)
        self.assertEqual(index[1], generator.UNDEFINED)
        self.assertEqual(index[2], 0x20AC)
        self.assertEqual(index[23938], generator.UNDEFINED)
        self.assertEqual(index[23939], 0xFFFE)

    def test_index_rejects_duplicate_pointers_and_invalid_scalars_or_rows(self):
        invalid = [
            b"0\t0x20AC\tx\n0\t0x20AC\tx\n",
            b"23940\t0x20AC\tx\n",
            b"-1\t0x20AC\tx\n",
            b"0\t0xD800\tx\n",
            b"0\t0xDFFF\tx\n",
            b"0\t0xFFFF\tx\n",
            b"0\t0x10000\tx\n",
            b"0\t0x110000\tx\n",
            b"0 0x20AC x\n",
            b"0\t0x20AC\n",
            b"# empty table\n",
        ]
        for data in invalid:
            with self.subTest(data=data), self.assertRaises(ValueError):
                generator.parse_index(data)

    def test_range_values_preserve_supplementary_scalars(self):
        self.assertEqual(
            generator.parse_ranges(b"0\t0x0080\n189000\t0x10000\n1237575\t0x10FFFF\n"),
            [(0, 0x80), (189000, 0x10000), (1237575, 0x10FFFF)],
        )

    def test_range_order_bounds_scalars_and_shape_are_checked(self):
        invalid = [
            b"0\t0x0080\n0\t0x0081\n",
            b"1\t0x0080\n0\t0x0081\n",
            b"0\t0x0080\n1\t0x0080\n",
            b"0\t0x0081\n1\t0x0080\n",
            b"0\t0xD800\n",
            b"0\t0xDFFF\n",
            b"0\t0x110000\n",
            b"1237576\t0x10FFFF\n",
            b"-1\t0x0080\n",
            b"0 0x0080\n",
            b"0\t0x0080\textra\n",
            b"# empty ranges\n",
        ]
        for data in invalid:
            with self.subTest(data=data), self.assertRaises(ValueError):
                generator.parse_ranges(data)

    def test_selected_aliases_do_not_add_other_families(self):
        entries, _, _, _ = generator.model(generator.read_inputs())
        self.assertEqual(dict(entries), {
            "gbk": ["chinese", "csgb2312", "csiso58gb231280", "gb2312", "gb_2312",
                    "gb_2312-80", "gbk", "iso-ir-58", "x-gbk"],
            "gb18030": ["gb18030"],
            "euc-kr": ["cseuckr", "csksc56011987", "euc-kr", "iso-ir-149", "korean",
                       "ks_c_5601-1987", "ks_c_5601-1989", "ksc5601", "ksc_5601",
                       "windows-949"],
        })

    def test_pinned_mapping_edges_and_korean_holes(self):
        _, gb, kr, ranges = generator.model(generator.read_inputs())
        self.assertEqual((gb[0], gb[23939]), (0x4E02, 0xE4C5))
        self.assertEqual((gb[16293], gb[14761]), (0x4E2D, 0x6587))
        self.assertEqual((kr[0], kr[9026], kr[23749]), (0xAC02, 0xAC00, 0x8A70))
        self.assertEqual(kr[26], generator.UNDEFINED)
        self.assertEqual(kr[23750:], [generator.UNDEFINED] * 190)
        self.assertEqual(sum(value == generator.UNDEFINED for value in kr), 6892)
        self.assertEqual((len(ranges), ranges[0], ranges[-1]), (207, (0, 0x80), (189000, 0x10000)))

    def test_duplicate_json_fields_and_invalid_labels_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "Duplicate JSON field"):
            json.loads('{"revision": "first", "revision": "second"}',
                       object_pairs_hook=generator.unique_object)
        inputs = generator.read_inputs()
        original = json.loads(inputs["encodings.json"])
        for alias in ["gbk", "GBK", " gbk", "gbk\n", "gbk\"", "euc-kr"]:
            with self.subTest(alias=alias):
                groups = copy.deepcopy(original)
                selected = next(encoding for group in groups for encoding in group["encodings"]
                                if encoding["name"] == "GBK")
                selected["labels"][0] = alias
                changed = {**inputs, "encodings.json": json.dumps(groups).encode("utf-8")}
                with self.assertRaises(ValueError):
                    generator.model(changed)


if __name__ == "__main__":
    unittest.main(verbosity=2)
