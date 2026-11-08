"""Validate a release candidate without modifying the repository."""

import argparse
import json
import os
import re
import subprocess
import tomllib
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen


def candidate(root):
    manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    core = tomllib.loads((root / 'crates/rustiq-core/Cargo.toml').read_text())
    version = manifest['package']['version']
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?', version):
        raise ValueError('invalid release version')
    pre = version.partition('-')[2].split('.')
    if 'dev' in pre:
        raise ValueError('development versions cannot be released')
    if any(part.isdigit() and len(part) > 1 and part.startswith('0') for part in pre):
        raise ValueError('numeric prerelease identifiers cannot have leading zeros')
    if core['package']['version'] != version:
        raise ValueError('workspace package versions differ')
    if manifest['dependencies']['rustiq-core']['version'].removeprefix('=') != version:
        raise ValueError('internal dependency must match release version')
    return version


def lookup(url, token=None):
    headers = {'User-Agent': 'RustiQ-release-check', 'Accept': 'application/json'}
    if token:
        headers['Authorization'] = f'Bearer {token}'
    try:
        with urlopen(Request(url, headers=headers), timeout=30) as response:
            return json.load(response)
    except HTTPError as error:
        if error.code == 404:
            return None
        raise


def available(version, repository):
    token = os.environ.get('GH_TOKEN')
    base = f'https://api.github.com/repos/{repository}'
    for kind in ('git/ref/tags', 'releases/tags'):
        if lookup(f'{base}/{kind}/v{version}', token) is not None:
            raise ValueError(f'v{version} already exists ({kind})')
    for package in ('rustiq-chem-core', 'rustiq-chem'):
        if lookup(f'https://crates.io/api/v1/crates/{package}/{version}') is not None:
            raise ValueError(f'{package} {version} is already published')
    owner, name = repository.split('/')
    versions = lookup(f'https://{owner}.github.io/{name}/versions.json')
    if versions and any(entry['version'] == f'v{version}' for entry in versions['versions']):
        raise ValueError(f'manual v{version} is already published')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', default='mveril/RustiQ')
    parser.add_argument('--tag')
    parser.add_argument('--check-availability', action='store_true')
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    try:
        version = candidate(Path.cwd())
        if args.tag and args.tag != f'v{version}':
            raise ValueError('tag does not match package version')
        if args.check_availability:
            available(version, args.repository)
        if args.dry_run:
            subprocess.run(['cargo', 'publish', '--workspace', '--locked', '--dry-run'], check=True)
        print(f'v{version}')
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Release validation failed: {error}\n')


if __name__ == '__main__':
    main()
