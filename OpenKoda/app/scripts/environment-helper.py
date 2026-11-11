"""Fixed, private maintenance helper for the managed OpenKoda Kali only.

Invoked by the native broker with a fixed action and host-issued environment ID.
This is not a shell endpoint. stdout is one JSON result; package output is data.
"""
import fcntl
import hashlib
import json
import os
import pathlib
import pwd
import re
import signal
import subprocess
import sys
import time

ROOT = pathlib.Path('/var/lib/openkoda')
MARKER = pathlib.Path('/etc/openkoda/environment-id')
MAX_OUTPUT = 512 * 1024
PACKAGE = re.compile(r'^[a-z0-9][a-z0-9+.-]*(?::[a-z0-9]+)?$')
VERSION = re.compile(r'^[A-Za-z0-9.+:~\-]+$')
ENV = {'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'DEBIAN_FRONTEND': 'noninteractive'}
APT_OPTIONS = ['-o', 'Dir::Etc::sourcelist=/etc/openkoda/kali.list', '-o', 'Dir::Etc::sourceparts=-',
               '-o', 'Dir::State::lists=/var/lib/openkoda/apt/lists', '-o', 'Acquire::Retries=1',
               '-o', 'Acquire::Languages=none',
               '-o', 'Acquire::IndexTargets::deb::Contents-deb::DefaultEnabled=false',
               '-o', 'Acquire::IndexTargets::deb::Contents-deb-legacy::DefaultEnabled=false',
               '-o', 'Acquire::IndexTargets::deb::Contents-udeb::DefaultEnabled=false',
               '-o', 'Acquire::http::Timeout=30', '-o', 'Acquire::https::Timeout=30',
               '-o', 'DPkg::Lock::Timeout=0', '-o', 'APT::Get::AllowUnauthenticated=false']


class CommandFailed(RuntimeError):
    """A completed command with a nonzero exit, distinct from uncertain capture."""
    def __init__(self, returncode, stdout, stderr):
        self.returncode, self.stdout, self.stderr = returncode, stdout, stderr
        super().__init__('Command failed (' + str(returncode) + '): ' + (stderr or stdout)[-12000:])


def atomic_json(path, value):
    temporary = path.with_suffix('.new')
    with temporary.open('w', encoding='utf-8') as file:
        json.dump(value, file, ensure_ascii=True)
        file.flush()
        os.fsync(file.fileno())
    temporary.replace(path)


def maintenance_snapshot():
    path = ROOT / 'maintenance.json'
    if path.is_symlink():
        raise RuntimeError('Maintenance journal cannot be a link')
    if not path.exists():
        return None, None, None
    with path.open('rb') as file:
        raw = file.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise RuntimeError('Maintenance journal exceeds the capture limit')
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise RuntimeError('Maintenance journal is malformed')
    return value, hashlib.sha256(raw).hexdigest(), raw


def verified_repair_receipt(journal, sha256, raw):
    # The fixed native broker supplies this only after the reviewed --terminate
    # completed and --list --running proved this exact owned distro stopped.
    # Linux root is an explicit trust boundary; no WebView-supplied receipt is used.
    if len(sys.argv) != 4:
        raise RuntimeError('Missing native termination receipt')
    receipt = json.loads(sys.argv[3])
    fields = {'repairId', 'planId', 'environmentId', 'distro', 'maintenanceSha256',
              'terminationVerified', 'verifiedAtUnixSeconds'}
    if not isinstance(receipt, dict) or set(receipt) != fields:
        raise RuntimeError('Invalid native termination receipt fields')
    uuid_pattern = r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}'
    if any(not isinstance(receipt[key], str) or not re.fullmatch(uuid_pattern, receipt[key])
           for key in ('repairId', 'planId', 'environmentId')):
        raise RuntimeError('Invalid native termination identity')
    issued = receipt['verifiedAtUnixSeconds']
    if receipt['terminationVerified'] is not True or receipt['distro'] != 'OpenKoda-Kali' \
            or receipt['environmentId'] != sys.argv[2] or type(issued) is not int \
            or not -30 <= time.time() - issued <= 180:
        raise RuntimeError('Unverified, expired or mismatched termination receipt')
    if not journal or receipt['maintenanceSha256'] != sha256 \
            or journal.get('action') not in ('refresh', 'install') \
            or journal.get('status') not in ('running', 'unknown'):
        raise RuntimeError('Maintenance journal changed or no longer needs this repair')
    history = ROOT / 'maintenance-history'
    if history.is_symlink():
        raise RuntimeError('Maintenance history cannot be a link')
    history.mkdir(mode=0o700, exist_ok=True)
    record = {'format': 1, 'receipt': receipt, 'originalSha256': sha256,
              'originalRaw': raw.decode('utf-8'), 'archivedAt': time.time()}
    # Exclusive creation prevents receipt replay. Audit failures retain both the
    # untouched original journal and this immutable history entry.
    with (history / (receipt['repairId'] + '.json')).open('x', encoding='utf-8') as file:
        json.dump(record, file, ensure_ascii=True)
        file.flush()
        os.fsync(file.fileno())
    return receipt


