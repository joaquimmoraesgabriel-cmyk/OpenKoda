"""Temporary loopback Cargo transport workaround; exclusively fetches official crates.io.

The Cargo/libcurl transport can fail on a host where Python HTTPS succeeds. This
helper changes transport only: index records and crate archives retain their
official checksums, which Cargo independently checks. Never use a public bind.
Run for one build, pass --config source overrides, then stop the helper.
"""
import argparse
import hashlib
import http.server
import json
import pathlib
import re
import threading
import urllib.request
import urllib.error

parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=18473)
args = parser.parse_args()
ROOT = pathlib.Path(__file__).resolve().parents[1] / '.local' / 'cargo-official-cache'
ROOT.mkdir(parents=True, exist_ok=True)
locks = {}
guard = threading.Lock()
semaphore = threading.Semaphore(16)
OFFICIAL = pathlib.Path.home() / '.cargo' / 'registry'


def validate_index(data, crate_name):
    records = data.splitlines()
    if not records:
        raise ValueError('Official index is empty')
    for raw in records:
        record = json.loads(raw)
        if str(record.get('name', '')).lower() != crate_name.lower() or not isinstance(record.get('vers'), str) or not re.fullmatch(r'[a-f0-9]{64}', record.get('cksum', '')):
            raise ValueError('Response is not an official newline crate index')


def fetch_official(url, path, is_crate):
    headers = {'User-Agent': 'OpenKoda-build'}
    candidates = [(url, headers)]
    if not is_crate:
        # GitHub's API avoids the unreliable raw/Fastly route observed on this
        # host. Preserve the raw URL as cache key so existing artifacts are reused.
        candidates = [
            ('https://api.github.com/repos/rust-lang/crates.io-index/contents/' + path + '?ref=master',
             {**headers, 'Accept': 'application/vnd.github.raw+json'}),
            (url, headers),
            ('https://index.crates.io/' + path, headers),
        ]
    last_error = None
    for endpoint, request_headers in candidates:
        try:
            request = urllib.request.Request(endpoint, headers=request_headers)
            with urllib.request.urlopen(request, timeout=20) as response:
                final = response.url
                if not final.startswith(('https://api.github.com/repos/rust-lang/crates.io-index/contents/',
                                         'https://raw.githubusercontent.com/rust-lang/crates.io-index/',
                                         'https://index.crates.io/', 'https://static.crates.io/')):
                    raise ValueError('Unexpected download redirect')
                data = response.read(64 * 1024 * 1024 + 1)
            if len(data) > 64 * 1024 * 1024:
                raise ValueError('Official artifact exceeds transport limit')
            if not is_crate:
                validate_index(data, path.rsplit('/', 1)[-1])
            return data, endpoint
        except (OSError, ValueError) as error:
            # Includes HTTP 403/429 rate limits: fallback remains exclusively on
            # the official raw repository or original crates.io service.
            last_error = error
    raise last_error


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        path = self.path.split('?', 1)[0].lstrip('/')
        if path == 'config.json':
            self.reply(json.dumps({'dl': f'http://127.0.0.1:{args.port}/crates',
                                   'api': 'https://crates.io'}).encode())
            return
        crate = re.fullmatch(r'crates/([A-Za-z0-9_-]+)/([A-Za-z0-9.+_-]+)/download', path)
        if crate:
            name, version = crate.groups()
            url = f'https://static.crates.io/crates/{name}/{name}-{version}.crate'
            cached = OFFICIAL / 'cache' / 'index.crates.io-1949cf8c6b5b557f' / f'{name}-{version}.crate'
            if cached.is_file():
                self.reply(cached.read_bytes())
                return
        elif re.fullmatch(r'(?:[a-z0-9_-]{1,2}/){1,2}[A-Za-z0-9_-]+', path):
            # The official index is also published by rust-lang on GitHub.
            url = 'https://raw.githubusercontent.com/rust-lang/crates.io-index/master/' + path
            cached = OFFICIAL / 'index' / 'index.crates.io-1949cf8c6b5b557f' / '.cache' / path
            if cached.is_file():
                records = [part for part in cached.read_bytes().split(b'\0') if part.startswith(b'{')]
                if records:
                    self.reply(b'\n'.join(records) + b'\n')
                    return
        else:
            self.send_error(404)
            return
        key = hashlib.sha256(url.encode()).hexdigest()
        target = ROOT / key
        with guard:
            lock = locks.setdefault(key, threading.Lock())
        try:
            with lock:
                if target.is_file():
                    data = target.read_bytes()
                else:
                    with semaphore:
                        data, transport_url = fetch_official(url, path, bool(crate))
                    target.write_bytes(data)
                    target.with_suffix('.json').write_text(json.dumps({'url': url, 'transportUrl': transport_url, 'bytes': len(data),
                                                                      'sha256': hashlib.sha256(data).hexdigest()}))
                    print(f'Fetched {path}: {len(data)} bytes', flush=True)
            self.reply(data)
        except urllib.error.HTTPError as error:
            self.send_error(error.code)
        except Exception as error:
            print(f'Fetch failed {path}: {type(error).__name__}', flush=True)
            self.send_error(502, 'Official upstream fetch failed')

    def reply(self, data):
        self.send_response(200)
        self.send_header('Content-Type', 'application/octet-stream')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (ConnectionResetError, BrokenPipeError):
            pass


class BuildServer(http.server.ThreadingHTTPServer):
    # Cargo can open many HTTP/1 connections at once. The default backlog of
    # five caused Windows connect timeouts even for artifacts already cached.
    request_queue_size = 256
    allow_reuse_address = False

server = BuildServer(('127.0.0.1', args.port), Handler)
print(f'Official Cargo transport ready on 127.0.0.1:{args.port}', flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
