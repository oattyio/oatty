"""Exercise packed, script-free npm installations against a private loopback registry.

NPM_CLI selects an npm-cli.js (invoked with Node). Release jobs can additionally
set OATTY_NPM_TEST_BINARY and OATTY_NPM_TEST_TARGET to install their unchanged
signed native executable and run --version and update --check through the launcher.
"""
import http.server
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import tarfile
import io
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch
from urllib.parse import unquote

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import licenses


def load_script(name):
    """Import a release CLI with a hyphenated filename for direct contract tests."""
    specification = importlib.util.spec_from_file_location(name.replace('-', '_'), SCRIPTS / f'{name}.py')
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


packaging = load_script('package-npm')
publishing = load_script('publish-npm')
FIXTURE = b'''#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
if (args[0] === '--version') { console.log('oatty ' + fs.readFileSync(require('node:path').join(__dirname, '.oatty-installed-version'), 'utf8').trim()); }
else if (args[0] === 'update' && args[1] === '--check') {
  const version = fs.readFileSync(require('node:path').join(__dirname, '.oatty-installed-version'), 'utf8').trim();
  console.log('Installed version: ' + version);
} else if (args[0] === 'status') { process.exit(Number(args[1])); }
else if (args[0] === 'signal') { process.kill(process.pid, args[1]); }
else if (args[0] === 'wait') { console.log('ready'); setInterval(() => {}, 1000); }
else { console.log(JSON.stringify({args, cwd:process.cwd(), environment:process.env.OATTY_FIXTURE_VALUE})); }
'''


def fixture_root(destination):
    """Create complete legal fixtures bound to the real packaging inputs, without Cargo."""
    source = packaging.ROOT
    for name in licenses.inputs(source):
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / name, path)
    for name in ('npm/bin/oatty.js', 'npm/platforms.json', 'README.md'):
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / name, path)
    (destination / 'THIRD-PARTY-NOTICES.txt').write_bytes(b'Complete fixture notice\n')
    (destination / 'third-party-licenses/example').mkdir(parents=True)
    (destination / 'third-party-licenses/example/LICENSE').write_bytes(b'Fixture upstream license\n')
    (destination / 'third-party-sources').mkdir()
    (destination / 'third-party-sources/example.crate').write_bytes(b'Fixture covered source\x00\xff')
    refresh_license_manifest(destination)


def refresh_license_manifest(root):
    """Bind fixture materials exactly as the real license verifier expects."""
    (root / licenses.MANIFEST).write_text(json.dumps({'inputs': licenses.inputs(root), 'files': licenses.inventory(root)}))


class Registry(http.server.BaseHTTPRequestHandler):
    """Serve original packed bytes and normal npm packuments, recording every request."""
    def log_message(self, *_):
        pass

    def do_GET(self):
        name = unquote(self.path).split('?', 1)[0].lstrip('/')
        self.server.requests.append(name)
        if name.startswith('tarballs/'):
            archive = self.server.archives.get(name.removeprefix('tarballs/'))
            if archive:
                return self.respond(200, archive.read_bytes(), 'application/octet-stream')
        package = self.server.packages.get(name)
        if package:
            return self.respond(200, json.dumps(package).encode(), 'application/json')
        return self.respond(404, b'{"error":"not found"}', 'application/json')

    def respond(self, status, data, content_type):
        self.send_response(status)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def start_registry(archives):
    """Start a local registry with exact-version metadata for unchanged tarballs."""
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Registry)
    server.requests, server.archives, server.packages = [], {}, {}
    address = f'http://127.0.0.1:{server.server_port}'
    for archive in archives:
        contents, _ = packaging.read_artifact(archive)
        manifest = json.loads(contents['package.json'])
        manifest['dist'] = {'tarball': f'{address}/tarballs/{archive.name}',
                            'integrity': publishing.artifact_integrity(archive)}
        server.archives[archive.name] = archive
        server.packages[manifest['name']] = {'name': manifest['name'], 'dist-tags': {'latest': manifest['version']},
                                            'versions': {manifest['version']: manifest}}
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, address


