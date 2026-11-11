"""Run only the selected MCP fixtures as non-root in the owned Kali distro.

No application profile, credential, project source or network target is copied.
The temporary test directory is created and removed by Python on Linux.
"""
import base64
import datetime
import hashlib
import json
import pathlib
import subprocess
import sys
import uuid

root = pathlib.Path(__file__).resolve().parent.parent
directory = root / '.local' / ('linux-helper-tests-' + str(uuid.uuid4()))
directory.mkdir(parents=True)
names = ['kali-executor.py', 'kali-bootstrap.py', 'test_kali_executor.py',
         'kali-archive-inspect.py', 'kali-restore-marker.py', 'test_kali_archives.py',
         'environment-helper.py', 'test-environment-helper.py']
files = {name: (root / 'scripts' / name).read_bytes() for name in names}
bootstrap = r'''
import base64,json,os,pathlib,subprocess,sys,tempfile
assert sys.platform=='linux' and os.geteuid()!=0
wire=json.load(sys.stdin)
expected={'kali-executor.py','kali-bootstrap.py','test_kali_executor.py','kali-archive-inspect.py',
          'kali-restore-marker.py','test_kali_archives.py','environment-helper.py','test-environment-helper.py'}
assert set(wire)==expected
with tempfile.TemporaryDirectory(prefix='openkoda-unit-') as folder:
    for name,source in wire.items():
        pathlib.Path(folder,name).write_bytes(base64.b64decode(source,validate=True))
    for test in ['test_kali_executor.py','test_kali_archives.py','test-environment-helper.py']:
        result=subprocess.run([sys.executable,str(pathlib.Path(folder,test)),'-v'],cwd=folder,timeout=300)
        if result.returncode: sys.exit(result.returncode)
'''
payload = json.dumps({name: base64.b64encode(data).decode('ascii') for name,data in files.items()}).encode()
result = {'passed': False, 'directory': str(directory), 'files': {
    name: hashlib.sha256(source).hexdigest() for name, source in files.items()}}
try:
    run = subprocess.run(['C:/Windows/System32/wsl.exe', '--distribution', 'OpenKoda-Kali',
                          '--user', 'openkoda', '--cd', '/', '--exec', '/usr/bin/timeout',
                          '--signal=TERM', '--kill-after=5', '420', '/usr/bin/python3', '-I', '-c', bootstrap],
                         input=payload, capture_output=True, timeout=435)
    log = (run.stdout + b'\n' + run.stderr).decode('utf-8', errors='replace')
    (directory / 'tests.log').write_text(log, encoding='utf-8')
    result.update(exitCode=run.returncode, passed=run.returncode == 0)
    print('\n'.join(log.splitlines()[-38:]))
except subprocess.TimeoutExpired:
    result['error'] = 'The bounded Linux fixture run exceeded its host deadline; inspect only its own test processes.'
finally:
    result['generatedAt'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    (directory / 'result.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))
sys.exit(0 if result['passed'] else 1)
