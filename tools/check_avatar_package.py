#!/usr/bin/env python3
"""Audit the public Unity package, source equality, and original PNG hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, default=ROOT/'dist/avatar/MorphForgeAvatar-1.0.0.unitypackage')
    parser.add_argument('--output', type=Path, default=ROOT/'docs/avatar-plugin/package-audit.json')
    args = parser.parse_args()
    prefix = 'Assets/MorphForgeAvatar/'
    paths = []
    checked_sources = 0
    with tarfile.open(args.package, 'r:gz') as archive:
        members = {item.name: item for item in archive.getmembers()}
        for name in members:
            if not name.endswith('/pathname'):
                continue
            path = archive.extractfile(members[name]).read().decode().strip()
            assert path.startswith(prefix) and '/Variants/' not in path, f'Unexpected asset: {path}'
            paths.append(path)
            asset_name = name[:-len('pathname')] + 'asset'
            content = archive.extractfile(members[asset_name]).read()
            assert not re.search(rb'avtr_[0-9a-fA-F-]{36}', content), f'Bound Avatar ID in {path}'
            source = ROOT/path
            assert source.is_file(), f'Packaged asset missing from repository: {path}'
            expected = source.read_bytes()
            if path.endswith(('.cs', '.shader', '.cginc', '.json', '.md', '.txt', '.mat', '.anim', '.prefab', '.controller', '.meta')):
                content = content.replace(b'\r\n', b'\n')
                expected = expected.replace(b'\r\n', b'\n')
            assert content == expected, f'Package differs from checked-in source: {path}'
            checked_sources += 1
    catalog = json.loads((ROOT/prefix/'CatalogSource/catalog.json').read_text())
    assert not any(field in catalog for field in ['targetAvatarId', 'sourceProject', 'sourceRevision'])
    textures = list((ROOT/prefix/'CatalogSource/artifacts').glob('*.png'))
    assert len(textures) == 54
    assert all(hashlib.sha256(path.read_bytes()).hexdigest() == path.stem for path in textures)
    digest = hashlib.sha256(args.package.read_bytes()).hexdigest()
    report = {
        'success': True,
        'packageBytes': args.package.stat().st_size,
        'sha256': digest,
        'packageEntries': len(paths),
        'sourceMatches': checked_sources,
        'allPathsUnderPlugin': True,
        'includesUserVariants': False,
        'includesThirdPartyPackages': False,
        'privateAvatarBinding': False,
        'publicMaterialCount': len(catalog['materials']),
        'verifiedOriginalPngs': len(textures),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    print(json.dumps(report, ensure_ascii=False))

if __name__ == '__main__':
    main()
