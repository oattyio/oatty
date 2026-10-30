#!/usr/bin/env python3
"""Prepare and verify deterministic licensing materials for every Oatty distribution.

Requires Python 3.11+, cargo-about 0.9.2, Cargo, and `npm ci --ignore-scripts`.
Generation uses the CLI's default features and about.toml's union of release targets.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
GENERATED = ('THIRD-PARTY-NOTICES.txt', 'third-party-licenses', 'third-party-sources')
MANIFEST = '.license-manifest.json'
NOTICE_NAME = re.compile(r'^(?:LICEN[CS]E|NOTICE|COPYRIGHT|COPYING)(?:$|[._-])', re.I)
PLACEHOLDER = re.compile(r'<year>|<owner>|<copyright holders>', re.I)
ATTRIBUTION = re.compile(r'Copyright\s+(?:\([cC]\)|©|\d{4})[^\n]*', re.I)
REQUIRED = {
    'aws-lc-sys': ('LICENSE', 'aws-lc/LICENSE', 'aws-lc/third_party/fiat/LICENSE'),
    'ring': ('LICENSE', 'LICENSE-other-bits', 'LICENSE-BoringSSL',
             'src/polyfill/once_cell/LICENSE-APACHE', 'src/polyfill/once_cell/LICENSE-MIT', 'third_party/fiat/LICENSE'),
    'encoding_rs': ('COPYRIGHT', 'LICENSE-APACHE', 'LICENSE-MIT', 'LICENSE-WHATWG'),
}
NPM_LICENSES = {'MIT', 'ISC', 'BlueOak-1.0.0'}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inputs(root):
    """Fingerprint locks, policy, generation code, and every workspace manifest."""
    paths = ['LICENSE', 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json',
             'about.toml', 'scripts/licenses.py']
    paths.extend(p.relative_to(root).as_posix() for p in (root / 'crates').rglob('Cargo.toml'))
    paths.extend(p.relative_to(root).as_posix() for p in (root / 'scripts/license-supplements').rglob('*') if p.is_file())
    # Git may check out these committed text files with CRLF on Windows.
    return {name: hashlib.sha256((root / name).read_bytes().replace(b'\r\n', b'\n')).hexdigest()
            for name in sorted(paths)}


def inventory(root):
    files = []
    for name in GENERATED:
        path = root / name
        if not path.exists():
            raise ValueError(f'Missing generated material: {name}; run npm run licenses:prepare')
        files.extend([path] if path.is_file() else (p for p in path.rglob('*') if p.is_file()))
    return {p.relative_to(root).as_posix(): sha256(p) for p in sorted(files)}


def verify(root):
    """Reject missing, stale, edited, or extra generated materials before packing."""
    manifest = json.loads((root / MANIFEST).read_text())
    if manifest['inputs'] != inputs(root):
        raise ValueError('Stale licensing inputs; run npm run licenses:prepare')
    if manifest['files'] != inventory(root):
        raise ValueError('Missing or changed licensing output/source; run npm run licenses:prepare')
    if not manifest['files'] or not any(n.endswith('.crate') for n in manifest['files']):
        raise ValueError('Required MPL source bundle is missing')


def copy_notices(source, destination, required=()):
    """Preserve notice bytes and relative paths, including vendored native trees."""
    for name in required:
        if not (source / name).is_file():
            raise ValueError(f'Missing required upstream notice: {source / name}')
    copied = []
    for path in sorted(source.rglob('*')):
        if path.is_file() and NOTICE_NAME.match(path.name) and path.suffix.lower() != '.rs':
            if not path.read_bytes().strip():
                raise ValueError(f'Empty upstream notice: {path}')
            relative = path.relative_to(source)
            output = destination / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, output)
            copied.append(relative.as_posix())
    return copied


def exact_source_path(source, relative):
    """Require original path casing even on case-insensitive developer filesystems."""
    path = source
    for part in Path(relative).parts:
        if part in ('.', '..') or part not in {entry.name for entry in path.iterdir()}:
            raise ValueError(f'Reviewed source path is missing or has incorrect case: {relative}')
        path = path / part
    return path


def preserve_supplement(package, locked, destination, supplements, evidence_root):
    """Use only version/checksum-bound reviewed evidence; never infer copyright owners."""
    record = supplements.get(f"{package['name']}@{package['version']}")
    if record is None:
        return []
    if record['crate-checksum'] != locked['checksum']:
        raise ValueError(f"Stale reviewed license evidence: {package['name']}")
    source = Path(package['manifest_path']).parent
    if 'vcs-commit' in record:
        vcs = json.loads((source / '.cargo_vcs_info.json').read_text())
        if vcs['git']['sha1'] != record['vcs-commit']:
            raise ValueError(f"Changed upstream source revision: {package['name']}")
    preserved = []
    for evidence in record['files']:
        # Restore immutable upstream text bytes after a possible CRLF Git checkout.
        data = (evidence_root / evidence['file']).read_bytes().replace(b'\r\n', b'\n')
        if hashlib.sha256(data).hexdigest() != evidence['sha256']:
            raise ValueError(f"Reviewed license supplement checksum mismatch: {evidence['file']}")
        if 'source-path' in evidence and sha256(exact_source_path(source, evidence['source-path'])) != evidence['source-sha256']:
            raise ValueError(f"Reviewed source header changed: {package['name']}")
        destination.mkdir(parents=True, exist_ok=True)
        target = destination / evidence['file']
        target.write_bytes(data)
        preserved.append(evidence['file'])
    destination.mkdir(parents=True, exist_ok=True)
    (destination / 'PROVENANCE.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n',
                                               encoding='utf-8', newline='\n')
    return preserved


def validate_attribution(package, notices, destination, template, reviewed):
    """Reject generic templates that omit real upstream attribution or reviewed grant evidence."""
    if not notices and not reviewed:
        raise ValueError(f"Missing reviewed upstream notices: {package['name']} {package['version']}")
    if template and not reviewed:
        texts = [(destination / name).read_text(encoding='utf-8', errors='replace') for name in notices]
        if not any(ATTRIBUTION.search(text) and not PLACEHOLDER.search(text) for text in texts):
            raise ValueError(f"Generic license template lacks reviewed attribution: {package['name']}")


def rust_notices(report, output, lock, supplements=None, evidence_root=None):
    """Use cargo-about's SPDX evaluation; never invent a parallel SPDX parser."""
    supplements = supplements or {}
    evidence_root = evidence_root or ROOT / 'scripts/license-supplements'
    covered = set()
    templates = set()
    sections = []
    for license_info in report['licenses']:
        if not license_info.get('text', '').strip():
            raise ValueError('Missing selected license text')
        users = sorted(f"{u['crate']['name']} {u['crate']['version']}" for u in license_info['used_by'])
        covered.update(u['crate']['id'] for u in license_info['used_by'])
        if PLACEHOLDER.search(license_info['text']):
            templates.update(u['crate']['id'] for u in license_info['used_by'])
        sections.append(f"## {license_info['name']} ({license_info['id']})\n\nUsed by: {', '.join(users)}\n\n{license_info['text']}\n")
    locked = {(p['name'], p['version']): p for p in lock['package']}
    packages = []
    source_bundles = []
    for entry in sorted(report['crates'], key=lambda x: (x['package']['name'], x['package']['version'])):
        package = entry['package']
        if not package.get('license') or package['id'] not in covered:
            raise ValueError(f"Unresolved licensing: {package['name']} {package['version']}")
        if not package['source']:
            continue  # Oatty's workspace MIT text ships separately as LICENSE.
        name, version = package['name'], package['version']
        source = Path(package['manifest_path']).parent
        destination = output / 'third-party-licenses' / 'rust' / f'{name}-{version}'
        notices = copy_notices(source, destination, REQUIRED.get(name, ()))
        notices += preserve_supplement(package, locked[(name, version)], destination, supplements, evidence_root)
        validate_attribution(package, notices, destination, package['id'] in templates, f'{name}@{version}' in supplements)
        packages.append(f"- {name} {version}: {entry['license']}; upstream: {package.get('repository') or package.get('homepage') or package['source']}; preserved notices: {len(notices)}")
        if 'MPL-2.0' in package['license']:
            if name != 'nucleo-matcher' or version != '0.3.1':
                raise ValueError(f'Unreviewed MPL source obligation: {name} {version}')
            checksum = locked[(name, version)]['checksum']
            # Cargo's registry cache is a sibling of its extracted source directory.
            archive = source.parents[1] / '..' / 'cache' / source.parent.name / f'{name}-{version}.crate'
            bundle_source(archive.resolve(), output / 'third-party-sources' / archive.name, checksum)
            source_bundles.append(f'- {archive.name}; SHA-256 {checksum}; https://crates.io/crates/{name}/{version}')
    if not source_bundles:
        raise ValueError('Expected nucleo-matcher source was not selected; review MPL policy')
    (output / 'third-party-sources' / 'README.txt').write_text(
        'MPL-2.0 source availability\n\nOatty includes unchanged nucleo-matcher 0.3.1 under MPL-2.0.\n'
        'The complete corresponding upstream source is provided here as its original\n'
        'crates.io .crate archive (a gzip-compressed tar archive). Extract it with\n'
        '`tar -xzf nucleo-matcher-0.3.1.crate`; its LICENSE contains MPL-2.0.\n'
        'You may modify and redistribute this covered source under MPL-2.0.\n'
        'Oatty makes no modifications to this covered source.\n\n' + '\n'.join(source_bundles) + '\n', encoding='utf-8', newline='\n')
    return '## Rust package inventory\n\n' + '\n'.join(packages) + '\n\n' + '\n'.join(sections)