class NpmDistribution(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix='oatty-npm-tests-')
        cls.base = Path(cls.temporary.name)
        cls.root = cls.base / 'source'
        cls.root.mkdir()
        fixture_root(cls.root)
        cls.binary = cls.base / 'oatty'
        cls.binary.write_bytes(Path(shutil.which('node')).read_bytes() if os.name == 'nt' else FIXTURE)
        cls.binary.chmod(0o755)
        cls.platforms = packaging.load_platforms(cls.root)
        cls.version = packaging.root_manifest(cls.root)['version']
        cls.destination = cls.base / 'dist'
        cls.archives = [packaging.package_binary(cls.root, cls.binary, platform['target'], cls.destination)
                        for platform in cls.platforms.values()]
        cls.umbrella = packaging.package_umbrella(cls.root, cls.destination)
        cls.archives.append(cls.umbrella)
        cls.server, cls.address = start_registry(cls.archives)
        cls.host = subprocess.check_output(['node', '-p', 'process.platform+"-"+process.arch'], text=True).strip()
        cls.installs = {}
        if cls.host in cls.platforms:
            for globally in (False, True):
                cls.installs[globally] = cls.install(globally)

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.temporary.cleanup()

    @classmethod
    def install(cls, globally, omit=False, archive=None, label=None, platform_flags=()):
        """Install a real root tarball against the loopback registry and an empty cache."""
        prefix = cls.base / (label or ('global' if globally else 'local') + ('-omit' if omit else ''))
        cache = cls.base / (prefix.name + '-cache')
        config = cls.base / 'empty.npmrc'
        config.write_text('')
        command = [*packaging.npm_command(), 'install', '--ignore-scripts', '--prefix', str(prefix),
                   '--cache', str(cache), '--registry', cls.address, '--no-audit', '--no-fund',
                   '--userconfig', str(config), '--globalconfig', str(cls.base / 'empty-global.npmrc')]
        if globally:
            command.append('--global')
        if omit:
            command.append('--omit=optional')
        command.extend(platform_flags)
        command.append(str(archive or cls.umbrella))
        result = subprocess.run(command, capture_output=True, text=True)
        if result.returncode:
            raise AssertionError(f"{' '.join(command)}\n{result.stdout}\n{result.stderr}")
        module = prefix / ('lib/node_modules' if globally and os.name != 'nt' else 'node_modules') / 'oatty'
        return module

    def launcher(self, module, *arguments, **kwargs):
        """Run an installed launcher, retaining stdout/stderr and the real exit status."""
        return subprocess.run(['node', str(module / 'npm/bin/oatty.js'), *arguments],
                              capture_output=True, text=True, timeout=30, **kwargs)

    def run_shim(self, module, globally, *arguments):
        if globally:
            executable = module.parent.parent / 'oatty.cmd' if os.name == 'nt' else module.parents[2] / 'bin/oatty'
        else:
            executable = module.parent / '.bin' / ('oatty.cmd' if os.name == 'nt' else 'oatty')
        command = ['cmd', '/d', '/c', str(executable), *arguments] if os.name == 'nt' else [str(executable), *arguments]
        return subprocess.run(command, capture_output=True, text=True, timeout=30)

    def native_directory(self, module):
        nested = module / 'node_modules' / self.platforms[self.host]['package']
        return nested if nested.exists() else module.parent / self.platforms[self.host]['package']

    def fixture_module(self):
        if not self.installs or os.name == 'nt':
            self.skipTest('POSIX fixture script behavior; Windows covers real Node/native payload installs')
        return self.installs[False]

    def test_all_four_metadata_legal_bytes_modes_and_determinism(self):
        for platform, archive in zip(self.platforms.values(), self.archives):
            with self.subTest(platform=platform['target']):
                contents = packaging.verify_artifact(self.root, archive, platform)
                self.assertEqual(self.binary.read_bytes(), contents[f"bin/{platform['binary']}"])
                again = packaging.package_binary(self.root, self.binary, platform['target'], self.base / 'repeat')
                self.assertEqual(archive.read_bytes(), again.read_bytes())
        packaging.verify_artifact(self.root, self.umbrella)
        repeated = packaging.package_umbrella(self.root, self.base / 'repeat')
        self.assertEqual(self.umbrella.read_bytes(), repeated.read_bytes())

    def test_registry_reads_exact_metadata_and_distinguishes_e404(self):
        environment = {'npm_config_registry': self.address,
                       'npm_config_cache': str(self.base / 'view-cache')}
        with patch.dict(os.environ, environment):
            registry = publishing.registry_manifest('oatty', self.version)
            self.assertEqual(self.version, registry['version'])
            self.assertEqual(publishing.artifact_integrity(self.umbrella), registry['dist']['integrity'])
            self.assertIsNone(publishing.registry_manifest('@oattyio/not-a-real-package', self.version))

    def test_windows_mode_normalization_preserves_every_file_byte(self):
        archive = self.base / 'windows-mode.tgz'
        with tarfile.open(self.archives[0], 'r:gz') as original:
            with tarfile.open(archive, 'w:gz') as modified:
                for member in original.getmembers():
                    if member.name == 'package/bin/oatty':
                        member.mode = 0o644
                    content = original.extractfile(member).read() if member.isfile() else None
                    modified.addfile(member, io.BytesIO(content) if content is not None else None)
        before, _ = packaging.read_artifact(archive)
        packaging.normalize_executable_mode(archive, 'bin/oatty')
        after, modes = packaging.read_artifact(archive)
        self.assertEqual(before, after)
        self.assertEqual(0o755, modes['bin/oatty'])
        packaging.verify_artifact(self.root, archive, next(iter(self.platforms.values())))

    def test_first_party_crlf_checkout_produces_identical_archives(self):
        names = ('LICENSE', 'npm/bin/oatty.js', 'npm/platforms.json', 'README.md')
        originals = {name: (self.root / name).read_bytes() for name in names}
        try:
            for name, data in originals.items():
                (self.root / name).write_bytes(data.replace(b'\r\n', b'\n').replace(b'\n', b'\r\n'))
            directory = self.base / 'crlf-dist'
            umbrella = packaging.package_umbrella(self.root, directory)
            self.assertEqual(self.umbrella.read_bytes(), umbrella.read_bytes())
            platform = next(iter(self.platforms.values()))
            native = packaging.package_binary(self.root, self.binary, platform['target'], directory)
            self.assertEqual(self.archives[0].read_bytes(), native.read_bytes())
        finally:
            for name, data in originals.items():
                (self.root / name).write_bytes(data)

    def test_libc_selection_installs_glibc_payload_and_omits_it_on_musl(self):
        for libc in ('glibc', 'musl'):
            module = self.install(False, label='libc-' + libc,
                                  platform_flags=('--os=linux', '--cpu=x64', '--libc=' + libc))
            for key, platform in self.platforms.items():
                locations = (module / 'node_modules' / platform['package'], module.parent / platform['package'])
                self.assertEqual(libc == 'glibc' and key == 'linux-x64', any(path.exists() for path in locations),
                                 (libc, key))

    def test_source_lock_bootstraps_without_registry_or_scripts(self):
        work = self.base / 'offline-bootstrap'
        work.mkdir()
        for name in ('package.json', 'package-lock.json'):
            shutil.copyfile(self.root / name, work / name)
        result = subprocess.run([*packaging.npm_command(), 'ci', '--omit=optional', '--ignore-scripts',
                                 '--offline', '--cache', str(self.base / 'offline-cache')],
                                cwd=work, capture_output=True, text=True)
        self.assertEqual(0, result.returncode, result.stderr)
        locked = json.loads((work / 'package-lock.json').read_text())['packages']
        self.assertEqual(5, len(locked))
        for platform in self.platforms.values():
            entry = locked['node_modules/' + platform['package']]
            self.assertEqual(self.version, entry['version'])
            self.assertTrue(entry['optional'])
            self.assertEqual([platform['os']], entry['os'])
            self.assertEqual([platform['cpu']], entry['cpu'])
            self.assertEqual([platform['libc']] if 'libc' in platform else None, entry.get('libc'))
            self.assertNotIn('integrity', entry)

    def test_local_global_ignore_scripts_installs_run_unchanged_tarballs(self):
        self.assertTrue(self.installs, 'test host needs a supported native package')
        for globally, module in self.installs.items():
            with self.subTest(globally=globally):
                self.assertEqual(subprocess.check_output(['node', '--version'], text=True).strip() if os.name == 'nt' else 'oatty ' + self.version,
                                 self.launcher(module, '--version').stdout.strip())
                if os.name != 'nt':
                    check = self.launcher(module, 'update', '--check')
                    self.assertEqual(0, check.returncode, check.stderr)
                    self.assertIn('Installed version: ' + self.version, check.stdout)
                native = self.native_directory(module) / 'bin' / self.platforms[self.host]['binary']
                self.assertEqual(self.binary.read_bytes(), native.read_bytes())
                for key, platform in self.platforms.items():
                    locations = (module / 'node_modules' / platform['package'], module.parent / platform['package'])
                    self.assertEqual(key == self.host, any(path.exists() for path in locations), key)
                self.assertEqual(0, self.run_shim(module, globally, '--version').returncode)
        self.assertTrue(any(name.startswith('tarballs/oattyio-cli-') for name in self.server.requests))

    def test_arguments_cwd_environment_and_status_are_preserved(self):
        module = self.fixture_module()
        arguments = ('one two', 'quote"', '--flag=x', '💚')
        result = self.launcher(module, *arguments, cwd=self.base, env={**os.environ, 'OATTY_FIXTURE_VALUE': 'forwarded'})
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual({'args': list(arguments), 'cwd': str(self.base.resolve()), 'environment': 'forwarded'}, json.loads(result.stdout))
        self.assertEqual(37, self.launcher(module, 'status', '37').returncode)

    def test_child_and_parent_directed_signals_preserve_status(self):
        module = self.fixture_module()
        for name in ('SIGINT', 'SIGTERM'):
            with self.subTest(signal=name):
                number = getattr(signal, name)
                self.assertEqual(-number, self.launcher(module, 'signal', name).returncode)
                process = subprocess.Popen(['node', str(module / 'npm/bin/oatty.js'), 'wait'],
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                try:
                    self.assertEqual('ready', process.stdout.readline().strip())
                    process.send_signal(number)
                    process.communicate(timeout=10)
                    self.assertEqual(-number, process.returncode)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()

    def test_omit_optional_gives_reinstall_guidance(self):
        module = self.install(False, omit=True)
        result = self.launcher(module, '--version')
        self.assertEqual(1, result.returncode)
        self.assertIn('--include=optional', result.stderr)
        self.assertIn(self.platforms[self.host]['package'] + '@' + self.version, result.stderr)

    def test_rejects_mismatch_unsupported_musl_and_missing_marker(self):
        module = self.fixture_module()
        metadata_path = self.native_directory(module) / 'package.json'
        original = metadata_path.read_bytes()
        manifest = json.loads(original)
        for key, value in [('version', '0.0.0'), ('os', ['unsupported']), ('cpu', ['unsupported']), ('oattyTarget', 'wrong')]:
            with self.subTest(field=key):
                modified = dict(manifest, **{key: value})
                metadata_path.write_text(json.dumps(modified))
                try:
                    result = self.launcher(module)
                    self.assertEqual(1, result.returncode)
                    self.assertIn(f'metadata: {key}', result.stderr)
                    self.assertIn('--include=optional', result.stderr)
                finally:
                    metadata_path.write_bytes(original)
        marker = metadata_path.parent / 'bin/.oatty-installed-version'
        original_marker = marker.read_bytes()
        marker.unlink()
        try:
            self.assertEqual(1, self.launcher(module).returncode)
        finally:
            marker.write_bytes(original_marker)
        for setup, expected in [
                ("Object.defineProperty(process,'arch',{value:'riscv64'});", 'Unsupported platform'),
                ("Object.defineProperty(process,'platform',{value:'linux'});Object.defineProperty(process,'arch',{value:'x64'});"
                 "process.report.getReport=()=>({header:{}});", 'musl')]:
            result = subprocess.run(['node', '-e', setup + f'require({json.dumps(str(module / "npm/bin/oatty.js"))});'],
                                    capture_output=True, text=True)
            self.assertEqual(1, result.returncode)
            self.assertIn(expected, result.stderr)

    def test_stale_legal_material_aborts_before_npm_pack(self):
        notices = self.root / 'THIRD-PARTY-NOTICES.txt'
        original = notices.read_bytes()
        notices.write_bytes(b'changed')
        try:
            with patch.object(packaging, 'pack_stage') as pack:
                with self.assertRaisesRegex(ValueError, 'Missing or changed'):
                    packaging.package_umbrella(self.root, self.base / 'invalid')
                pack.assert_not_called()
        finally:
            notices.write_bytes(original)

    def test_release_native_binary_installs_and_reports_marker(self):
        binary = os.environ.get('OATTY_NPM_TEST_BINARY')
        if not binary:
            self.skipTest('Set OATTY_NPM_TEST_BINARY and OATTY_NPM_TEST_TARGET for release native smoke')
        target = os.environ.get('OATTY_NPM_TEST_TARGET')
        self.assertEqual(self.platforms[self.host]['target'], target)
        directory = self.base / 'native-dist'
        archives = [packaging.package_binary(packaging.ROOT, Path(binary), target, directory),
                    packaging.package_umbrella(packaging.ROOT, directory)]
        server, address = start_registry(archives)
        original_address = self.address
        try:
            self.__class__.address = address
            for globally in (False, True):
                module = self.install(globally, archive=archives[1], label=f'native-{globally}')
                version = self.launcher(module, '--version')
                self.assertEqual(0, version.returncode, version.stderr)
                self.assertIn(packaging.root_manifest(self.root)['version'], version.stdout)
                check = self.launcher(module, 'update', '--check')
                self.assertEqual(0, check.returncode, check.stderr)
                self.assertIn('This installation is managed by npm. Please run `npm update -g oatty`.', check.stdout)
                self.assertEqual(0, self.run_shim(module, globally, '--version').returncode)
                native = self.native_directory(module) / 'bin' / self.platforms[self.host]['binary']
                self.assertEqual(Path(binary).read_bytes(), native.read_bytes())
        finally:
            self.__class__.address = original_address
            server.shutdown()
            server.server_close()


FAKE_NPM = r'''
const fs = require('node:fs');
const state = JSON.parse(fs.readFileSync(process.env.OATTY_PUBLISH_STATE, 'utf8'));
const args = process.argv.slice(2);
state.calls.push(args);
const save = () => fs.writeFileSync(process.env.OATTY_PUBLISH_STATE, JSON.stringify(state));
if (args[0] === 'view') {
  if (state.networkFailure) { save(); console.log(JSON.stringify({error:{code:'E401'}})); process.exit(1); }
  const manifest = state.registry[args[1]];
  if (!manifest || state.invisible === args[1]) { save(); console.log(JSON.stringify({error:{code:'E404'}})); process.exit(1); }
  save(); console.log(JSON.stringify(manifest));
} else if (args[0] === 'publish') {
  const artifact = state.artifacts[require('node:path').basename(args[1])];
  if (state.failPublish === artifact.name) { save(); process.exit(1); }
  state.registry[artifact.name+'@'+artifact.version] = artifact;
  save();
} else { save(); process.exit(2); }
'''


class PublishOrdering(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Reuse the actual npm-packed tarballs while keeping the fake publisher isolated.
        NpmDistribution.setUpClass()
        cls.base, cls.root, cls.directory = NpmDistribution.base, NpmDistribution.root, NpmDistribution.destination
        cls.version = NpmDistribution.version

    @classmethod
    def tearDownClass(cls):
        NpmDistribution.tearDownClass()

    def setUp(self):
        self.fake = self.base / 'fake-npm.js'
        self.fake.write_text(FAKE_NPM)
        self.state_path = self.base / 'publish-state.json'
        artifacts = {}
        for archive, manifest in publishing.collect_artifacts(self.root, self.directory):
            artifacts[archive.name] = dict(manifest, dist={'integrity': publishing.artifact_integrity(archive)})
        self.state = {'calls': [], 'registry': {}, 'artifacts': artifacts}
        self.environment = patch.dict(os.environ, {'NPM_CLI': str(self.fake), 'OATTY_PUBLISH_STATE': str(self.state_path)})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def run_publish(self):
        self.state_path.write_text(json.dumps(self.state))
        with patch.object(publishing.time, 'sleep'):
            try:
                publishing.publish(self.root, self.directory)
            finally:
                self.state = json.loads(self.state_path.read_text())

    def test_native_first_root_last_and_idempotent_retry(self):
        self.run_publish()
        published = [call for call in self.state['calls'] if call[0] == 'publish']
        self.assertEqual(5, len(published))
        self.assertEqual(f'oatty-{self.version}.tgz', Path(published[-1][1]).name)
        for call in published:
            self.assertIn('--provenance', call)
            self.assertIn('--access', call)
            self.assertIn('public', call)
        for index, call in enumerate(self.state['calls']):
            if call[0] == 'publish':
                self.assertEqual('view', self.state['calls'][index + 1][0])
        self.state['calls'] = []
        self.run_publish()
        self.assertFalse(any(call[0] == 'publish' for call in self.state['calls']))

    def test_existing_root_collision_prevents_all_payload_publication(self):
        umbrella = self.state['artifacts'][f'oatty-{self.version}.tgz']
        self.state['registry']['oatty@' + self.version] = dict(umbrella, dist={'integrity': 'sha512-old-release'})
        with self.assertRaisesRegex(ValueError, 'Published integrity differs'):
            self.run_publish()
        self.assertFalse(any(call[0] == 'publish' for call in self.state['calls']))

    def test_network_failure_is_not_treated_as_unpublished(self):
        self.state['networkFailure'] = True
        with self.assertRaisesRegex(RuntimeError, 'npm view failed'):
            self.run_publish()
        self.assertFalse(any(call[0] == 'publish' for call in self.state['calls']))

    def test_payload_failure_blocks_umbrella(self):
        self.state['failPublish'] = '@oattyio/cli-linux-x64'
        with self.assertRaises(subprocess.CalledProcessError):
            self.run_publish()
        self.assertNotIn('oatty@' + self.version, self.state['registry'])
        self.assertFalse(any(f'oatty-{self.version}.tgz' == Path(call[1]).name for call in self.state['calls'] if call[0] == 'publish'))

    def test_unconfirmed_payload_visibility_blocks_umbrella(self):
        self.state['invisible'] = '@oattyio/cli-darwin-x64@' + self.version
        with self.assertRaisesRegex(RuntimeError, 'not visible'):
            self.run_publish()
        self.assertNotIn('oatty@' + self.version, self.state['registry'])

    def test_changed_existing_integrity_or_manifest_fails(self):
        for changed in ({'dist': {'integrity': 'sha512-different'}}, {'os': ['wrong']}):
            with self.subTest(changed=changed):
                first = next(iter(self.state['artifacts'].values()))
                self.state['registry'] = {first['name'] + '@' + first['version']: dict(first, **changed)}
                self.state['calls'] = []
                with self.assertRaisesRegex(ValueError, 'Published .* differs'):
                    self.run_publish()
                self.assertFalse(any(call[0] == 'publish' for call in self.state['calls']))

    def test_missing_artifact_fails_before_registry_calls(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(publishing, 'registry_manifest') as registry:
                with self.assertRaisesRegex(ValueError, 'exactly four'):
                    publishing.publish(self.root, Path(temporary))
                registry.assert_not_called()


if __name__ == '__main__':
    unittest.main()
