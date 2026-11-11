"""Fixed bootstrap envelope for the embedded executor; no caller code paths."""
import base64
import binascii
import hashlib
import json
import signal
import sys
import time
import zlib

signal.alarm(45)
session = sys.argv[sys.argv.index('--openkoda-session') + 1]
deadline = int(sys.argv[sys.argv.index('--openkoda-deadline') + 1])


def unique(items):
    value = dict(items)
    if len(value) != len(items):
        raise ValueError('Duplicate bootstrap field')
    return value


def unpack_bootstrap(raw):
    if len(raw) > 65536 or not raw.endswith(b'\n'):
        raise ValueError('Bootstrap envelope exceeds limit')
    value = json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    keys = {'version', 'sourceZlibBase64', 'sourceBytes', 'sourceSha256', 'environmentId',
            'sessionId', 'assessmentId', 'mode', 'capture', 'control'}
    if not isinstance(value, dict) or set(value) != keys or value['version'] != 'openkoda.executor-bootstrap.v2':
        raise ValueError('Unsupported bootstrap envelope')
    if type(value['sourceBytes']) is not int or not 0 < value['sourceBytes'] <= 128 * 1024:
        raise ValueError('Invalid embedded source size')
    encoded = value['sourceZlibBase64']
    if not isinstance(encoded, str):
        raise ValueError('Invalid compressed source')
    compressed = base64.b64decode(encoded, validate=True)
    if base64.b64encode(compressed).decode('ascii') != encoded:
        raise ValueError('Noncanonical compressed source')
    decoder = zlib.decompressobj()
    source = decoder.decompress(compressed, value['sourceBytes'] + 1)
    if len(source) != value['sourceBytes'] or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
        raise ValueError('Incomplete or excessive embedded source')
    if hashlib.sha256(source).hexdigest() != value['sourceSha256']:
        raise ValueError('Embedded source digest differs')
    return value, source.decode('utf-8', errors='strict')


r, source = unpack_bootstrap(sys.stdin.buffer.readline(65537))
if r['sessionId'] != session or time.time() > deadline:
    raise ValueError('Expired or foreign bootstrap session')
__openkoda_source_sha256__ = r['sourceSha256']
__openkoda_mode__, __openkoda_capture__, __openkoda_control__ = r['mode'], r['capture'], r['control']
sys.argv = ['openkoda-mcp', '--environment-id', r['environmentId'], '--session-id', r['sessionId'], '--assessment-id', r['assessmentId']]
exec(compile(source, 'openkoda-kali-executor', 'exec'))
