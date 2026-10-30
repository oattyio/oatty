"""Fail-closed licensing checks, plus the actual npm/release packaging contracts."""
import importlib.util
import json
import hashlib
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import licenses

spec = importlib.util.spec_from_file_location('package_release', SCRIPTS / 'package-release.py')
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class LicensingFailures(unittest.TestCase):
    def test_text_inputs_are_stable_across_checkout_newlines(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ('LICENSE', 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json',
                         'about.toml', 'scripts/licenses.py', 'scripts/license-supplements/reviewed.json',
                         'crates/example/Cargo.toml'):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'line one\nline two\n')
            before = licenses.inputs(root)
            for name in before:
                (root / name).write_bytes((root / name).read_bytes().replace(b'\n', b'\r\n'))
            self.assertEqual(before, licenses.inputs(root))

    def test_generator_failure_aborts_without_publishing_outputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(licenses.subprocess, 'check_output', side_effect=[
                    'cargo-about 0.9.2', subprocess.CalledProcessError(1, ['cargo-about', 'generate'])]):
                with self.assertRaises(subprocess.CalledProcessError):
                    licenses.prepare(root)
            self.assertEqual([], list(root.iterdir()))

    def test_unknown_generic_template_and_code_named_copying_are_not_notices(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'copying.rs').write_text('pub fn copy() {}')
            notices = licenses.copy_notices(root, root / 'output')
            self.assertEqual([], notices)
            with self.assertRaisesRegex(ValueError, 'Missing reviewed upstream notices'):
                licenses.validate_attribution({'name': 'unknown', 'version': '1'}, notices, root, True, False)
            (root / 'LICENSE').write_text('Copyright (c) <year> <copyright holders>')
            with self.assertRaisesRegex(ValueError, 'Generic license template lacks reviewed attribution'):
                licenses.validate_attribution({'name': 'unknown', 'version': '1'}, ['LICENSE'], root, True, False)

    def test_changed_npm_dependency_spec_requires_updated_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'package-lock.json').write_text(json.dumps({'packages': {'': {}}}))
            for key in ('dependencies', 'optionalDependencies'):
                with self.subTest(key=key):
                    (root / 'package.json').write_text(json.dumps({key: {'new': '^1'}}))
                    with self.assertRaisesRegex(ValueError, 'Stale npm package-lock root metadata'):
                        licenses.npm_notices(root, root / 'output')

    def test_missing_selected_license_text(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, 'Missing selected license text'):
                licenses.rust_notices({'licenses': [{'text': ''}], 'crates': []}, Path(temporary), {'package': []})

    def test_unresolved_crate(self):
        report = {'licenses': [], 'crates': [{'package': {'name': 'unknown', 'version': '1', 'license': None}}]}
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, 'Unresolved licensing'):
                licenses.rust_notices(report, Path(temporary), {'package': []})

    def test_missing_required_native_notice(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, 'Missing required upstream notice'):
                licenses.copy_notices(Path(temporary), Path(temporary) / 'out', ('aws-lc/LICENSE',))

    def test_exact_source_path_rejects_wrong_case_on_every_filesystem(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'readme.md').write_text('original upstream name')
            self.assertEqual(root / 'readme.md', licenses.exact_source_path(root, 'readme.md'))
            with self.assertRaisesRegex(ValueError, 'incorrect case'):
                licenses.exact_source_path(root, 'README.md')

    def test_reviewed_supplement_checksum_and_crate_binding(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = {'name': 'example', 'version': '1', 'manifest_path': str(root / 'Cargo.toml')}
            record = {'example@1': {'crate-checksum': 'expected', 'files': [
                {'file': 'LICENSE.txt', 'sha256': '0' * 64}]}}
            with self.assertRaisesRegex(ValueError, 'Stale reviewed license evidence'):
                licenses.preserve_supplement(package, {'checksum': 'changed'}, root, record, root)
            (root / 'LICENSE.txt').write_text('changed upstream attribution')
            with self.assertRaisesRegex(ValueError, 'Reviewed license supplement checksum mismatch'):
                licenses.preserve_supplement(package, {'checksum': 'expected'}, root, record, root)

    def test_source_checksum_mismatch(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / 'source.crate'
            source.write_bytes(b'changed covered source')
            with self.assertRaisesRegex(ValueError, 'Source checksum mismatch'):
                licenses.bundle_source(source, Path(temporary) / 'out.crate', '0' * 64)

    def test_npm_unknown_license_and_missing_text(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'package.json').write_text('{}')
            package = root / 'node_modules/example'
            package.mkdir(parents=True)
            for license_id, error in [('UNKNOWN', 'Unresolved'), ('MIT', 'Missing npm license text')]:
                metadata = {'name': 'example', 'version': '1', 'license': license_id}
                (package / 'package.json').write_text(json.dumps(metadata))
                (root / 'package-lock.json').write_text(json.dumps({'packages': {'': {}, 'node_modules/example': metadata}}))
                with self.assertRaisesRegex(ValueError, error):
                    licenses.npm_notices(root, root / 'output')

    def test_missing_stale_and_changed_materials(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in licenses.GENERATED:
                if '.' in name:
                    (root / name).write_text('notices')
                else:
                    (root / name).mkdir()
            source = root / 'third-party-sources/example.crate'
            source.write_bytes(b'covered source')
            manifest = {'inputs': {'lock': 'old'}, 'files': licenses.inventory(root)}
            (root / licenses.MANIFEST).write_text(json.dumps(manifest))
            with patch.object(licenses, 'inputs', return_value={'lock': 'new'}):
                with self.assertRaisesRegex(ValueError, 'Stale licensing inputs'):
                    licenses.verify(root)
            with patch.object(licenses, 'inputs', return_value={'lock': 'old'}):
                licenses.verify(root)
                source.write_bytes(b'tampered source')
                with self.assertRaisesRegex(ValueError, 'changed licensing output/source'):
                    licenses.verify(root)
                (root / 'THIRD-PARTY-NOTICES.txt').unlink()
                with self.assertRaisesRegex(ValueError, 'Missing generated material'):
                    licenses.verify(root)


class DistributionContracts(unittest.TestCase):
    def test_every_reviewed_source_path_and_hash_matches_exact_crate_archive_member(self):
        metadata = json.loads(subprocess.check_output(
            ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline'], cwd=licenses.ROOT))
        packages = {f"{p['name']}@{p['version']}": p for p in metadata['packages']}
        records = json.loads((SCRIPTS / 'license-supplements/reviewed.json').read_text())
        for identity, record in records.items():
            package = packages[identity]
            source = Path(package['manifest_path']).parent
            crate_name = f"{package['name']}-{package['version']}"
            archive = source.parents[1] / '..' / 'cache' / source.parent.name / f'{crate_name}.crate'
            with tarfile.open(archive.resolve()) as published:
                names = set(published.getnames())
                for evidence in record['files']:
                    if 'source-path' not in evidence:
                        continue
                    with self.subTest(package=identity, path=evidence['source-path']):
                        member = f"{crate_name}/{evidence['source-path']}"
                        self.assertIn(member, names)
                        data = published.extractfile(member).read()
                        self.assertEqual(evidence['source-sha256'], hashlib.sha256(data).hexdigest())

    def test_fxhash_real_copyright_and_reviewed_upstream_grants_are_distributed(self):
        evidence = licenses.ROOT / 'third-party-licenses/rust'
        fxhash = (evidence / 'fxhash-0.2.1/fxhash-0.2.1-header.txt').read_text()
        self.assertIn('Copyright 2015 The Rust Project Developers.', fxhash)
        self.assertIn('Copyright (c) 2016 Dropbox, Inc.',
                      (evidence / 'alloc-stdlib-0.3.0/alloc-stdlib-0.3.0-LICENSE.txt').read_text())
        self.assertIn('Apple SDKs', (evidence / 'objc2-foundation-0.3.2/objc2-LICENSE.md').read_text())
        self.assertIn('Copyright 2013 Google Inc.',
                      (evidence / 'brotli-decompressor-6.0.1/brotli-decompressor-6.0.1-src-context.rs-header.txt').read_text())
        self.assertIn('John Nagle',
                      (evidence / 'chrono-0.4.45/chrono-0.4.45-src-format-parse.rs-header.txt').read_text())
        self.assertIn('license = "MIT/Apache-2.0"',
                      (evidence / 'rat-focus-2.1.1/rat-focus-2.1.1-Cargo.toml.orig.txt').read_text())

    def test_all_release_archives_keep_binary_at_root_and_legal_materials(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            binary = directory / 'signed-binary'
            binary.write_bytes(b'unchanged signed executable\x00\xff')
            binary.chmod(0o755)
            for target in release.TARGETS:
                with self.subTest(target=target):
                    archive = release.package(licenses.ROOT, binary, target, 'v0.0.0', directory)
                    self.assertTrue(archive.is_file())  # package() reads back every file and the executable bytes.

    def test_npm_prepack_rejects_stale_lock_and_missing_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            root = directory / 'fixture'
            (root / 'scripts').mkdir(parents=True)
            for name in ('LICENSE', 'Cargo.toml', 'Cargo.lock', 'package-lock.json', 'about.toml'):
                (root / name).write_text('fixture')
            (root / 'scripts/licenses.py').write_bytes((SCRIPTS / 'licenses.py').read_bytes())
            (root / 'package.json').write_text(json.dumps({
                'name': 'licensing-guard-fixture', 'version': '1.0.0',
                'scripts': {'prepack': 'python3 scripts/licenses.py verify'}}))
            (root / 'THIRD-PARTY-NOTICES.txt').write_text('fixture notices')
            (root / 'third-party-licenses').mkdir()
            (root / 'third-party-sources').mkdir()
            source = root / 'third-party-sources/source.crate'
            source.write_bytes(b'fixture source')
            manifest = {'inputs': licenses.inputs(root), 'files': licenses.inventory(root)}
            (root / licenses.MANIFEST).write_text(json.dumps(manifest))
            command = ['npm', 'pack', '--json', '--cache', str(directory / 'cache'),
                       '--pack-destination', str(directory)]
            (root / 'Cargo.lock').write_text('changed dependency lock')
            stale = subprocess.run(command, cwd=root, text=True, capture_output=True)
            self.assertNotEqual(0, stale.returncode)
            self.assertIn('Stale licensing inputs', stale.stderr)
            (root / 'Cargo.lock').write_text('fixture')
            source.unlink()
            missing = subprocess.run(command, cwd=root, text=True, capture_output=True)
            self.assertNotEqual(0, missing.returncode)
            self.assertIn('Missing or changed licensing output/source', missing.stderr)

    def test_actual_npm_tarball_contains_notices_source_and_installer(self):
        with tempfile.TemporaryDirectory() as temporary:
            packed = subprocess.check_output(['npm', 'pack', '--json', '--cache', str(Path(temporary) / 'cache'), '--pack-destination', temporary], cwd=licenses.ROOT, text=True)
            filename = json.loads(packed)[0]['filename']
            with tarfile.open(Path(temporary) / filename) as archive:
                names = set(archive.getnames())
                expected = {'package/LICENSE', 'package/THIRD-PARTY-NOTICES.txt',
                            'package/npm/bin/oatty.js', 'package/npm/scripts/postinstall.js'}
                expected.update('package/' + name for name in licenses.inventory(licenses.ROOT))
                self.assertTrue(expected.issubset(names), expected - names)
                self.assertFalse(any('/node_modules/' in name for name in names))


if __name__ == '__main__':
    unittest.main()
