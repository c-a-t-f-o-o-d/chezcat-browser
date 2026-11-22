"""Original adapter controls. These do not count as product conformance cases."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import foundation_corpus as f

class FoundationControls(unittest.TestCase):
    def case(self,expected='| "A"\n'):
        return f.Case('control','tree','original','',1,'A',expected,entry='fragment',context='div')
    def test_wrong_tree_is_product_failure(self):
        result=f.classify(self.case(),dict(status='tree',value='| "wrong"\n',diagnostics='diagnostics-unavailable'))
        self.assertEqual(result['outcome'],'fail')
    def test_diagnostic_omission_is_not_a_pass(self):
        case=self.case();case.errors={'count':0}
        result=f.classify(case,dict(status='tree',value=case.expected,diagnostics='diagnostics-unavailable'))
        self.assertEqual(result['outcome'],'harness_blocked')
        self.assertEqual(result['comparison'],'pass')
    def test_malformed_fixture_is_not_a_product_failure(self):
        with self.assertRaises(f.FixtureError):f.tree_cases('broken.dat',b'#data\nA\n#document\n| "A"\n')
    def test_byte_newlines_and_mode_expansion_are_preserved(self):
        cases=f.tree_cases('control.dat',b'#data\nA\r\nB\n#errors\n#document-fragment\ndiv\n#document\n| "A\nB"\n')
        self.assertEqual([c.scripting for c in cases],['off','on'])
        self.assertEqual(cases[0].source,'A\r\nB')
        self.assertEqual(cases[0].expected,'| "A\nB"\n')
    def test_tokenizer_double_escapes_preserve_code_units(self):
        data=b'{"tests":[{"description":"original","input":"\\\\uD800","output":[["Character","\\\\uD800"]],"doubleEscaped":true,"initialStates":["Data state","RCDATA state"]}]}'
        cases=f.tokenizer_cases('control.test',data)
        self.assertEqual(len(cases),2)
        self.assertEqual(ord(cases[0].source),0xd800)
    def test_missing_input_is_owned_harness_block(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(f.FixtureError):
                f.checked_inputs(Path(folder).resolve(),[dict(repository='original',path='missing.dat',bytes=1,sha256='0'*64)])
    def test_missing_worker_is_not_product_failure(self):
        with tempfile.TemporaryDirectory() as folder:
            self.assertEqual(f.execute(Path(folder)/'missing.exe',self.case(),Path(folder))['outcome'],'harness_blocked')
    def test_timeout_and_nonzero_exit_have_distinct_categories(self):
        try:
            subprocess.run([sys.executable,'-c','import time;time.sleep(1)'],timeout=.01,capture_output=True)
            self.fail('control must time out')
        except subprocess.TimeoutExpired:
            result=f.classify(self.case(),dict(outcome='timeout',reason='controlled deadline'))
            self.assertEqual(result['outcome'],'timeout')
        result=subprocess.run([sys.executable,'-c','import os;os._exit(17)'],capture_output=True)
        self.assertEqual(result.returncode,17)
        self.assertEqual(f.classify(self.case(),dict(outcome='crash',exit_code=result.returncode))['outcome'],'crash')

if __name__=='__main__':unittest.main()
