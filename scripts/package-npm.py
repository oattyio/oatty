#!/usr/bin/env python3
"""Pack script-free npm packages from verified legal files and unchanged executables.

NPM_CLI may name an npm-cli.js file to run with Node (for example npm 12 in CI).
Packaging runs npm pack only against an isolated staging directory, with scripts
explicitly disabled. Native release archive packaging remains independent.
"""
import argparse
import gzip
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

import licenses

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = {'type': 'git', 'url': 'git+https://github.com/oattyio/oatty.git'}
PLATFORM_KEYS = {'darwin-x64', 'darwin-arm64', 'linux-x64', 'win32-x64'}
TARGETS = {
    'darwin-x64': 'x86_64-apple-darwin',
    'darwin-arm64': 'aarch64-apple-darwin',
    'linux-x64': 'x86_64-unknown-linux-gnu',
    'win32-x64': 'x86_64-pc-windows-msvc',
}


def npm_command():
    """Return the npm executable, honoring the documented npm-cli.js override."""
    if os.environ.get('NPM_CLI'):
        return ['node', os.environ['NPM_CLI']]
    if os.name == 'nt':
        wrapper = shutil.which('npm')
        if wrapper:
            cli = Path(wrapper).parent / 'node_modules/npm/bin/npm-cli.js'
            if cli.is_file():
                return ['node', str(cli)]
        raise ValueError('Cannot locate npm-cli.js; set NPM_CLI to its absolute path')
    return ['npm']


def load_platforms(root):
    """Read and validate the canonical four supported package/target contracts."""
    platforms = json.loads((root / 'npm/platforms.json').read_text())
    if set(platforms) != PLATFORM_KEYS:
        raise ValueError('Expected exactly four canonical npm platforms')
    for key, platform in platforms.items():
        system, cpu = key.split('-')
        expected = dict(package=f'@oattyio/cli-{key}', target=TARGETS[key], os=system,
                        cpu=cpu, binary='oatty.exe' if system == 'win32' else 'oatty')
        if system == 'linux':
            expected['libc'] = 'glibc'
        if platform != expected:
            raise ValueError(f'Invalid canonical platform metadata: {key}')
    return platforms


def root_manifest(root):
    """Require exact native optional versions and prohibit runtime dependencies/hooks."""
    manifest = json.loads((root / 'package.json').read_text())
    expected = {platform['package']: manifest['version'] for platform in load_platforms(root).values()}
    if manifest.get('optionalDependencies') != expected or manifest.get('dependencies'):
        raise ValueError('Root package must contain only exact-version native optional dependencies')
    forbidden = {'preinstall', 'install', 'postinstall', 'prepare', 'postpack', 'prepublish',
                 'prepublishOnly', 'publish', 'postpublish'}
    if manifest.get('scripts', {}).get('prepack') != 'python3 scripts/licenses.py verify':
        raise ValueError('Required source prepack licensing guard is missing')
    if forbidden.intersection(manifest.get('scripts', {})):
        raise ValueError('npm lifecycle hooks are forbidden')
    if manifest.get('repository') != REPOSITORY or manifest.get('name') != 'oatty' or manifest.get('license') != 'MIT':
        raise ValueError('Incorrect Oatty package identity, repository, or license')
    return manifest


def payload_manifest(root, platform):
    """Construct dependency-free metadata for one exact-version native payload."""
    root_package = root_manifest(root)
    manifest = dict(name=platform['package'], version=root_package['version'],
                    description=f"Oatty native executable for {platform['target']}", license='MIT',
                    repository=REPOSITORY, os=[platform['os']], cpu=[platform['cpu']],
                    oattyTarget=platform['target'], files=['bin', 'LICENSE', *licenses.GENERATED])
    if 'libc' in platform:
        manifest['libc'] = [platform['libc']]
    return manifest


def umbrella_manifest(root):
    """Return published launcher metadata without development-only npm commands."""
    manifest = root_manifest(root)
    manifest.pop('scripts', None)
    return manifest


def legal_files(root):
    """Return every required legal/source filename, retaining authoritative bytes."""
    return ['LICENSE', *sorted(licenses.inventory(root))]


def source_bytes(root, name):
    """Canonicalize first-party checkout text; preserve every upstream legal byte."""
    data = (root / name).read_bytes()
    owned_text = {'LICENSE', 'npm/bin/oatty.js', 'npm/platforms.json', 'README.md'}
    return data.replace(b'\r\n', b'\n') if name in owned_text else data


def stage_legal(root, stage):
    """Copy the complete verified notices and sources into an isolated package."""
    for name in legal_files(root):
        destination = stage / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source_bytes(root, name))


def write_manifest(stage, manifest):
    """Write stable, readable package metadata to the staging directory."""
    (stage / 'package.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')


def pack_stage(stage, destination):
    """Run guarded npm pack with all lifecycle execution explicitly disabled."""
    destination.mkdir(parents=True, exist_ok=True)
    result = subprocess.run([*npm_command(), 'pack', '--ignore-scripts', '--json',
                             '--pack-destination', str(destination.resolve())], cwd=stage,
                            capture_output=True, text=True, check=True,
                            env={**os.environ, 'npm_config_ignore_scripts': 'true',
                                 'npm_config_cache': str(stage / '.npm-cache')})
    report = json.loads(result.stdout)
    packages = report if isinstance(report, list) else list(report.values())
    if len(packages) != 1 or Path(packages[0]['filename']).name != packages[0]['filename']:
        raise ValueError('Expected exactly one npm pack artifact filename')
    archive = destination.resolve() / packages[0]['filename']
    if (stage / 'npm/bin/oatty.js').exists():
        executable = 'npm/bin/oatty.js'
    else:
        executable = 'bin/oatty.exe' if (stage / 'bin/oatty.exe').exists() else 'bin/oatty'
    normalize_executable_mode(archive, executable)
    return archive


