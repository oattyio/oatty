#!/usr/bin/env python3
"""Package the signed release executable unchanged, with verified legal materials."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

from licenses import GENERATED, ROOT, verify

TARGETS = ('x86_64-unknown-linux-gnu', 'x86_64-apple-darwin',
           'aarch64-apple-darwin', 'x86_64-pc-windows-msvc')


def package(root, binary, target, version, destination):
    verify(root)
    binary_name = 'oatty.exe' if target.endswith('windows-msvc') else 'oatty'
    extension = '.tar.gz' if target.endswith('linux-gnu') else '.zip'
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination.resolve() / f'oatty-{version}-{target}{extension}'
    with tempfile.TemporaryDirectory(prefix='oatty-package-') as temporary:
        stage = Path(temporary)
        shutil.copy2(binary, stage / binary_name)
        shutil.copy2(root / 'LICENSE', stage / 'LICENSE')
        for name in GENERATED:
            source = root / name
            if source.is_dir():
                shutil.copytree(source, stage / name)
            else:
                shutil.copy2(source, stage / name)
        if extension == '.tar.gz':
            with tarfile.open(archive, 'w:gz') as output:
                for item in sorted(stage.iterdir()):
                    output.add(item, arcname=item.name)
        elif sys.platform == 'darwin' and target.endswith('apple-darwin'):
            # Keep the existing macOS resource-fork aware zip/notarization format.
            subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', str(stage), str(archive)], check=True)
        else:
            with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as output:
                for item in sorted(stage.rglob('*')):
                    if item.is_file():
                        output.write(item, item.relative_to(stage).as_posix())
    verify_archive(archive, binary_name, binary, root)
    return archive


def verify_archive(archive, binary_name, binary, root):
    """Read back all archive entries, enforcing root layout and byte preservation."""
    if archive.name.endswith('.tar.gz'):
        with tarfile.open(archive) as source:
            contents = {member.name: source.extractfile(member).read() for member in source.getmembers() if member.isfile()}
    else:
        with zipfile.ZipFile(archive) as source:
            contents = {name: source.read(name) for name in source.namelist() if not name.endswith('/') and not name.startswith('__MACOSX/')}
    expected = ['LICENSE', GENERATED[0]]
    for name in GENERATED[1:]:
        expected.extend(p.relative_to(root).as_posix() for p in (root / name).rglob('*') if p.is_file())
    if contents.pop(binary_name, None) != binary.read_bytes():
        raise ValueError('Packaged executable bytes or root path changed')
    if set(contents) != set(expected):
        raise ValueError('Release archive is missing or has unexpected licensing entries')
    for name in expected:
        if contents[name] != (root / name).read_bytes():
            raise ValueError(f'Packaged license/source bytes changed: {name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--target', required=True, choices=TARGETS)
    parser.add_argument('--version', required=True)
    parser.add_argument('--output', type=Path, default=Path('dist'))
    args = parser.parse_args()
    print(package(ROOT, args.binary, args.target, args.version, args.output))


if __name__ == '__main__':
    main()
