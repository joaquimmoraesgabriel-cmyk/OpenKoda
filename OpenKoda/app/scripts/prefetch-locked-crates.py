"""Finite transport workaround for specific locked Windows/core dependencies.

Only official static.crates.io archives named in Cargo.lock are fetched. Every
archive must match Cargo.lock's SHA-256 before entering the local proxy cache.
Existing Cargo caches are read-only; no registry configuration is changed.
"""
import concurrent.futures
import hashlib
import json
import os
import pathlib
import re
import tempfile
import time
import tomllib
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
DESTINATION = ROOT / '.local' / 'cargo-official-cache'
REGISTRY = pathlib.Path.home() / '.cargo' / 'registry' / 'cache'
NAMES = {
    'bytemuck', 'cssparser-macros', 'find-msvc-tools', 'futures-io', 'hyper-rustls',
    'icu_normalizer', 'icu_properties_data', 'libsqlite3-sys', 'mio', 'proc-macro-crate',
    'quick-xml', 'reqwest', 'rustversion', 'serde_repr', 'tauri-macros', 'tauri-runtime-wry',
    'tinystr', 'tokio-util', 'toml', 'toml_datetime', 'toml_edit', 'untrusted',
    'webview2-com-sys', 'windows', 'windows-core', 'winnow',
}
MAX_ARCHIVE = 64 * 1024 * 1024


def verified(path, checksum):
    if not path.is_file() or path.stat().st_size > MAX_ARCHIVE:
        return False
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest() == checksum


def atomic_write(path, data):
    with tempfile.NamedTemporaryFile(dir=DESTINATION, prefix='prefetch-', delete=False) as stream:
        temporary = pathlib.Path(stream.name)
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    try:
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def fetch(package):
    name, version, checksum = package['name'], package['version'], package['checksum']
    if not re.fullmatch(r'[A-Za-z0-9_-]+', name) or not re.fullmatch(r'[A-Za-z0-9.+_-]+', version):
        raise ValueError('Invalid locked package identity')
    if not re.fullmatch(r'[0-9a-f]{64}', checksum):
        raise ValueError('Invalid lockfile checksum')
    archive = f'{name}-{version}.crate'
    url = f'https://static.crates.io/crates/{name}/{archive}'
    target = DESTINATION / hashlib.sha256(url.encode()).hexdigest()
    if verified(target, checksum):
        return {'name': name, 'version': version, 'status': 'verified-local-cache'}
    for candidate in REGISTRY.glob(f'*/{archive}'):
        if verified(candidate, checksum):
            return {'name': name, 'version': version, 'status': 'verified-registry-cache'}
    error_kind = None
    for attempt in range(1, 4):
        try:
            transport_url=url if attempt != 2 else f'https://static.crates.io/crates/{name}/{version}/download'
            request = urllib.request.Request(transport_url, headers={'User-Agent': 'OpenKoda-locked-dependency-prefetch'})
            deadline=time.monotonic()+45
            with urllib.request.urlopen(request, timeout=20) as response:
                endpoint = urllib.parse.urlparse(response.url)
                if endpoint.scheme != 'https' or endpoint.hostname != 'static.crates.io' or endpoint.username or endpoint.password:
                    raise ValueError('Download left official crate archive host')
                data=bytearray()
                while len(data)<=MAX_ARCHIVE:
                    if time.monotonic()>=deadline:
                        raise TimeoutError('Archive exceeded finite download deadline')
                    chunk=response.read1(min(65536,MAX_ARCHIVE+1-len(data)))
                    if not chunk:
                        break
                    data.extend(chunk)
            if len(data) > MAX_ARCHIVE or hashlib.sha256(data).hexdigest() != checksum:
                raise ValueError('Archive differs from exact Cargo.lock checksum')
            atomic_write(target, data)
            metadata = {'url': url, 'transportUrl': transport_url, 'bytes': len(data), 'sha256': checksum,
                        'verifiedAgainst': 'src-tauri/Cargo.lock'}
            atomic_write(target.with_suffix('.json'), json.dumps(metadata).encode())
            return {'name': name, 'version': version, 'status': 'downloaded-verified', 'bytes': len(data), 'attempt': attempt}
        except (OSError, ValueError) as error:
            error_kind = type(error).__name__
            if attempt < 3:
                time.sleep(attempt / 2)
    return {'name': name, 'version': version, 'status': 'failed', 'error': error_kind}


def main():
    with (ROOT / 'src-tauri' / 'Cargo.lock').open('rb') as stream:
        lock = tomllib.load(stream)
    packages = [p for p in lock['package'] if p['name'] in NAMES
                and p.get('source') == 'registry+https://github.com/rust-lang/crates.io-index']
    DESTINATION.mkdir(parents=True, exist_ok=True)
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as workers:
        for result in workers.map(fetch, packages):
            results.append(result)
            print(json.dumps(result), flush=True)
    counts = {status: sum(r['status'] == status for r in results) for status in sorted({r['status'] for r in results})}
    report = {'packages': len(packages), 'counts': counts, 'results': results}
    atomic_write(DESTINATION / 'prefetch-result.json', json.dumps(report, indent=2).encode())
    print(json.dumps({'packages': len(packages), 'counts': counts}), flush=True)
    return int(any(r['status'] == 'failed' for r in results))


if __name__ == '__main__':
    raise SystemExit(main())
