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
        self.assertEqual(len(inputs), 8)
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

    def test_big5_table_keeps_full_unicode_and_distinct_hole_sentinel(self):
        data = b"0\t0xFFFF\tx\n947\t0x27267\tx\n19781\t0x10FFFF\tx\n"
        index = generator.parse_index(data, generator.BIG5_SIZE, wide=True)
        self.assertEqual(len(index), 19782)
        self.assertEqual(index[0], 0xFFFF)
        self.assertEqual(index[947], 0x27267)
        self.assertEqual(index[19781], 0x10FFFF)
        self.assertEqual(index[946], 0x110000)
        self.assertNotEqual(index[946], index[0])

    def test_each_index_enforces_its_own_pointer_domain_and_scalar_width(self):
        for size, wide in [(generator.BIG5_SIZE, True),
                           (generator.JIS0208_SIZE, False),
                           (generator.JIS0212_SIZE, False)]:
            with self.subTest(size=size):
                valid = f"{size - 1}\t0x0041\tx\n".encode("ascii")
                self.assertEqual(generator.parse_index(valid, size, wide)[-1], 0x41)
                for data in [f"{size}\t0x0041\tx\n".encode("ascii"),
                             b"-1\t0x0041\tx\n", b"0\t0xD800\tx\n",
                             b"0\t0xDFFF\tx\n", b"0\t0x110000\tx\n",
                             b"0\t0xFFFFFFFF\tx\n",
                             b"0\t0x0041\tx\n0\t0x0042\tx\n"]:
                    with self.subTest(data=data), self.assertRaises(ValueError):
                        generator.parse_index(data, size, wide)
        with self.assertRaises(ValueError):
            generator.parse_index(b"0\t0x10000\tx\n", generator.JIS0208_SIZE)

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
        entries, _, _ = generator.model(generator.read_inputs())
        self.assertEqual(dict(entries), {
            "gbk": ["chinese", "csgb2312", "csiso58gb231280", "gb2312", "gb_2312",
                    "gb_2312-80", "gbk", "iso-ir-58", "x-gbk"],
            "gb18030": ["gb18030"],
            "big5": ["big5", "big5-hkscs", "cn-big5", "csbig5", "x-x-big5"],
            "euc-jp": ["cseucpkdfmtjapanese", "euc-jp", "x-euc-jp"],
            "iso-2022-jp": ["csiso2022jp", "iso-2022-jp"],
            "shift_jis": ["csshiftjis", "ms932", "ms_kanji", "shift-jis", "shift_jis",
                          "sjis", "windows-31j", "x-sjis"],
            "euc-kr": ["cseuckr", "csksc56011987", "euc-kr", "iso-ir-149", "korean",
                       "ks_c_5601-1987", "ks_c_5601-1989", "ksc5601", "ksc_5601",
                       "windows-949"],
        })

    def test_pinned_mapping_edges_and_korean_holes(self):
        _, indexes, ranges = generator.model(generator.read_inputs())
        gb, kr = indexes["GB18030"], indexes["EUC_KR"]
        self.assertEqual((gb[0], gb[23939]), (0x4E02, 0xE4C5))
        self.assertEqual((gb[16293], gb[14761]), (0x4E2D, 0x6587))
        self.assertEqual((kr[0], kr[9026], kr[23749]), (0xAC02, 0xAC00, 0x8A70))
        self.assertEqual(kr[26], generator.UNDEFINED)
        self.assertEqual(kr[23750:], [generator.UNDEFINED] * 190)
        self.assertEqual(sum(value == generator.UNDEFINED for value in kr), 6892)
        self.assertEqual((len(ranges), ranges[0], ranges[-1]), (207, (0, 0x80), (189000, 0x10000)))

    def test_pinned_big5_supplementary_values_and_algorithmic_pair_holes(self):
        _, indexes, _ = generator.model(generator.read_inputs())
        index = indexes["BIG5"]
        self.assertEqual(index[:942], [generator.UNDEFINED_WIDE] * 942)
        self.assertEqual((index[942], index[947], index[19781]), (0x43F0, 0x27267, 0x79D4))
        self.assertEqual(sum(v != generator.UNDEFINED_WIDE for v in index), 18590)
        self.assertEqual(sum(0xFFFF < v < generator.UNDEFINED_WIDE for v in index), 1713)
        for pointer in [1133, 1135, 1164, 1166]:
            self.assertEqual(index[pointer], generator.UNDEFINED_WIDE)

    def test_pinned_jis_planes_preserve_holes_extensions_and_full_domains(self):
        _, indexes, _ = generator.model(generator.read_inputs())
        jis0208, jis0212 = indexes["JIS0208"], indexes["JIS0212"]
        self.assertEqual((len(jis0208), len(jis0212)), (11280, 8836))
        self.assertEqual((jis0208[0], jis0208[11103]), (0x3000, 0x9ED1))
        self.assertEqual(jis0208[11104:], [generator.UNDEFINED] * 176)
        self.assertEqual((jis0212[108], jis0212[7210]), (0x02D8, 0x9FA5))
        self.assertEqual(jis0212[:108], [generator.UNDEFINED] * 108)
        self.assertEqual(jis0212[7211:], [generator.UNDEFINED] * 1625)
        self.assertEqual(sum(v != generator.UNDEFINED for v in jis0208), 7724)
        self.assertEqual(sum(v != generator.UNDEFINED for v in jis0212), 6067)

    def test_changed_new_indexes_are_rejected_before_generation(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary) / "inputs"
            shutil.copytree(generator.DATA, folder)
            for name in ["index-big5.txt", "index-jis0208.txt", "index-jis0212.txt"]:
                with self.subTest(name=name):
                    path = folder / name
                    original = path.read_bytes()
                    path.write_bytes(original + b"\n")
                    with self.assertRaisesRegex(ValueError, "Pinned multibyte input mismatch"):
                        generator.read_inputs(folder)
                    path.write_bytes(original)

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
