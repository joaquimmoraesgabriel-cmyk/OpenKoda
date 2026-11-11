"""Compare every byte, accounting only for Tauri's documented NSIS bundle marker."""
import hashlib
import json
import pathlib
import sys

source = pathlib.Path(sys.argv[1]).read_bytes()
installed = pathlib.Path(sys.argv[2]).read_bytes()
unknown = b'__TAURI_BUNDLE_TYPE_VAR_UNK'
nsis = b'__TAURI_BUNDLE_TYPE_VAR_NSS'
if len(source) != len(installed):
    raise SystemExit('Packaged executable length differs')
differences = [i for i, (left, right) in enumerate(zip(source, installed)) if left != right]
matched = source == installed
for start in range(len(source)):
    if not matched and source.startswith(unknown, start):
        expected = source[:start] + nsis + source[start + len(unknown):]
        if expected == installed:
            matched = True
            break
if not matched:
    raise SystemExit('Installed executable differs beyond the Tauri NSIS marker')
print(json.dumps({'verified': True, 'differingBytes': len(differences),
                  'sha256': hashlib.sha256(installed).hexdigest(),
                  'sourceSha256': hashlib.sha256(source).hexdigest()}))