def process_start(pid):
    try:
        return pathlib.Path('/proc/' + str(pid) + '/stat').read_text().rsplit(') ', 1)[1].split()[19]
    except (OSError, IndexError):
        return None


def boot_id():
    value = pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    if not re.fullmatch(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', value):
        raise RuntimeError('Kernel boot identity is unavailable; maintenance cannot be confirmed')
    return value


def execute(argv, timeout=60, journal=None):
    # Output goes to bounded files rather than in-memory communicate() buffers.
    # The supervisor kills the whole group when timeout/output limits are reached.
    import tempfile
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        if journal is not None:
            # Persist the boot before allowing a child to exist. A crash between
            # Popen and the PID record must remain unresolved in this boot, even
            # when the child has not acquired an APT/dpkg lock yet.
            journal.update(bootId=boot_id(), processPid=None, processStart=None)
            atomic_json(ROOT / 'maintenance.json', journal)
        process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                   env=ENV, start_new_session=True, close_fds=True)
        if journal is not None:
            journal.update(processPid=process.pid, processStart=process_start(process.pid))
            atomic_json(ROOT / 'maintenance.json', journal)
        deadline = time.monotonic() + timeout
        reason = None
        while process.poll() is None:
            if time.monotonic() > deadline:
                reason = 'timeout'
            elif os.fstat(out.fileno()).st_size + os.fstat(err.fileno()).st_size > MAX_OUTPUT:
                reason = 'output_limit'
            if reason:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
                break
            time.sleep(0.05)
        out.seek(0)
        err.seek(0)
        # A fast command can exit before the polling loop observes its output.
        if os.fstat(out.fileno()).st_size + os.fstat(err.fileno()).st_size > MAX_OUTPUT:
            reason = reason or 'output_limit'
        stdout = out.read(MAX_OUTPUT).decode('utf-8', 'replace')
        stderr = err.read(MAX_OUTPUT).decode('utf-8', 'replace')
        if reason:
            raise RuntimeError('Maintenance limited: ' + reason + '. Inspect dpkg before retrying.')
        if process.returncode:
            raise CommandFailed(process.returncode, stdout, stderr)
        return stdout


def package_status(name):
    # dpkg-query documents 1 as no match, 2 as a fatal database/system error:
    # https://manpages.debian.org/trixie/dpkg/dpkg-query.1.en.html
    try:
        value = execute(['/usr/bin/dpkg-query', '-W', '-f=${db:Status-Abbrev}\t${Version}', name], 10)
    except CommandFailed as error:
        if error.returncode == 1 and not error.stdout.strip():
            return {'package': name, 'installed': False, 'version': None}
        raise
    match = re.fullmatch(r'([uihrp][ncHUFWti][ R])\t([^\r\n\t]*)', value)
    if not match:
        raise RuntimeError('Package state is unknown: malformed dpkg-query response for ' + name)
    status, version = match.groups()
    if status[2] != ' ' or status[1] not in ('i', 'n', 'c'):
        raise RuntimeError('Package state needs repair; not classified as absent: ' + name)
    installed = status[1] == 'i'  # Held or pending-removal packages remain installed.
    if installed and not VERSION.fullmatch(version):
        raise RuntimeError('Package version is unknown: ' + name)
    return {'package': name, 'installed': installed, 'version': version if installed else None}


def require_identity(identity):
    if not re.fullmatch(r'[0-9a-f-]{36}', identity):
        raise RuntimeError('Invalid environment identity')
    if MARKER.is_symlink() or not MARKER.is_file() or MARKER.read_text().strip() != identity:
        raise RuntimeError('Managed environment identity does not match')
    release = pathlib.Path('/etc/os-release').read_text()
    if not re.search(r'^ID=[\"\']?kali[\"\']?$', release, re.M):
        raise RuntimeError('The registered distribution is not Kali')