def bundle_source(archive, destination, checksum):
    if sha256(archive) != checksum:
        raise ValueError(f'Source checksum mismatch: {archive.name}')
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(archive, destination)


def npm_notices(root, output):
    lock = json.loads((root / 'package-lock.json').read_text())
    sections = []
    own_package = json.loads((root / 'package.json').read_text())
    locked_root = lock['packages']['']
    metadata_changed = any(locked_root.get(key) != own_package.get(key) for key in ('name', 'version', 'license'))
    dependencies_changed = any(locked_root.get(key, {}) != own_package.get(key, {})
                               for key in ('dependencies', 'optionalDependencies'))
    if metadata_changed or dependencies_changed:
        raise ValueError('Stale npm package-lock root metadata; update the lock with npm install --package-lock-only --ignore-scripts, then npm ci --ignore-scripts')
    for location, package in sorted(lock['packages'].items()):
        if not location:
            continue
        source = root / location
        installed = json.loads((source / 'package.json').read_text())
        license_id = package.get('license')
        if license_id not in NPM_LICENSES or installed['version'] != package['version'] or installed.get('license') != license_id:
            raise ValueError(f'Unresolved or stale npm licensing: {location}')
        destination = output / 'third-party-licenses' / 'npm' / location.removeprefix('node_modules/')
        names = copy_notices(source, destination)
        if not names:
            raise ValueError(f'Missing npm license text: {location}')
        sections.append(f"- {installed['name']} {installed['version']}: {license_id}; full text in third-party-licenses/npm/{location.removeprefix('node_modules/')}/")
    return '## npm installer dependencies\n\n' + '\n'.join(sections) + '\n'