def normalize_executable_mode(archive, executable):
    """Ensure Unix executable metadata even when npm pack runs on Windows.

    Windows chmod cannot express Unix execute bits. Only tar metadata changes;
    signed executable content stays byte-for-byte unchanged. Existing mode-755
    npm output is retained exactly. Rewritten gzip headers use a fixed timestamp.
    """
    with tarfile.open(archive, 'r:gz') as source:
        members = source.getmembers()
        if source.getmember('package/' + executable).mode == 0o755:
            return
        data = [(member, source.extractfile(member).read() if member.isfile() else None) for member in members]
    with archive.open('wb') as output:
        with gzip.GzipFile(fileobj=output, mode='wb', filename='', mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as destination:
                for member, content in data:
                    if member.name == 'package/' + executable:
                        member.mode = 0o755
                    destination.addfile(member, io.BytesIO(content) if content is not None else None)


def package_binary(root, binary, target, destination):
    """Pack one signed/notarized binary unchanged, with legal files and version marker."""
    licenses.verify(root)
    platform = next((item for item in load_platforms(root).values() if item['target'] == target), None)
    if platform is None:
        raise ValueError(f'Unsupported Rust target: {target}')
    with tempfile.TemporaryDirectory(prefix='oatty-npm-native-') as temporary:
        stage = Path(temporary)
        stage_legal(root, stage)
        write_manifest(stage, payload_manifest(root, platform))
        (stage / 'bin').mkdir()
        executable = stage / 'bin' / platform['binary']
        shutil.copyfile(binary, executable)
        executable.chmod(0o755)
        (stage / 'bin/.oatty-installed-version').write_text(root_manifest(root)['version'] + '\n', encoding='utf-8', newline='\n')
        archive = pack_stage(stage, destination)
    contents = verify_artifact(root, archive, platform)
    if contents[f"bin/{platform['binary']}"] != binary.read_bytes():
        raise ValueError('npm packing changed signed executable bytes')
    return archive


def package_umbrella(root, destination):
    """Pack only the launcher, canonical mapping, documentation, and legal materials."""
    licenses.verify(root)
    with tempfile.TemporaryDirectory(prefix='oatty-npm-launcher-') as temporary:
        stage = Path(temporary)
        stage_legal(root, stage)
        write_manifest(stage, umbrella_manifest(root))
        for name in ('npm/bin/oatty.js', 'npm/platforms.json', 'README.md'):
            output = stage / name
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(source_bytes(root, name))
        (stage / 'npm/bin/oatty.js').chmod(0o755)
        archive = pack_stage(stage, destination)
    verify_artifact(root, archive)
    return archive


def read_artifact(archive):
    """Read safe regular npm archive entries, rejecting aliases and duplicate paths."""
    contents = {}
    modes = {}
    with tarfile.open(archive, 'r:gz') as source:
        for member in source.getmembers():
            if not member.name.startswith('package/') or '..' in Path(member.name).parts:
                raise ValueError(f'Unsafe npm archive entry: {member.name}')
            if member.isdir():
                continue
            name = member.name.removeprefix('package/')
            if not member.isfile() or name in contents:
                raise ValueError(f'Non-regular or duplicate npm archive entry: {member.name}')
            contents[name] = source.extractfile(member).read()
            modes[name] = member.mode
    return contents, modes


def verify_artifact(root, archive, platform=None):
    """Validate metadata, exact package layout, legal bytes, and executable mode."""
    contents, modes = read_artifact(archive)
    manifest = json.loads(contents['package.json'])
    expected_manifest = payload_manifest(root, platform) if platform else umbrella_manifest(root)
    if manifest != expected_manifest:
        raise ValueError(f'Unexpected npm manifest in {archive}')
    expected = {'package.json', *legal_files(root)}
    if platform:
        binary_name = f"bin/{platform['binary']}"
        expected.update((binary_name, 'bin/.oatty-installed-version'))
        if not contents.get(binary_name) or modes.get(binary_name) != 0o755:
            raise ValueError('Native executable is missing or not mode 755')
        if contents.get('bin/.oatty-installed-version') != (manifest['version'] + '\n').encode():
            raise ValueError('Invalid static installed-version marker')
    else:
        expected.update(('npm/bin/oatty.js', 'npm/platforms.json', 'README.md'))
        if modes.get('npm/bin/oatty.js') != 0o755:
            raise ValueError('Launcher is not mode 755')
        for name in ('npm/bin/oatty.js', 'npm/platforms.json', 'README.md'):
            if contents.get(name) != source_bytes(root, name):
                raise ValueError(f'Changed launcher package content: {name}')
    if set(contents) != expected:
        raise ValueError(f'Missing or unexpected npm package files: {set(contents) ^ expected}')
    for name in legal_files(root):
        if contents[name] != source_bytes(root, name):
            raise ValueError(f'Changed npm legal/source bytes: {name}')
    return contents


def main():
    """Expose native and umbrella packaging as mutually exclusive release commands."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--target', choices=TARGETS.values())
    parser.add_argument('--umbrella', action='store_true')
    parser.add_argument('--output', type=Path, default=Path('dist'))
    arguments = parser.parse_args()
    if arguments.umbrella == bool(arguments.binary) or (not arguments.umbrella and not arguments.target):
        parser.error('use --umbrella OR --binary PATH --target TARGET')
    if arguments.umbrella and arguments.target:
        parser.error('--target is only valid with --binary')
    archive = package_umbrella(ROOT, arguments.output) if arguments.umbrella else package_binary(
        ROOT, arguments.binary, arguments.target, arguments.output)
    print(archive)


if __name__ == '__main__':
    main()