def fingerprint():
    digest = hashlib.sha256()
    for path in [pathlib.Path('/var/lib/dpkg/status'), pathlib.Path('/etc/openkoda/kali.list')]:
        digest.update(str(path).encode())
        digest.update(path.read_bytes())
    for path in sorted((ROOT / 'apt/lists').glob('*')):
        if path.is_file() and path.name != 'lock':
            digest.update(path.name.encode())
            with path.open('rb') as file:
                while chunk := file.read(65536):
                    digest.update(chunk)
    return digest.hexdigest()


def simulate(packages):
    output = execute(['/usr/bin/apt-get', *APT_OPTIONS, '--simulate', '--no-install-recommends', 'install', *packages], 120)
    if re.search(r'^Remv ', output, re.M):
        raise RuntimeError('This plan removes packages and is not supported')
    resolved = []
    for name, version in re.findall(r'^Inst (\S+)(?: \[[^\]]*\])? \((\S+)', output, re.M):
        if not PACKAGE.fullmatch(name) or not VERSION.fullmatch(version):
            raise RuntimeError('Unexpected package identity in simulation')
        resolved.append(name + '=' + version)
    return {'resolved': sorted(resolved), 'output': output, 'fingerprint': fingerprint()}


def wsl_configuration(original):
    import configparser
    import io
    config = configparser.ConfigParser(strict=True, interpolation=None)
    config.optionxform = str
    config.read_string(original)
    for section, values in {'interop': {'enabled': 'false', 'appendWindowsPath': 'false'},
                            'automount': {'enabled': 'false'}, 'user': {'default': 'openkoda'}}.items():
        if not config.has_section(section):
            config.add_section(section)
        for key, value in values.items():
            for existing in list(config[section]):
                if existing.lower() == key.lower():
                    config.remove_option(section, existing)
            config.set(section, key, value)
    output = io.StringIO()
    config.write(output)
    return output.getvalue()


def interop_enabled(registration):
    if registration is None:
        return False
    # Unknown kernel state remains conservative; only an explicit disabled
    # registration (or no registration) demonstrates no interop handler.
    lines = registration.splitlines()
    return not lines or lines[0].strip().lower() != 'disabled'


def windows_mounts(mount_table):
    mounts = []
    for line in mount_table.splitlines():
        fields = line.split()
        if len(fields) >= 4 and (fields[2] == 'drvfs' or re.match(r'^[A-Za-z]:', fields[0]) or 'aname=drvfs' in fields[3]):
            mounts.append(fields[1])
    return mounts


def main():
    if len(sys.argv) < 3:
        raise RuntimeError('Missing fixed action and environment identity')
    action, identity = sys.argv[1:3]
    if action == 'bootstrap':
        if os.geteuid() != 0 or not re.fullmatch(r'[0-9a-f-]{36}', identity):
            raise RuntimeError('Bootstrap requires the managed root channel')
        release = pathlib.Path('/etc/os-release').read_text()
        if not re.search(r'^ID=[\"\']?kali[\"\']?$', release, re.M):
            raise RuntimeError('Only the official Kali environment is supported')
        if MARKER.exists() and MARKER.read_text().strip() != identity:
            raise RuntimeError('Refusing to adopt another managed environment')
        MARKER.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
        if not MARKER.exists():
            with MARKER.open('x') as file:
                file.write(identity + '\n')
                file.flush()
                os.fsync(file.fileno())
            MARKER.chmod(0o644)
        try:
            user = pwd.getpwnam('openkoda')
        except KeyError:
            execute(['/usr/sbin/useradd', '--create-home', '--shell', '/bin/bash', 'openkoda'])
            user = pwd.getpwnam('openkoda')
        if user.pw_uid == 0:
            raise RuntimeError('Assessment user must not be root')
        ROOT.mkdir(mode=0o755, parents=True, exist_ok=True)
        (ROOT / 'apt/lists/partial').mkdir(mode=0o755, parents=True, exist_ok=True)
        workspace = ROOT / 'workspaces'
        workspace.mkdir(mode=0o700, exist_ok=True)
        os.chown(workspace, user.pw_uid, user.pw_gid)
        sources = pathlib.Path('/etc/openkoda/kali.list')
        sources.write_text('deb [signed-by=/usr/share/keyrings/kali-archive-keyring.gpg] https://http.kali.org/kali kali-rolling main contrib non-free non-free-firmware\n')
        if not pathlib.Path('/usr/share/keyrings/kali-archive-keyring.gpg').is_file():
            raise RuntimeError('Official Kali archive keyring missing')
        # Preserve non-managed settings and retain the original for diagnosis.
        configuration = pathlib.Path('/etc/wsl.conf')
        original = configuration.read_text() if configuration.exists() else ''
        backup = pathlib.Path('/etc/openkoda/wsl.conf.original')
        if not backup.exists():
            backup.write_text(original)
        with configuration.open('w') as file:
            file.write(wsl_configuration(original))
            file.flush()
            os.fsync(file.fileno())
        return {'bootstrapped': True, 'user': 'openkoda', 'uid': user.pw_uid, 'restartRequired': True}

    require_identity(identity)
    if action == 'probe':
        user = pwd.getpwnam('openkoda')
        operation, maintenance_sha256, _ = maintenance_snapshot()
        interop_file = pathlib.Path('/proc/sys/fs/binfmt_misc/WSLInterop')
        interop = interop_enabled(interop_file.read_text() if interop_file.exists() else None)
        mounts = windows_mounts(pathlib.Path('/proc/mounts').read_text())
        return {'environmentId': identity, 'user': user.pw_name, 'uid': user.pw_uid,
                'kernel': os.uname().release, 'interopEnabled': interop, 'windowsMounts': mounts,
                'maintenance': operation, 'maintenanceSha256': maintenance_sha256,
                'containment': 'Managed WSL; not a hostile-code sandbox',
                'executorReady': False}

    if action not in ('catalog', 'refresh', 'simulate', 'apply', 'audit', 'reconcile', 'repair'):
        raise RuntimeError('Unknown maintenance action')
    if os.geteuid() != 0:
        raise RuntimeError('Maintenance requires the separate managed root channel')
    lock = (ROOT / 'maintenance.lock').open('a')
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        lock.close()
        raise RuntimeError('Another managed maintenance operation is active')
    try:
        return maintenance_action(action)
    finally:
        lock.close()


