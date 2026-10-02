import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest
from types import SimpleNamespace

spec = importlib.util.spec_from_file_location('agent_run', Path(__file__).with_name('run.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class CorpusTests(unittest.TestCase):
    def test_current_and_missing_input(self):
        self.assertEqual(len(runner.cases()), 8)
        with tempfile.TemporaryDirectory() as temporary:
            suite = Path(temporary) / 'suite'
            shutil.copytree(runner.HERE / 'fixtures', suite / 'fixtures')
            shutil.copytree(runner.HERE / 'oracles', suite / 'oracles')
            (suite / 'fixtures/case-01/source.rs').unlink()
            with self.assertRaisesRegex(ValueError, 'case-01: missing input'):
                runner.cases(suite)

    def test_input_escape_and_unregistered_answer(self):
        with tempfile.TemporaryDirectory() as temporary:
            suite = Path(temporary) / 'suite'
            shutil.copytree(runner.HERE / 'fixtures', suite / 'fixtures')
            shutil.copytree(runner.HERE / 'oracles', suite / 'oracles')
            case = suite / 'fixtures/case-01/case.json'
            value = runner.read(case)
            value['inputs'] = ['../../oracles/case-01.json']
            runner.write(case, value)
            with self.assertRaisesRegex(ValueError, 'outside treatment'):
                runner.cases(suite)
            value['inputs'] = ['source.rs']
            runner.write(case, value)
            (case.parent / 'answer.md').write_text('Evaluator answer')
            with self.assertRaisesRegex(ValueError, 'unregistered treatment'):
                runner.cases(suite)

    def test_manual_atoms_and_hard_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'run'
            runner.prepare(SimpleNamespace(run=path, condition='baseline', model='test-model', tools='test-tools'))
            with self.assertRaises(FileNotFoundError):
                runner.score(path)
            judgments = runner.read(path / 'judgments.json')
            for case, oracle, _ in runner.cases():
                ident = case['id']
                (path / 'responses' / (ident + '.md')).write_text('Synthetic harness input, not model evidence')
                for item in judgments[ident]['atoms'].values():
                    item.update({'pass': True, 'evidence': 'synthetic adjudicator evidence'})
                for item in judgments[ident]['hard_failures'].values():
                    item.update({'triggered': False, 'evidence': 'synthetic adjudicator evidence'})
            runner.write(path / 'judgments.json', judgments)
            self.assertTrue(all(c['pass'] for c in runner.score(path)['cases'].values()))
            judgments['case-01']['hard_failures']['h1']['triggered'] = True
            runner.write(path / 'judgments.json', judgments)
            self.assertFalse(runner.score(path)['cases']['case-01']['pass'])
            judgments['case-01']['atoms']['a1']['pass'] = None
            runner.write(path / 'judgments.json', judgments)
            with self.assertRaisesRegex(ValueError, 'unadjudicated'):
                runner.score(path)

    def test_frozen_packages(self):
        for directory in (runner.HERE / 'frozen-skills').iterdir():
            runner.frozen(directory.name)
        self.assertEqual(runner.digest({'b': b'2', 'a': b'1'}), runner.digest({'a': b'1', 'b': b'2'}))
        self.assertNotEqual(runner.digest({'a': b'1'}), runner.digest({'a': b'2'}))


if __name__ == '__main__':
    unittest.main()
