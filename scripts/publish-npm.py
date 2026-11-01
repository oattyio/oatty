#!/usr/bin/env python3
"""Validate all npm artifacts, publish native payloads, then publish the launcher.

Retries accept existing versions only when their registry integrity and release
metadata match these exact local artifacts. Network/authentication errors never
mean that a version is available to publish. No binaries are built or repacked.
"""
import argparse
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import time

import licenses

SPEC = importlib.util.spec_from_file_location('package_npm', Path(__file__).with_name('package-npm.py'))
packaging = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(packaging)
ROOT = Path(__file__).resolve().parents[1]


def artifact_integrity(archive):
    """Compute the npm sha512 SRI value for the unchanged local archive bytes."""
    return 'sha512-' + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode('ascii')


def registry_manifest(name, version):
    """Return exact registry metadata or None only for a structured npm E404."""
    result = subprocess.run([*packaging.npm_command(), 'view', f'{name}@{version}', '--json'],
                            capture_output=True, text=True)
    if result.returncode:
        try:
            error = json.loads(result.stdout).get('error', {})
        except (ValueError, AttributeError):
            error = {}
        if error.get('code') == 'E404':
            return None
        raise RuntimeError(f'npm view failed for {name}@{version}: {result.stderr.strip() or result.stdout.strip()}')
    metadata = json.loads(result.stdout)
    # npm 12 keeps the version result array; npm 11 unwraps a single version.
    if isinstance(metadata, list) and len(metadata) == 1:
        metadata = metadata[0]
    if not isinstance(metadata, dict):
        raise ValueError(f'Unexpected registry response for {name}@{version}')
    return metadata


def verify_registry(archive, expected, registry):
    """Reject an already-published version with different bytes or platform metadata."""
    if registry.get('dist', {}).get('integrity') != artifact_integrity(archive):
        raise ValueError(f"Published integrity differs for {expected['name']}@{expected['version']}")
    keys = ('name', 'version', 'license', 'repository', 'os', 'cpu', 'libc', 'oattyTarget',
            'bin', 'engines', 'optionalDependencies', 'dependencies', 'scripts')
    for key in keys:
        if registry.get(key) != expected.get(key):
            raise ValueError(f"Published {key} differs for {expected['name']}@{expected['version']}")


def ensure_published(archive, manifest):
    """Publish once, or verify an existing version; require confirmed registry visibility."""
    registry = registry_manifest(manifest['name'], manifest['version'])
    if registry is not None:
        verify_registry(archive, manifest, registry)
        return
    subprocess.run([*packaging.npm_command(), 'publish', str(archive.resolve()),
                    '--provenance', '--access', 'public', '--ignore-scripts'], check=True)
    for attempt in range(5):
        registry = registry_manifest(manifest['name'], manifest['version'])
        if registry is not None:
            verify_registry(archive, manifest, registry)
            return
        if attempt < 4:
            time.sleep(1)
    raise RuntimeError(f"Published package not visible: {manifest['name']}@{manifest['version']}")


def collect_artifacts(root, directory):
    """Validate the complete artifact set before making any registry changes."""
    licenses.verify(root)
    root_manifest = packaging.root_manifest(root)
    version = root_manifest['version']
    releases = []
    for platform in packaging.load_platforms(root).values():
        filename = f"{platform['package'].replace('@', '').replace('/', '-')}-{version}.tgz"
        releases.append((directory / filename, platform))
    releases.append((directory / f'oatty-{version}.tgz', None))
    if {path.name for path, _ in releases} != {path.name for path in directory.glob('*.tgz')}:
        raise ValueError('Expected exactly four native payload tarballs and one umbrella tarball')
    verified = []
    for archive, platform in releases:
        contents = packaging.verify_artifact(root, archive, platform)
        verified.append((archive, json.loads(contents['package.json'])))
    return verified


def publish(root, directory):
    """Publish all validated payloads before exposing their exact-version umbrella."""
    releases = collect_artifacts(root, directory)
    # Detect every version collision, including the umbrella, before any publish.
    for archive, manifest in releases:
        registry = registry_manifest(manifest['name'], manifest['version'])
        if registry is not None:
            verify_registry(archive, manifest, registry)
    for archive, manifest in releases:
        ensure_published(archive, manifest)


def main():
    """Publish an already-produced and locally validated release artifact directory."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', required=True, type=Path)
    arguments = parser.parse_args()
    publish(ROOT, arguments.directory)


if __name__ == '__main__':
    main()
