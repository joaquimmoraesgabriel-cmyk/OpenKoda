"""Exercise the installed Kali APT and the production helper in a disposable container."""
import base64
from email.utils import formatdate
from functools import partial
import gzip
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import uuid


# An audit hook observes Python subprocess starts in the helper. The helper and
# the Kali executables run unchanged. Network counts alone would miss a second
# invocation that failed before its first HTTP request.
AUDITED_HELPER = """
import json, runpy, sys
log = open(sys.argv[1], 'x', encoding='utf-8', buffering=1)
def record(event, args):
    if event == 'subprocess.Popen':
        log.write(json.dumps(args[1]) + '\\n')
sys.addaudithook(record)
sys.argv = sys.argv[2:]
runpy.run_path(sys.argv[0], run_name='__main__')
"""


def check_tmpfs(directory):
    for line in Path("/proc/self/mountinfo").read_text().splitlines():
        mount, kind = line.split(" - ", 1)
        if mount.split()[4] == str(directory) and kind.split()[0] == "tmpfs":
            return
    raise RuntimeError(f"Expected disposable tmpfs at {directory}")


def make_repository(path):
    # Incompressible synthetic metadata forces APT's actual write past the 2 MiB limit.
    text = base64.b64encode(hashlib.shake_256(b"OpenKoda APT disk fixture").digest(4 * 1024**2))
    package = b"Package: fixture\nVersion: 1\nArchitecture: all\nDescription: fixture\n"
    package += b"".join(b" " + text[i:i + 128] + b"\n" for i in range(0, len(text), 128))
    entries = {"Packages": package, "Packages.gz": gzip.compress(package, mtime=0)}
    release = f"Origin: OpenKoda fixture\nLabel: OpenKoda fixture\nDate: {formatdate(usegmt=True)}\nSHA256:\n"
    for name, data in entries.items():
        (path / name).write_bytes(data)
        release += f" {hashlib.sha256(data).hexdigest()} {len(data)} {name}\n"
    (path / "Release").write_text(release)
    with tempfile.TemporaryDirectory() as keys:
        gpg = ["gpg", "--homedir", keys, "--batch", "--pinentry-mode", "loopback", "--passphrase", ""]
        subprocess.run([*gpg, "--quick-generate-key", "OpenKoda lab <apt@invalid.example>", "rsa2048", "sign", "1d"], check=True)
        with Path("/etc/openkoda/lab.gpg").open("xb") as out:
            subprocess.run([*gpg, "--export"], stdout=out, check=True)
        subprocess.run([*gpg, "--clearsign", "--output", str(path / "InRelease"), str(path / "Release")], check=True)


def main():
    state = Path("/var/lib/openkoda")
    config = Path("/etc/openkoda")
    lists = state / "apt/lists"
    if not Path("/.dockerenv").exists():
        raise RuntimeError("Run in the disposable Kali container")
    if os.geteuid() != 0 or set(os.listdir('/sys/class/net')) != {'lo'}:
        raise RuntimeError('Expected the isolated root container with loopback only')
    for directory in (state, config, lists):
        check_tmpfs(directory)
    if any(config.iterdir()) or (state / "maintenance.json").exists() or any(lists.iterdir()):
        raise RuntimeError("Expected fresh temporary volumes")
    capacity = subprocess.check_output(["df", "-B1", "--output=size", str(lists)], text=True)
    assert 0 < int(capacity.splitlines()[-1]) <= 2 * 1024**2
    identity = str(uuid.uuid4())
    (config / "environment-id").write_text(identity)
    (lists / "partial").mkdir()
    requests = []

    class Handler(SimpleHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            super().do_GET()

    def run_helper(action, log):
        cmd = [sys.executable, '-I', '-c', AUDITED_HELPER, str(log), "/lab/environment-helper.py", action, identity]
        run = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        if not run.stdout.strip():
            raise RuntimeError(f'Helper {action} exited {run.returncode} without JSON: {run.stderr[-2000:]}')
        return run.returncode, json.loads(run.stdout), [json.loads(line) for line in log.read_text().splitlines()]

    with tempfile.TemporaryDirectory() as folder:
        repository = Path(folder) / 'repo'
        repository.mkdir()
        make_repository(repository)
        with ThreadingHTTPServer(("127.0.0.1", 0), partial(Handler, directory=str(repository))) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                url = f"http://127.0.0.1:{server.server_port}/"
                (config / "kali.list").write_text(f"deb [signed-by=/etc/openkoda/lab.gpg] {url} ./\n")
                code, failed, refresh_commands = run_helper("refresh", Path(folder) / 'refresh.jsonl')
                assert len(refresh_commands) == 1 and refresh_commands[0][0] == '/usr/bin/apt-get' and refresh_commands[0][-1] == 'update', refresh_commands
                assert code and not failed["ok"] and "No space left on device" in failed["error"], failed
                journal_path = state / "maintenance.json"
                journal_bytes = journal_path.read_bytes()
                journal = json.loads(journal_bytes)
                assert journal['action'] == 'refresh' and journal["status"] == "unknown" and journal["refreshSucceeded"] is False
                assert any(name.endswith("/InRelease") for name in requests)
                assert any("/Packages" in name for name in requests)
                count = len(requests)
                code, reconciled, reconcile_commands = run_helper("reconcile", Path(folder) / 'reconcile.jsonl')
                assert code == 0 and reconciled["ok"], reconciled
                assert reconcile_commands == [['/usr/bin/dpkg', '--audit']], reconcile_commands
                assert reconciled['result']['originalSha256'] == hashlib.sha256(journal_bytes).hexdigest()
                observed = reconciled["result"]["maintenance"]
                assert observed["status"] == "reconciled" and observed["refreshSucceeded"] is False
                assert observed == json.loads(journal_path.read_bytes())
                assert observed["quiescent"] and len(requests) == count
                print(json.dumps({"passed": True, "indexVolumeBytes": int(capacity.splitlines()[-1]),
                                  "aptReportedENOSPC": True, "statusAfterFailure": journal["status"],
                                  "statusAfterReconciliation": observed["status"], "repeatedRequests": 0,
                                  "refreshAptInvocations": 1, "reconcileAptInvocations": 0,
                                  "reconcileCommands": reconcile_commands}))
            finally:
                server.shutdown()
                thread.join()


if __name__ == "__main__":
    main()