def prepare(root):
    tool = os.environ.get('CARGO_ABOUT', 'cargo-about')
    version = subprocess.check_output([tool, '--version'], text=True).strip()
    if version != 'cargo-about 0.9.2':
        raise ValueError(f'Expected cargo-about 0.9.2, got {version}')
    with tempfile.TemporaryDirectory(prefix='oatty-licenses-') as directory:
        output = Path(directory)
        report = json.loads(subprocess.check_output([
            tool, 'generate', '--locked', '--offline', '--fail', '--manifest-path', 'crates/cli/Cargo.toml',
            '--config', 'about.toml', '--format', 'json'], cwd=root))
        text = ('# Oatty third-party notices\n\nOatty itself is MIT licensed; see LICENSE.\n'
                'Dependencies retain their original licenses. MIT is preferred where upstream offers a choice.\n'
                'This inventory covers the CLI default features across Linux x86_64, macOS x86_64/aarch64,\n'
                'and Windows x86_64 MSVC, including build and transitive dependencies, excluding dev-only dependencies.\n'
                'Full upstream supplemental notices (including native code) are preserved under third-party-licenses/.\n'
                'MPL-2.0 corresponding source and extraction instructions are in third-party-sources/.\n\n')
        evidence_root = root / 'scripts/license-supplements'
        supplements = json.loads((evidence_root / 'reviewed.json').read_text())
        text += rust_notices(report, output, tomllib.loads((root / 'Cargo.lock').read_text()), supplements, evidence_root)
        text += '\n' + npm_notices(root, output)
        (output / GENERATED[0]).write_text(text, encoding='utf-8', newline='\n')
        manifest = {'generator': version, 'inputs': inputs(root), 'files': inventory(output)}
        for name in GENERATED:
            destination = root / name
            if destination.is_dir():
                shutil.rmtree(destination)
            elif destination.exists():
                destination.unlink()
            shutil.move(str(output / name), destination)
        (root / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n', encoding='utf-8', newline='\n')
    verify(root)
    print(f'Prepared {len(report["crates"])} Rust packages and verified distribution licensing materials.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('prepare', 'verify'))
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(ROOT)
    else:
        verify(ROOT)
        print('Distribution licensing materials are current and complete.', file=sys.stderr)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f'License check failed: {error}') from error