def maintenance_action(action):
    if action in ('reconcile', 'repair'):
        journal_path = ROOT / 'maintenance.json'
        journal, original_sha256, raw = maintenance_snapshot()
        receipt = verified_repair_receipt(journal, original_sha256, raw) if action == 'repair' else None
        if journal and receipt is None:
            current_boot = boot_id()
            recorded_boot = journal.get('bootId')
            known_boot = isinstance(recorded_boot, str) and re.fullmatch(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', recorded_boot)
            pid, started = journal.get('processPid'), journal.get('processStart')
            known_process = type(pid) is int and pid > 0 and isinstance(started, str) and started.isdecimal()
            if journal.get('status') != 'completed' and (not known_boot or (recorded_boot == current_boot and not known_process)):
                raise RuntimeError('Maintenance launch identity is incomplete; free locks do not prove termination. '
                                   'The journal was preserved. Reconciliation requires a confirmed different recorded boot.')
            observed_start = process_start(pid) if known_process and recorded_boot == current_boot else None
            if observed_start is not None and observed_start == started:
                raise RuntimeError('The recorded maintenance process is still running; no operation was repeated')
        # Process identity/boot evidence is required before these supplementary
        # locks. Refresh holds the lists lock, not necessarily either dpkg lock.
        # Keep all three advisory locks until observations have been journaled.
        locks = []
        try:
            for name in ('/var/lib/dpkg/lock-frontend', '/var/lib/dpkg/lock', str(ROOT / 'apt/lists/lock')):
                handle = open(name, 'a+')
                locks.append(handle)
                fcntl.lockf(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            audit = execute(['/usr/bin/dpkg', '--audit'])
            if audit.strip():
                raise RuntimeError('Package repair required: ' + audit)
            if journal and journal.get('status') != 'completed':
                if journal.get('action') == 'install':
                    for pin in journal.get('resolved', []):
                        name, version = pin.split('=', 1)
                        if not PACKAGE.fullmatch(name) or not VERSION.fullmatch(version):
                            raise RuntimeError('Invalid recovery package identity')
                        actual = package_status(name)
                        if not actual['installed'] or actual['version'] != version:
                            raise RuntimeError('Installation was partial; inspect and repair packages before resuming')
                    journal.update(status='completed', reconciled=True, quiescent=True, endedAt=time.time(),
                                   resolution='Observed exact installed versions; no maintenance command repeated')
                elif journal.get('action') == 'refresh':
                    journal.update(status='reconciled', reconciled=True, quiescent=True, refreshSucceeded=False,
                                   endedAt=time.time(), resolution='Processes and locks are quiescent; index refresh success was not established. Request a new refresh explicitly.')
                else:
                    raise RuntimeError('Unknown maintenance action cannot be reconciled as successful')
                if receipt is not None:
                    # A fresh audit witness makes future ordinary reconciliation
                    # possible even when WSL reused the same utility-VM boot ID.
                    witness_start = process_start(os.getpid())
                    if not isinstance(witness_start, str) or not witness_start.isdecimal():
                        raise RuntimeError('Repair audit witness could not be recorded')
                    journal.update(repairId=receipt['repairId'], originalSha256=original_sha256,
                                   terminationReceipt=receipt, bootId=boot_id(), processPid=os.getpid(),
                                   processStart=witness_start, witnessRole='repair-audit-helper')
                atomic_json(journal_path, journal)
            return {'clean': True, 'reconciled': bool(journal), 'maintenance': journal,
                    'originalSha256': original_sha256,
                    'repairId': receipt['repairId'] if receipt is not None else None}
        except BlockingIOError:
            raise RuntimeError('APT/dpkg still holds a lock; do not restart maintenance')
        finally:
            for handle in locks:
                handle.close()
    if action == 'audit':
        output = execute(['/usr/bin/dpkg', '--audit'])
        return {'clean': not output.strip(), 'output': output}
    if action == 'catalog':
        names = sys.argv[3:]
        if not names or any(not PACKAGE.fullmatch(name) for name in names):
            raise RuntimeError('Invalid catalog packages')
        return {'packages': [package_status(name) for name in names]}
    if action == 'refresh':
        journal = {'action': 'refresh', 'status': 'running', 'startedAt': time.time()}
        atomic_json(ROOT / 'maintenance.json', journal)
        try:
            # Fail on transient acquisition errors too; default update may reuse
            # old lists and exit zero. Unsupported APT must fail, never weaken it.
            # https://manpages.debian.org/trixie/apt/apt-get.8.en.html
            output = execute(['/usr/bin/apt-get', *APT_OPTIONS, '--error-on=any', 'update'], 600, journal)
            journal.update(status='completed', refreshSucceeded=True, endedAt=time.time())
            atomic_json(ROOT / 'maintenance.json', journal)
            return {'output': output, 'fingerprint': fingerprint()}
        except Exception as error:
            journal.update(status='unknown', refreshSucceeded=False, error=str(error)[-12000:], endedAt=time.time())
            atomic_json(ROOT / 'maintenance.json', journal)
            raise
    if action == 'simulate':
        names = sys.argv[3:]
        if not names or any(not PACKAGE.fullmatch(name) for name in names):
            raise RuntimeError('Invalid package selection')
        return simulate(names)
    # apply receives one structured host-owned plan, never shell text.
    plan = json.loads(sys.argv[3])
    if not isinstance(plan.get('packages'), list) or not plan['packages'] or len(plan['packages']) > 12 or any(not isinstance(name, str) or not PACKAGE.fullmatch(name) for name in plan['packages']):
        raise RuntimeError('Invalid package plan')
    current = simulate(plan['packages'])
    if current['fingerprint'] != plan['fingerprint'] or current['resolved'] != plan['resolved']:
        raise RuntimeError('Catalog or installed packages changed; calculate and review a new plan')
    if not current['resolved']:
        return {'output': 'Requested packages are already installed', 'changed': False}
    journal = {'action': 'install', 'planId': plan['id'], 'status': 'running', 'startedAt': time.time(),
               'resolved': current['resolved']}
    atomic_json(ROOT / 'maintenance.json', journal)
    try:
        output = execute(['/usr/bin/apt-get', *APT_OPTIONS, '--yes', '--no-remove', '--no-install-recommends',
                          '-o', 'Dpkg::Options::=--force-confold', 'install', *current['resolved']], 900, journal)
        audit = execute(['/usr/bin/dpkg', '--audit'])
        if audit.strip():
            raise RuntimeError('dpkg reported unfinished package state: ' + audit)
        journal.update(status='completed', endedAt=time.time())
        atomic_json(ROOT / 'maintenance.json', journal)
        return {'output': output, 'changed': True, 'fingerprint': fingerprint()}
    except Exception:
        journal.update(status='unknown', endedAt=time.time())
        atomic_json(ROOT / 'maintenance.json', journal)
        raise


if __name__ == '__main__':
    try:
        print(json.dumps({'ok': True, 'result': main()}, ensure_ascii=True))
    except Exception as error:
        print(json.dumps({'ok': False, 'error': str(error)}, ensure_ascii=True))
        sys.exit(1)
