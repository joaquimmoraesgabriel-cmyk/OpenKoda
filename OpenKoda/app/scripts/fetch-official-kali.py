"""Cache the official WSL Kali image and verify the manifest's SHA-256.

No WSL process, registry, distribution or user project is changed by this helper.
"""
import hashlib
import json
import pathlib
import urllib.request
import urllib.parse

MANIFEST = 'https://api.github.com/repos/microsoft/WSL/contents/distributions/DistributionInfo.json?ref=master'
ROOT = pathlib.Path(__file__).resolve().parents[1] / '.local' / 'official-kali'
LIMIT = 2 * 1024 * 1024 * 1024
ROOT.mkdir(parents=True, exist_ok=True)
request = urllib.request.Request(MANIFEST, headers={'Accept':'application/vnd.github.raw+json', 'User-Agent':'OpenKoda-build'})
with urllib.request.urlopen(request, timeout=45) as response:
    raw = response.read(2 * 1024 * 1024 + 1)
if len(raw) > 2 * 1024 * 1024:
    raise RuntimeError('Official manifest exceeds the expected limit')
manifest = json.loads(raw)
item = next(d for d in manifest['ModernDistributions']['kali'] if d['Name'] == 'kali-linux')
image = item['Amd64Url']
url = urllib.parse.urlparse(image['Url'])
expected = image['Sha256'].lower()
if url.scheme != 'https' or url.hostname != 'kali.download' or not url.path.startswith('/wsl-images/') or not url.path.endswith('.wsl') or len(expected) != 64 or any(c not in '0123456789abcdef' for c in expected):
    raise RuntimeError('Unexpected official Kali image declaration')
target = ROOT / (expected + '.wsl')
def digest_file(path):
    digest = hashlib.sha256()
    with path.open('rb') as file:
        while block := file.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()
if target.exists() and digest_file(target) != expected:
    raise RuntimeError('Existing cache failed integrity verification; it was preserved for inspection')
if not target.exists():
    temporary = ROOT / (expected + '.partial')
    with urllib.request.urlopen(urllib.request.Request(image['Url'], headers={'User-Agent':'OpenKoda-build'}), timeout=60) as response:
        if urllib.parse.urlparse(response.url).scheme != 'https':
            raise RuntimeError('Insecure image redirect refused')
        if int(response.headers.get('Content-Length',0)) > LIMIT:
            raise RuntimeError('Image exceeds the download limit')
        digest = hashlib.sha256()
        total = 0
        next_progress = 64 * 1024 * 1024
        with temporary.open('wb') as file:
            while block := response.read(1024 * 1024):
                total += len(block)
                if total > LIMIT:
                    raise RuntimeError('Image exceeds the download limit')
                digest.update(block)
                file.write(block)
                if total >= next_progress:
                    print(json.dumps({'downloadedMiB':total//(1024*1024)}),flush=True)
                    next_progress += 64 * 1024 * 1024
    if digest.hexdigest() != expected:
        raise RuntimeError('Official image checksum mismatch; installation is not permitted')
    temporary.replace(target)
receipt = {'verified':True,'manifestUrl':MANIFEST,'manifestSha256':hashlib.sha256(raw).hexdigest(),'url':image['Url'],'sha256':expected,'path':str(target),'bytes':target.stat().st_size}
(ROOT / 'manifest.json').write_bytes(raw)
(ROOT / 'verified-image.json').write_text(json.dumps(receipt,indent=2),encoding='utf-8')
print(json.dumps(receipt,indent=2),flush=True)
