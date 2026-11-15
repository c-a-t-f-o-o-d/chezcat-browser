"""Focused standard-library checks for package/Windows version identity.

No PE execution, image/Pillow import, resource generation or canonical edits.
"""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_windows_pe as checker


class PackageVersion(unittest.TestCase):
    def test_alpha_keeps_complete_string_and_maps_only_numeric_core(self):
        self.assertEqual(checker.parse_package_version('0.79.0-alpha.1'),
                         ('0.79.0-alpha.1', (0, 79, 0, 0)))

    def test_nonzero_patch_and_prerelease_are_not_truncated(self):
        self.assertEqual(checker.parse_package_version('1.23.7-rc.2'),
                         ('1.23.7-rc.2', (1, 23, 7, 0)))
        self.assertEqual(checker.parse_package_version('1.23.7'),
                         ('1.23.7', (1, 23, 7, 0)))

    def test_new_stable_zero_patch_keeps_three_components(self):
        self.assertEqual(checker.parse_package_version('0.79.0'),
                         ('0.79.0', (0, 79, 0, 0)))

    def test_current_historical_baseline_remains_compatible(self):
        self.assertEqual(checker.parse_package_version('0.78.0'),
                         ('0.78', (0, 78, 0, 0)))
        self.assertEqual(checker.parse_package_version('0.78.0-alpha.1'),
                         ('0.78.0-alpha.1', (0, 78, 0, 0)))

    def test_full_build_metadata_is_preserved_for_static_inspection(self):
        self.assertEqual(checker.parse_package_version('1.2.3-alpha.1+build.00042'),
                         ('1.2.3-alpha.1+build.00042', (1, 2, 3, 0)))
        self.assertEqual(checker.parse_package_version('1.2.3+00042'),
                         ('1.2.3+00042', (1, 2, 3, 0)))

    def test_largest_windows_words_are_accepted(self):
        self.assertEqual(checker.parse_package_version('65535.65535.65535-alpha.1'),
                         ('65535.65535.65535-alpha.1', (65535, 65535, 65535, 0)))

    def test_each_windows_word_overflow_is_refused(self):
        for version in ['65536.1.1', '1.65536.1', '1.1.65536', '9' * 5000 + '.0.0']:
            with self.subTest(version=version[:32]):
                with self.assertRaisesRegex(ValueError, 'Windows limits'):
                    checker.parse_package_version(version)

    def test_numeric_prerelease_zero_rule(self):
        self.assertEqual(checker.parse_package_version('1.2.3-0.alpha.0'),
                         ('1.2.3-0.alpha.0', (1, 2, 3, 0)))
        for version in ['1.2.3-00', '1.2.3-alpha.01', '1.2.3-01.rc']:
            with self.subTest(version=version):
                with self.assertRaisesRegex(ValueError, 'leading zeroes'):
                    checker.parse_package_version(version)

    def test_malformed_core_and_identifiers_are_refused(self):
        for version in ['0.79', 'v0.79.0', '01.2.3', '1.02.3', '1.2.03',
                        '1.2.3-', '1.2.3-alpha..1', '1.2.3-alpha_', '1.2.3+',
                        '1.2.3+build..1', '1.2.3+build_', '1.2.3\n', '1.2.3-α']:
            with self.subTest(version=version):
                with self.assertRaises(ValueError):
                    checker.parse_package_version(version)

    def test_nonstring_package_values_are_refused(self):
        for value in [None, 79, True, {'workspace': True}]:
            with self.subTest(value=value):
                with self.assertRaisesRegex(ValueError, 'string package version'):
                    checker.parse_package_version(value)

    def manifest_version(self, text):
        with tempfile.TemporaryDirectory(prefix='chezcat-version-parser-') as temporary:
            root = Path(temporary)
            (root / 'Cargo.toml').write_text(text, encoding='utf-8')
            with patch.object(checker, 'PROJECT_ROOT', root):
                return checker.package_version()

    def test_actual_package_table_and_inline_comment_are_read(self):
        manifest = '[package]\nname = "fixture"\nversion = "0.79.3-alpha.2" # chosen milestone\n\n[dependencies.example]\nversion = "99.99.99"\n'
        self.assertEqual(self.manifest_version(manifest),
                         ('0.79.3-alpha.2', (0, 79, 3, 0)))

    def test_missing_duplicate_or_wrong_table_cannot_supply_package_identity(self):
        for manifest in ['[dependencies.fixture]\nversion = "1.2.3"\n',
                         '[package]\nname = "fixture"\n',
                         '[package]\nversion = "1.2.3"\nversion = "4.5.6"\n']:
            with self.subTest(manifest=manifest):
                with self.assertRaises(ValueError):
                    self.manifest_version(manifest)


if __name__ == '__main__':
    unittest.main(verbosity=2)
