"""Bounded MCP stdio proof for the dedicated, non-root OpenKoda Kali user.

Pinned MCP 2025-11-25 transport/lifecycle:
https://modelcontextprotocol.io/specification/2025-11-25/basic/transports
https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle

No shell, target paths, network operations, approvals, or credentials are exposed.
One fixed offline ripgrep parser accepts bounded bytes, never caller arguments.
The host must verify WSL registration and this script's content hash
before launch. This module alone does not establish a verified Rust/WSL bridge.
"""
import argparse
import base64
import binascii
import hashlib
import json
import os
import pathlib
import platform
import re
import stat
import sys
import time
import uuid
import tempfile
import select
import signal
import contextlib

PROTOCOL = '2025-11-25'
MAX_LINE = 65536
MAX_BYTES = 32768
MAX_MESSAGES = 128
MAX_TOTAL_INPUT = 4 * 1024 * 1024
MAX_TOTAL_OUTPUT = 65536
MAX_COLLECT_OUTPUT = 128 * 1024
MARKER = pathlib.Path('/etc/openkoda/environment-id')
ADAPTER = 'kali.rg.signals.v1'
RULES_VERSION = '2026-11-07.1'
SCAN_PATTERN = r'-----BEGIN (?:RSA |EC |OPENSSH |DSA |ENCRYPTED )?PRIVATE KEY-----|\b(?:eval|exec)\s*\(|\b(?:api_key|apiKey|password|passwd|secret|token)\b\s*[:=]'
RULE_LABELS = {'private-key-marker': 'Private key marker', 'dynamic-evaluation': 'Dynamic evaluation signal',
               'credential-identifier': 'Credential assignment identifier'}
MAX_SCAN_OUTPUT = 65536
MAX_CANDIDATES = 16
CHILD_SECONDS = 8
CATALOG_VERSION = 'openkoda.kali-tools.v3'
LEGACY_CATALOG_VERSION = 'openkoda.kali-tools.v2'
SPOOL_SCHEMA = 'openkoda.kali-spool.v1'
SPOOL_LIMIT = 16 * 1024 * 1024
SPOOL_COUNT = 256
COLLECT_PAGE = 4096


def tool_catalog():
    byte_schema = {'type': 'object', 'properties': {'base64': {'type': 'string'},
                   'expectedSha256': {'type': 'string', 'pattern': '^[0-9a-f]{64}$'}},
                   'required': ['base64', 'expectedSha256'], 'additionalProperties': False}
    return [
        {'name': 'runtime_diagnostics', 'description': 'Verified runtime identity and session inspection.',
         'inputSchema': {'type': 'object', 'properties': {'inspectSessionId': {'type': 'string'},
                          'dispatchDeadline': {'type': 'integer', 'minimum': 1}}, 'additionalProperties': False}},
        {'name': 'hash_bytes', 'description': 'SHA-256 of host-selected bytes, at most 32 KiB.', 'inputSchema': byte_schema},
        {'name': 'scan_bytes', 'description': 'Fixed offline ripgrep signals in host-selected bytes, at most 32 KiB. Candidates only.',
         'inputSchema': byte_schema},
        {'name': 'capture_collect', 'description': 'Read an existing owned capture without repeating its producer.',
         'inputSchema': {'oneOf': [
             {'type': 'object', 'properties': {'kind': {'const': 'describe'}, 'producerSessionId': {'type': 'string'}, 'captureId': {'type': 'string'}},
              'required': ['kind', 'producerSessionId', 'captureId'], 'additionalProperties': False},
             {'type': 'object', 'properties': {'kind': {'const': 'read'}, 'producerSessionId': {'type': 'string'}, 'captureId': {'type': 'string'},
                 'expectedManifestDigest': {'type': 'string', 'pattern': '^[0-9a-f]{64}$'}, 'offset': {'type': 'integer', 'minimum': 0},
                 'maxBytes': {'type': 'integer', 'minimum': 1, 'maximum': COLLECT_PAGE}},
              'required': ['kind', 'producerSessionId', 'captureId', 'expectedManifestDigest', 'offset', 'maxBytes'], 'additionalProperties': False}]}},
        {'name': 'capture_purge', 'description': 'Remove only a reviewed owned capture with a durable purge identity.',
         'inputSchema': {'type': 'object', 'properties': {'producerSessionId': {'type': 'string'}, 'captureId': {'type': 'string'},
             'expectedManifestDigest': {'type': 'string', 'pattern': '^[0-9a-f]{64}$'}, 'purgeReceiptId': {'type': 'string'},
             'previousReceiptId': {'type': ['string', 'null']}, 'previousReceiptDigest': {'type': ['string', 'null'], 'pattern': '^[0-9a-f]{64}$'}},
             'required': ['producerSessionId', 'captureId', 'expectedManifestDigest', 'purgeReceiptId', 'previousReceiptId', 'previousReceiptDigest'], 'additionalProperties': False}},
        {'name': 'capture_retire', 'description': 'Inspect or consolidate explicitly reviewed terminal capture metadata.',
         'inputSchema': retire_schema()}]


def retire_schema():
    uuid_schema = {'type': 'string'}
    digest = {'type': 'string', 'pattern': '^[0-9a-f]{64}$'}
    nullable_id = {'type': ['string', 'null']}
    nullable_digest = {'type': ['string', 'null'], 'pattern': '^[0-9a-f]{64}$'}
    pending = {'type': ['object', 'null'], 'properties': {
        'retireReceiptId': uuid_schema, 'previousReceiptId': nullable_id,
        'previousReceiptDigest': nullable_digest, 'reviewedStateDigest': digest,
        'producerRecordSha256': digest}, 'required': ['retireReceiptId', 'previousReceiptId',
        'previousReceiptDigest', 'reviewedStateDigest', 'producerRecordSha256'], 'additionalProperties': False}
    common = {'producerSessionId': uuid_schema, 'captureId': uuid_schema, 'ownerProfileFingerprint': digest,
        'producerDispatchDeadline': {'type': 'integer', 'minimum': 1}, 'expectedManifestDigest': digest,
        'purgeReceiptId': uuid_schema, 'pendingAttempt': pending}
    inspect = {'kind': {'const': 'inspect'}, **common, 'expectedPurgeReceiptSha256': nullable_digest}
    apply = {'kind': {'const': 'apply'}, **common, 'expectedPurgeReceiptSha256': digest,
        'retireReceiptId': uuid_schema, 'previousReceiptId': nullable_id,
        'previousReceiptDigest': nullable_digest, 'expectedStateDigest': digest}
    return {'oneOf': [{'type': 'object', 'properties': fields, 'required': list(fields),
                       'additionalProperties': False} for fields in (inspect, apply)]}


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True, allow_nan=False).encode('ascii')


def catalog_sha256(version=None):
    # Human descriptions are not protocol authority; Rust verifies the same schemas.
    tools = tool_catalog()
    if version == LEGACY_CATALOG_VERSION:
        tools = tools[:-1]
    elif version not in (None, CATALOG_VERSION):
        raise ProtocolError(-32001, 'Unsupported capture catalog')
    return hashlib.sha256(canonical([{'name': t['name'], 'inputSchema': t['inputSchema']} for t in tools])).hexdigest()


class ProtocolError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def exact(value, keys, required=None):
    if not isinstance(value, dict) or set(value) - set(keys) or not set(required or ()) <= set(value):
        raise ProtocolError(-32602, 'Invalid or unsupported fields')
    return value


def identifier(value):
    if not isinstance(value, str):
        raise ValueError('Invalid host identity')
    parsed = uuid.UUID(value)
    if str(parsed) != value:
        raise ValueError('Identity must be canonical UUID')
    return value


def maintenance_record_ready(raw):
    """The same readiness states accepted by the native maintenance probe."""
    def unique_object(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError('Duplicate maintenance field')
            value[key] = item
        return value

    def invalid_constant(_value):
        raise ValueError('Invalid maintenance number')

    try:
        journal = json.loads(raw.decode('utf-8'), object_pairs_hook=unique_object,
                             parse_constant=invalid_constant)
    except (ValueError, UnicodeError, RecursionError):
        raise RuntimeError('Maintenance readiness record is malformed') from None
    if not isinstance(journal, dict) or not (
            journal.get('status') == 'completed' or
            (journal.get('status') == 'reconciled' and journal.get('action') == 'refresh'
             and journal.get('quiescent') is True and journal.get('refreshSucceeded') is False)):
        raise RuntimeError('Managed maintenance requires reconciliation')


def verify_maintenance_ready():
    """Read a single root-owned journal; never execute maintenance or accept paths."""
    directory = os.open('/var/lib/openkoda', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        owner = os.fstat(directory)
        if not stat.S_ISDIR(owner.st_mode) or owner.st_uid != 0 or owner.st_mode & 0o022:
            raise RuntimeError('Maintenance directory ownership is invalid')
        try:
            descriptor = os.open('maintenance.json', os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
                                 dir_fd=directory)
        except FileNotFoundError:
            return  # Bootstrap can have no prior package maintenance.
        try:
            owner = os.fstat(descriptor)
            limit = 1024 * 1024
            if not stat.S_ISREG(owner.st_mode) or owner.st_uid != 0 or owner.st_mode & 0o022 \
                    or owner.st_size > limit:
                raise RuntimeError('Maintenance journal ownership or size is invalid')
            raw = bytearray()
            while len(raw) <= limit:
                part = os.read(descriptor, min(65536, limit + 1 - len(raw)))
                if not part:
                    break
                raw.extend(part)
            after = os.fstat(descriptor)
            if len(raw) > limit or (owner.st_dev, owner.st_ino, owner.st_size, owner.st_mtime_ns,
                                   owner.st_mode, owner.st_uid) != (
                    after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns,
                    after.st_mode, after.st_uid):
                raise RuntimeError('Maintenance journal changed or exceeds the capture limit')
            maintenance_record_ready(raw)
        finally:
            os.close(descriptor)
    finally:
        os.close(directory)


def verify_runtime(environment_id):
    """Fixed trusted paths only; tool arguments never reach the filesystem."""
    if sys.platform != 'linux' or os.geteuid() == 0:
        raise RuntimeError('Executor requires non-root Linux user')
    # This check runs before initialize and selected bytes. A separate Linux
    # readiness process would create an unjournaled process before this session.
    verify_maintenance_ready()
    import pwd
    if pwd.getpwuid(os.geteuid()).pw_name != 'openkoda':
        raise RuntimeError('Executor requires dedicated openkoda user')
    metadata = MARKER.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
        raise RuntimeError('Environment marker ownership is invalid')
    if MARKER.read_text(encoding='ascii').strip() != environment_id:
        raise RuntimeError('Environment identity does not match')
    if not re.search(r'^ID=[\"\']?kali[\"\']?$', pathlib.Path('/etc/os-release').read_text(), re.M):
        raise RuntimeError('Executor requires Kali')
    handler = pathlib.Path('/proc/sys/fs/binfmt_misc/WSLInterop')
    interop = handler.exists() and not handler.read_text().startswith('disabled\n')
    mounts = []
    for line in pathlib.Path('/proc/mounts').read_text().splitlines():
        fields = line.split()
        if len(fields) >= 4 and (fields[2] == 'drvfs' or re.match(r'^[A-Za-z]:', fields[0]) or 'aname=drvfs' in fields[3]):
            mounts.append(fields[1])
    if interop or mounts:
        raise RuntimeError('Windows interop and drive mounts must be disabled')
    return {'uid': os.geteuid(), 'user': 'openkoda', 'platform': 'linux',
            'python': platform.python_version(), 'markerVerified': True,
            'interopEnabled': False, 'windowsMounts': [],
            'containment': 'Dedicated WSL user; not a hostile-code sandbox'}


def process_start(pid):
    return pathlib.Path('/proc/' + str(pid) + '/stat').read_text().rsplit(') ', 1)[1].split()[19]


def session_directory():
    import pwd
    home = pathlib.Path(pwd.getpwuid(os.geteuid()).pw_dir)
    descriptor = directory_fd(home)
    try:
        for name in ('.local', 'state', 'openkoda-sessions'):
            metadata = os.fstat(descriptor)
            if metadata.st_uid != os.geteuid() or metadata.st_mode & 0o022:
                raise RuntimeError('Session ancestor ownership is invalid')
            try:
                os.mkdir(name, 0o700, dir_fd=descriptor)
                os.fsync(descriptor)
            except FileExistsError:
                pass
            child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        private_fd(descriptor, True)
    finally:
        os.close(descriptor)
    return home / '.local/state/openkoda-sessions'


def record_session(directory, identity, state, children=None, capture=None):
    record = {**identity, 'pid': os.getpid(), 'startTicks': process_start(os.getpid()),
              'bootId': pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'state': state}
    if children is not None:
        record['children'] = children
    if capture is not None:
        record['capture'] = capture
    descriptor = directory_fd(directory)
    try:
        private_fd(descriptor, True)
        atomic_owned(descriptor, identity['sessionId'] + '.json', record)
    finally:
        os.close(descriptor)


def inspect_without_record(session_id, dispatch_deadline):
    # The fixed bootstrap refuses new work after this deadline. Its immutable
    # process argv carries the session tag even before a marker could be written.
    if isinstance(dispatch_deadline, bool) or not isinstance(dispatch_deadline, int) or dispatch_deadline <= 0:
        return {'sessionId': session_id, 'known': False, 'active': None, 'reason': 'no_dispatch_deadline'}
    if time.time() <= dispatch_deadline + 60:
        return {'sessionId': session_id, 'known': False, 'active': None, 'reason': 'dispatch_window_not_settled'}
    try:
        for process in pathlib.Path('/proc').iterdir():
            if not process.name.isdigit():
                continue
            try:
                if process.stat().st_uid != os.geteuid():
                    continue
                with (process / 'cmdline').open('rb') as stream:
                    raw = stream.read(65537)
                if len(raw) > 65536:
                    return {'sessionId': session_id, 'known': False, 'active': None, 'reason': 'process_identity_too_large'}
                argv = raw.split(b'\0')
                if any(argv[index] == b'--openkoda-session' and argv[index + 1] == session_id.encode('ascii')
                       for index in range(len(argv) - 1)):
                    return {'sessionId': session_id, 'known': True, 'active': True, 'reason': 'session_bootstrap_present'}
            except FileNotFoundError:
                continue
    except OSError:
        return {'sessionId': session_id, 'known': False, 'active': None, 'reason': 'process_inventory_unverifiable'}
    return {'sessionId': session_id, 'known': True, 'active': False, 'reason': 'expired_dispatch_and_no_session_process'}


def validate_children(children):
    """Validate every child before any early process/boot observation."""
    if not isinstance(children, list) or len(children) > 2:
        raise ProtocolError(-32001, 'Invalid child journal')
    for child in children:
        exact(child, ['state', 'pid', 'startTicks', 'launchDeadline'], ['state', 'pid', 'startTicks', 'launchDeadline'])
        if child['state'] not in ('launching', 'running', 'reaped', 'unknown') or type(child['launchDeadline']) is not int or not 0 < child['launchDeadline'] <= 9007199254740991:
            raise ProtocolError(-32001, 'Invalid child journal')
        pid = child['pid']
        if pid is None:
            if child['startTicks'] is not None or child['state'] not in ('launching', 'unknown'):
                raise ProtocolError(-32001, 'Invalid launch identity')
        elif type(pid) is not int or not 0 < pid <= 2147483647 or not isinstance(child['startTicks'], str) or not 1 <= len(child['startTicks']) <= 32 or not child['startTicks'].isascii() or not child['startTicks'].isdigit():
            raise ProtocolError(-32001, 'Invalid child identity')


def children_quiescent(children, session_id):
    """PID absence alone is insufficient: also inspect the child's session/group.

    A launch intent without PID never permits exec: the child waits for a token
    after the parent's durable PID record. Wait out its absolute deadline anyway.
    """
    validate_children(children)
    if not children:
        return True
    for child in children:
        if child['pid'] is None:
            if time.time() <= child['launchDeadline'] + 2:
                return False
            # An unrecorded child retains the exact bootstrap argv until exec;
            # it cannot receive the exec token without a recorded PID.
            probe = inspect_without_record(session_id, child['launchDeadline'])
            if probe.get('known') is not True or probe.get('active') is not False:
                return False
    groups = {child['pid'] for child in children if child['pid'] is not None}
    try:
        for process in pathlib.Path('/proc').iterdir():
            if not process.name.isdigit():
                continue
            try:
                fields = (process / 'stat').read_text().rsplit(') ', 1)[1].split()
                pid = int(process.name)
                if int(fields[2]) in groups or int(fields[3]) in groups:
                    return False
                for child in children:
                    if pid == child['pid'] and fields[19] == child['startTicks']:
                        return False
            except FileNotFoundError:
                continue
    except (OSError, IndexError, ValueError):
        return False
    return True


def validate_session_record(raw, environment_id, session_id, assessment_id=None):
    """One closed reader, shared by process inspection and metadata retention."""
    def unique_object(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError('Duplicate journal field')
            value[key] = item
        return value
    try:
        record = json.loads(raw, object_pairs_hook=unique_object,
                            parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
        exact(record, ['environmentId', 'sessionId', 'assessmentId', 'pid', 'startTicks', 'bootId', 'state', 'children', 'capture', 'retiredCapture'],
              ['environmentId', 'sessionId', 'assessmentId', 'pid', 'startTicks', 'bootId', 'state'])
        if record['environmentId'] != environment_id or record['sessionId'] != session_id or (assessment_id is not None and record['assessmentId'] != assessment_id):
            raise ValueError('Identity differs')
        for key in ('environmentId', 'sessionId', 'assessmentId', 'bootId'):
            identifier(record[key])
        if record['state'] not in ('running', 'closed', 'unknown'):
            raise ValueError('Invalid session state')
        pid = record['pid']
        if type(pid) is not int or not 0 < pid <= 2147483647:
            raise ValueError('Invalid process identity')
        if not isinstance(record['startTicks'], str) or not 1 <= len(record['startTicks']) <= 32 or not record['startTicks'].isascii() or not record['startTicks'].isdigit():
            raise ValueError('Invalid process start')
        if 'capture' in record:
            exact(record['capture'], ['captureId', 'attemptId'], ['captureId', 'attemptId'])
            for value in record['capture'].values():
                identifier(value)
        validate_children(record.get('children', []))
        if 'retiredCapture' in record:
            validate_retired_capture(record['retiredCapture'])
            proof = record['retiredCapture']['proof']
            if record.get('capture') != {key: proof[key] for key in ('captureId', 'attemptId')}:
                raise ValueError('Retired capture differs from the producer')
    except (ValueError, TypeError, KeyError, UnicodeError, RecursionError):
        raise ProtocolError(-32001, 'Session record is invalid') from None
    return record


def inspect_session(directory, environment_id, session_id, dispatch_deadline=0, assessment_id=None):
    identifier(session_id)
    parent = directory_fd(directory)
    try:
        private_fd(parent, True)
        try:
            raw = read_owned(parent, session_id + '.json', 4096)
        except FileNotFoundError:
            return inspect_without_record(session_id, dispatch_deadline)
    finally:
        os.close(parent)
    record = validate_session_record(raw, environment_id, session_id, assessment_id)
    boot = pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    try:
        identifier(boot)
    except ValueError:
        raise ProtocolError(-32001, 'Current boot identity is invalid') from None
    if boot != record['bootId']:
        return {'sessionId': session_id, 'known': True, 'active': False, 'reason': 'boot_changed'}
    if not children_quiescent(record.get('children', []), session_id):
        return {'sessionId': session_id, 'known': True, 'active': None, 'reason': 'child_cleanup_unconfirmed'}
    try:
        start = process_start(record['pid'])
    except FileNotFoundError:
        return {'sessionId': session_id, 'known': True, 'active': False, 'reason': 'process_absent'}
    except (OSError, IndexError):
        return {'sessionId': session_id, 'known': True, 'active': None, 'reason': 'process_unverifiable'}
    return {'sessionId': session_id, 'known': True, 'active': start == record['startTicks'],
            'reason': 'same_process' if start == record['startTicks'] else 'pid_reused'}


def binary_digest(fd):
    """Recheck the pinned descriptor before each child dispatch."""
    import errno
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022 or info.st_mode & 0o6000 or not info.st_mode & 0o111 or not 0 < info.st_size <= 32 * 1024 * 1024:
            raise ProtocolError(-32001, 'System ripgrep ownership or mode is invalid')
        try:
            if os.getxattr(fd, 'security.capability'):
                raise ProtocolError(-32001, 'Privileged parser capabilities are not permitted')
        except OSError as error:
            if error.errno != errno.ENODATA:
                raise
        os.lseek(fd, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        while True:
            block = os.read(fd, 65536)
            if not block:
                break
            digest.update(block)
        after = os.fstat(fd)
        if (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise ProtocolError(-32001, 'System parser changed during verification')
        return digest.hexdigest()
    except OSError:
        raise ProtocolError(-32001, 'System parser metadata could not be verified') from None


def open_ripgrep():
    """Pin the verified system executable by FD; no path from a request."""
    if sys.platform != 'linux':
        raise ProtocolError(-32001, 'The fixed parser requires Linux')
    fd = os.open('/usr/bin/rg', os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        return fd, binary_digest(fd)
    except BaseException:
        os.close(fd)
        raise


def child_exec(binary_fd, mode, input_fd, output_fd, gate_fd, parent_pid, deadline):
    """Runs after fork. No request data can select a program, regex or argument."""
    import ctypes
    import resource
    os.setsid()
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(1, signal.SIGKILL, 0, 0, 0) != 0 or os.getppid() != parent_pid:
        os._exit(121)
    signal.signal(signal.SIGALRM, signal.SIG_DFL)
    signal.alarm(CHILD_SECONDS + 1)
    resource.setrlimit(resource.RLIMIT_CPU, (2, 2))
    resource.setrlimit(resource.RLIMIT_AS, (128 * 1024 * 1024, 128 * 1024 * 1024))
    resource.setrlimit(resource.RLIMIT_FSIZE, (0, 0))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))
    os.chdir('/')
    os.dup2(input_fd, 0)
    os.dup2(output_fd, 1)
    null = os.open('/dev/null', os.O_WRONLY)
    os.dup2(null, 2)
    for raw_descriptor in os.listdir('/proc/self/fd'):
        descriptor = int(raw_descriptor)
        if descriptor > 2 and descriptor not in (binary_fd, gate_fd):
            try:
                os.close(descriptor)
            except OSError:
                pass
    # EOF (parent died before durable identity), expiry, or PID mismatch fails closed.
    if os.read(gate_fd, 1) != b'G' or time.time() > deadline or os.getppid() != parent_pid:
        os._exit(122)
    os.close(gate_fd)
    args = ['/usr/bin/rg', '--version'] if mode == 'version' else [
        '/usr/bin/rg', '--no-config', '--color', 'never', '--line-number', '--only-matching',
        '--no-heading', '--no-filename', '--text', '--threads', '1', '--regexp', SCAN_PATTERN, '--', '-']
    os.execve(binary_fd, args, {'LANG': 'C', 'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'})


def private_fd(fd, directory=False):
    value = os.fstat(fd)
    kind = stat.S_ISDIR if directory else stat.S_ISREG
    if not kind(value.st_mode) or value.st_uid != os.geteuid() or value.st_mode & 0o077 \
            or (not directory and value.st_nlink != 1):
        raise ProtocolError(-32001, 'Owned capture metadata is invalid')
    return value


def read_owned(fd, name, cap):
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=fd)
    try:
        before = private_fd(descriptor)
        if before.st_size > cap:
            raise ProtocolError(-32001, 'Owned capture exceeds its limit')
        raw = bytearray()
        while len(raw) <= cap:
            part = os.read(descriptor, min(4096, cap + 1 - len(raw)))
            if not part:
                break
            raw.extend(part)
        after = private_fd(descriptor)
        if len(raw) > cap or (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns):
            raise ProtocolError(-32001, 'Owned capture changed during reading')
        return bytes(raw)
    finally:
        os.close(descriptor)


def strict_json(raw):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            if key in out:
                raise ValueError('duplicate')
            out[key] = value
        return out
    try:
        value = json.loads(raw, object_pairs_hook=unique)
        if canonical(value) != raw:
            raise ValueError('noncanonical')
        return value
    except (ValueError, UnicodeError, RecursionError):
        raise ProtocolError(-32001, 'Owned capture JSON is invalid') from None


def atomic_owned(fd, name, value, cap=4096):
    raw = canonical(value)
    if len(raw) > cap:
        raise ProtocolError(-32001, 'Owned capture metadata exceeds its limit')
    temporary = name + '.next'
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=fd)
    try:
        private_fd(descriptor)
        written = 0
        while written < len(raw):
            count = os.write(descriptor, raw[written:])
            if count <= 0:
                raise OSError('capture write did not advance')
            written += count
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, name, src_dir_fd=fd, dst_dir_fd=fd)
    os.fsync(fd)


def directory_fd(path):
    path = pathlib.Path(path)
    if not path.is_absolute() or '..' in path.parts:
        raise ProtocolError(-32001, 'Owned directory must be absolute')
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for part in path.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def capture_root(sessions, create=False):
    parent = directory_fd(sessions)
    try:
        private_fd(parent, True)
        if create:
            try:
                os.mkdir('captures-v1', 0o700, dir_fd=parent)
                os.fsync(parent)
            except FileExistsError:
                pass
        root = os.open('captures-v1', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
        try:
            private_fd(root, True)
        except BaseException:
            os.close(root)
            raise
        return root
    finally:
        os.close(parent)


def open_capture(root, capture_id):
    identifier(capture_id)
    descriptor = os.open(capture_id, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=root)
    try:
        private_fd(descriptor, True)
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def sha_text(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def spool_manifest(fd):
    return validate_spool_manifest(strict_json(read_owned(fd, 'manifest.json', 4096)))


def validate_spool_manifest(value):
    exact(value, ['body', 'digest'], ['body', 'digest'])
    body = value['body']
    fields = ['schema', 'captureId', 'attemptId', 'assessmentId', 'environmentId', 'producerSessionId',
              'executorSourceSha256', 'catalogVersion', 'catalogSha256', 'parserVersion', 'inputEvidenceId',
              'inputSha256', 'stream', 'childRole', 'toolBinarySha256', 'producer', 'child', 'phase', 'generation',
              'maxBytes', 'preservedBytes', 'prefixSha256', 'observedBytes', 'reachedEof', 'lossReason', 'encoding', 'ownerProfileFingerprint']
    exact(body, fields, fields)
    for key in ['captureId', 'attemptId', 'assessmentId', 'environmentId', 'producerSessionId', 'inputEvidenceId']:
        identifier(body[key])
    for key in ['executorSourceSha256', 'catalogSha256', 'inputSha256', 'toolBinarySha256', 'prefixSha256', 'ownerProfileFingerprint']:
        if not sha_text(body[key]):
            raise ProtocolError(-32001, 'Capture digest is invalid')
    if body['schema'] != SPOOL_SCHEMA or body['catalogVersion'] not in (LEGACY_CATALOG_VERSION, CATALOG_VERSION) \
            or body['catalogSha256'] != catalog_sha256(body['catalogVersion']) \
            or body['parserVersion'] != ADAPTER + '/' + RULES_VERSION or body['stream'] != 'tool_stdout' or body['childRole'] != 'scan' \
            or body['phase'] not in ('open', 'captured', 'sealed', 'interrupted') or body['encoding'] not in ('utf8', 'unknown') \
            or body['lossReason'] not in (None, 'output_limit', 'disk_error', 'interrupted', 'parser_error', 'encoding_unknown') \
            or type(body['reachedEof']) is not bool or body['maxBytes'] != MAX_SCAN_OUTPUT:
        raise ProtocolError(-32001, 'Capture schema is incompatible')
    for key, minimum, maximum in [('generation', 1, 1000000), ('preservedBytes', 0, MAX_SCAN_OUTPUT)]:
        if type(body[key]) is not int or not minimum <= body[key] <= maximum:
            raise ProtocolError(-32001, 'Capture counter is invalid')
    if body['observedBytes'] is not None and (type(body['observedBytes']) is not int or not body['preservedBytes'] <= body['observedBytes'] <= MAX_SCAN_OUTPUT):
        raise ProtocolError(-32001, 'Capture size is invalid')
    for key in ('producer', 'child'):
        process = body[key]
        keys = ['pid', 'startTicks', 'bootId'] if key == 'producer' else ['pid', 'startTicks']
        exact(process, keys, keys)
        if process['pid'] is None and key == 'child' and process['startTicks'] is None:
            continue
        if type(process['pid']) is not int or process['pid'] <= 0 or not isinstance(process['startTicks'], str) or not process['startTicks'].isdigit():
            raise ProtocolError(-32001, 'Capture process identity is invalid')
        if key == 'producer':
            identifier(process['bootId'])
    if value['digest'] != hashlib.sha256(canonical(body)).hexdigest():
        raise ProtocolError(-32001, 'Capture manifest integrity failed')
    return value


def spool_quota(root):
    total = count = metadata = 0
    with os.scandir(root) as entries:
        for entry in entries:
            if entry.name == 'quota.lock':
                continue
            identifier(entry.name)
            count += 1
            if count >= SPOOL_COUNT:
                raise ProtocolError(-32001, 'Capture spool entry quota is exhausted')
            fd = open_capture(root, entry.name)
            try:
                raw_size = None
                with os.scandir(fd) as files:
                    for file in files:
                        if file.name not in ('stdout.bin', 'manifest.json', 'manifest.json.next', 'purge.json', 'purge.json.next'):
                            raise ProtocolError(-32001, 'Capture spool has an unknown entry')
                        info = file.stat(follow_symlinks=False)
                        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid() or info.st_mode & 0o077 or info.st_nlink != 1:
                            raise ProtocolError(-32001, 'Capture spool ownership is invalid')
                        if info.st_size > (MAX_SCAN_OUTPUT if file.name == 'stdout.bin' else 4096):
                            raise ProtocolError(-32001, 'Capture spool file exceeds its limit')
                        if file.name != 'stdout.bin':
                            metadata += info.st_size
                        else:
                            raw_size = info.st_size
                try:
                    manifest = spool_manifest(fd)
                except FileNotFoundError:
                    total += MAX_SCAN_OUTPUT
                    continue
                try:
                    purge = purge_record(fd, manifest)
                    if purge['state'] == 'purged':
                        if raw_size is not None:
                            raise ProtocolError(-32001, 'Purged capture still contains bytes')
                        continue
                except FileNotFoundError:
                    pass
                body = manifest['body']
                total += max(body['preservedBytes'], raw_size or 0) if body['phase'] in ('sealed', 'interrupted') else body['maxBytes']
            finally:
                os.close(fd)
    if total + MAX_SCAN_OUTPUT > SPOOL_LIMIT or metadata + 8192 > 2 * 1024 * 1024:
        raise ProtocolError(-32001, 'Capture spool byte quota is exhausted')


class SpoolWriter:
    def __init__(self, server, binary_hash):
        import fcntl
        binding = server.capture_binding
        keys = ['schema', 'captureId', 'attemptId', 'inputEvidenceId', 'inputSha256', 'maxBytes', 'ownerProfileFingerprint']
        exact(binding, keys, keys)
        for key in ['captureId', 'attemptId', 'inputEvidenceId']:
            identifier(binding[key])
        if binding['schema'] != 'openkoda.kali-spool-binding.v1' or binding['maxBytes'] != MAX_SCAN_OUTPUT or not sha_text(binding['inputSha256']) or not sha_text(binding['ownerProfileFingerprint']):
            raise ProtocolError(-32001, 'Capture binding is invalid')
        self.root = capture_root(server.sessions, True)
        self.fd = self.output = None
        lock = None
        try:
            lock = os.open('quota.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, 0o600, dir_fd=self.root)
            private_fd(lock)
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            spool_quota(self.root)
            os.mkdir(binding['captureId'], 0o700, dir_fd=self.root)
            os.fsync(self.root)
            self.fd = open_capture(self.root, binding['captureId'])
            self.output = os.open('stdout.bin', os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=self.fd)
            private_fd(self.output)
            os.fsync(self.output)
            self.body = {'schema': SPOOL_SCHEMA, 'captureId': binding['captureId'], 'attemptId': binding['attemptId'],
                'assessmentId': server.assessment_id, 'environmentId': server.environment_id, 'producerSessionId': server.session_id, 'ownerProfileFingerprint': binding['ownerProfileFingerprint'],
                'executorSourceSha256': server.runtime['executorSourceSha256'], 'catalogVersion': CATALOG_VERSION,
                'catalogSha256': catalog_sha256(), 'parserVersion': ADAPTER + '/' + RULES_VERSION,
                'inputEvidenceId': binding['inputEvidenceId'], 'inputSha256': binding['inputSha256'], 'stream': 'tool_stdout',
                'childRole': 'scan', 'toolBinarySha256': binary_hash, 'producer': {'pid': os.getpid(), 'startTicks': process_start(os.getpid()),
                'bootId': pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip()}, 'child': {'pid': None, 'startTicks': None},
                'phase': 'open', 'generation': 1, 'maxBytes': MAX_SCAN_OUTPUT, 'preservedBytes': 0,
                'prefixSha256': hashlib.sha256(b'').hexdigest(), 'observedBytes': None, 'reachedEof': False, 'lossReason': None, 'encoding': 'utf8'}
            self.commit(self.body)
            self.digest = hashlib.sha256()
        except BaseException:
            self.close()
            raise
        finally:
            if lock is not None:
                os.close(lock)

    def commit(self, body):
        value = {'body': body, 'digest': hashlib.sha256(canonical(body)).hexdigest()}
        atomic_owned(self.fd, 'manifest.json', value)
        self.body = body
        return value

    def update(self, **fields):
        if self.body['phase'] in ('sealed', 'interrupted'):
            raise ProtocolError(-32001, 'Capture is already immutable')
        return self.commit({**self.body, **fields, 'generation': self.body['generation'] + 1})

    def append(self, data):
        if self.body['phase'] != 'open' or self.body['preservedBytes'] + len(data) > MAX_SCAN_OUTPUT:
            raise ProtocolError(-32001, 'Capture prefix exceeds its reservation')
        offset = 0
        while offset < len(data):
            written = os.write(self.output, data[offset:])
            if written <= 0:
                raise OSError('capture append did not advance')
            offset += written
        os.fsync(self.output)
        digest = self.digest.copy()
        digest.update(data)
        self.update(preservedBytes=self.body['preservedBytes'] + len(data), prefixSha256=digest.hexdigest())
        self.digest = digest

    def captured(self, termination, eof):
        prefix = os.pread(self.output, self.body['preservedBytes'], 0)
        if len(prefix) != self.body['preservedBytes'] or hashlib.sha256(prefix).hexdigest() != self.body['prefixSha256']:
            raise ProtocolError(-32001, 'Capture prefix integrity failed')
        try:
            prefix.decode('utf-8')
            encoding = 'utf8'
        except UnicodeError:
            encoding = 'unknown'
        reason = {'output_limited': 'output_limit', 'canceled': 'interrupted', 'timed_out': 'interrupted', 'parser_error': 'parser_error'}.get(termination)
        self.update(phase='captured', reachedEof=eof, observedBytes=len(prefix) if eof else None, lossReason=reason, encoding=encoding)

    def seal(self, reason=None):
        loss = reason or self.body['lossReason'] or ('encoding_unknown' if self.body['encoding'] == 'unknown' else None)
        return self.update(phase='interrupted' if loss == 'interrupted' else 'sealed', lossReason=loss)

    def close(self):
        for name in ('output', 'fd', 'root'):
            value = getattr(self, name, None)
            if value is not None:
                os.close(value)
                setattr(self, name, None)


def purge_record(fd, manifest, name='purge.json'):
    value = strict_json(read_owned(fd, name, 4096))
    fields = ['schema', 'captureId', 'producerSessionId', 'environmentId', 'assessmentId',
              'manifestDigest', 'purgeReceiptId', 'preservedBytes', 'prefixSha256', 'state', 'ownerProfileFingerprint']
    version2 = value.get('schema') == 'openkoda.kali-spool-purge.v2'
    if version2:
        fields += ['previousReceiptId', 'previousReceiptDigest']
    exact(value, fields, fields)
    body = manifest['body']
    if value['schema'] not in ('openkoda.kali-spool-purge.v1', 'openkoda.kali-spool-purge.v2') or value['state'] not in ('intent', 'purged') \
            or value['manifestDigest'] != manifest['digest'] \
            or any(value[key] != body[key] for key in ('captureId', 'producerSessionId', 'environmentId', 'assessmentId', 'preservedBytes', 'prefixSha256', 'ownerProfileFingerprint')):
        raise ProtocolError(-32001, 'Purge receipt differs from the immutable capture')
    identifier(value['purgeReceiptId'])
    previous_id, previous_digest = value.get('previousReceiptId'), value.get('previousReceiptDigest')
    if (previous_id is None) != (previous_digest is None):
        raise ProtocolError(-32001, 'Invalid retention predecessor')
    if previous_id is not None:
        identifier(previous_id)
        if previous_id == value['purgeReceiptId'] or not sha_text(previous_digest):
            raise ProtocolError(-32001, 'Invalid retention predecessor')
    return value


def control_identity(server, manifest):
    body = manifest['body']
    if body['environmentId'] != server.environment_id or body['assessmentId'] != server.assessment_id \
            or body['captureId'] != server.control['captureId'] or body['producerSessionId'] != server.control['producerSessionId'] or body['ownerProfileFingerprint'] != server.control['ownerProfileFingerprint']:
        raise ProtocolError(-32001, 'Capture belongs to another bound producer')
    parent = directory_fd(server.sessions)
    try:
        private_fd(parent, True)
        raw = read_owned(parent, body['producerSessionId'] + '.json', 4096)
    finally:
        os.close(parent)
    try:
        record = validate_session_record(raw, server.environment_id, body['producerSessionId'], server.assessment_id)
        if 'retiredCapture' in record:
            raise ProtocolError(-32001, 'Capture metadata consolidation requires its private inspection')
        if record['assessmentId'] != body['assessmentId'] or record['capture'] != {'captureId': body['captureId'], 'attemptId': body['attemptId']} \
                or any(record[key] != body['producer'][key] for key in ('pid', 'startTicks', 'bootId')):
            raise ValueError('identity')
    except (ValueError, KeyError, TypeError, RecursionError, UnicodeError):
        raise ProtocolError(-32001, 'Capture producer journal differs') from None
    observation = inspect_session(server.sessions, server.environment_id, body['producerSessionId'])
    if observation.get('known') is not True or observation.get('active') is not False:
        raise ProtocolError(-32001, 'Capture producer cleanup is unconfirmed')


def capture_prefix(fd, manifest):
    body = manifest['body']
    raw = read_owned(fd, 'stdout.bin', MAX_SCAN_OUTPUT)
    # A crash can leave an unacknowledged tail. It is never imported or exposed.
    prefix = raw[:body['preservedBytes']]
    if len(prefix) != body['preservedBytes'] or hashlib.sha256(prefix).hexdigest() != body['prefixSha256']:
        raise ProtocolError(-32001, 'Capture prefix integrity failed')
    return prefix


def check_capture_entries(fd):
    with os.scandir(fd) as entries:
        for entry in entries:
            if entry.name not in ('stdout.bin', 'manifest.json', 'manifest.json.next', 'purge.json', 'purge.json.next'):
                raise ProtocolError(-32001, 'Capture contains an unknown entry')
            value = entry.stat(follow_symlinks=False)
            if not stat.S_ISREG(value.st_mode) or value.st_uid != os.geteuid() or value.st_mode & 0o077 or value.st_nlink != 1 \
                    or value.st_size > (MAX_SCAN_OUTPUT if entry.name == 'stdout.bin' else 4096):
                raise ProtocolError(-32001, 'Capture entry ownership or limit changed')


def validate_retention_prefix(fd, manifest, receipt):
    if receipt is None:
        capture_prefix(fd, manifest)
        return
    try:
        if receipt['state'] == 'purged':
            read_owned(fd, 'stdout.bin', MAX_SCAN_OUTPUT)
            raise ProtocolError(-32001, 'Purged capture bytes unexpectedly reappeared')
        capture_prefix(fd, manifest)
    except FileNotFoundError:
        # An intent can survive the unlink and lose the final receipt commit.
        # Missing raw is valid here; existing raw must still match the prefix.
        pass


def describe_absent(server):
    control = server.control
    parent = directory_fd(server.sessions)
    try:
        private_fd(parent, True)
        try:
            raw = read_owned(parent, control['producerSessionId'] + '.json', 4096)
        except FileNotFoundError:
            raw = None
        if raw is not None:
            record = validate_session_record(raw, server.environment_id, control['producerSessionId'], server.assessment_id)
            if 'retiredCapture' in record:
                raise ProtocolError(-32001, 'Retired capture requires its private inspection, not generic absence')
    finally:
        os.close(parent)
    observed = inspect_session(server.sessions, server.environment_id, control['producerSessionId'],
                               control['producerDispatchDeadline'], server.assessment_id)
    if observed.get('known') is not True or observed.get('active') is not False:
        raise ProtocolError(-32001, 'Missing spool has no quiescent producer proof')
    body = {'schema': 'openkoda.kali-spool-absence.v1', 'assessmentId': server.assessment_id,
            'environmentId': server.environment_id, 'captureId': control['captureId'],
            'producerSessionId': control['producerSessionId'], 'ownerProfileFingerprint': control['ownerProfileFingerprint'],
            'producerDispatchDeadline': control['producerDispatchDeadline'], 'directoryAbsent': True}
    return {**server.metadata(), 'operation': 'capture_collect', 'kind': 'describe', 'availability': 'absent',
            'manifest': None, 'absence': {'body': body, 'digest': hashlib.sha256(canonical(body)).hexdigest()},
            'producerQuiescent': True, 'purgeReceipt': None}


def capture_control(server, name, args):
    if server.sessions is None:
        raise ProtocolError(-32001, 'Capture journal is unavailable')
    keys = ['producerSessionId', 'captureId']
    if name == 'capture_collect':
        if not isinstance(args, dict) or args.get('kind') not in ('describe', 'read'):
            raise ProtocolError(-32602, 'Unknown capture read operation')
        keys += ['kind'] + (['expectedManifestDigest', 'offset', 'maxBytes'] if args['kind'] == 'read' else [])
    else:
        keys += ['expectedManifestDigest', 'purgeReceiptId', 'previousReceiptId', 'previousReceiptDigest']
    exact(args, keys, keys)
    if any(args[key] != server.control[key] for key in server.control if key in args):
        raise ProtocolError(-32602, 'Capture control binding differs')
    try:
        root = capture_root(server.sessions)
    except FileNotFoundError:
        if name == 'capture_collect' and args['kind'] == 'describe':
            return describe_absent(server)
        raise ProtocolError(-32001, 'Known spool directory is absent') from None
    fd = None
    try:
        try:
            fd = open_capture(root, args['captureId'])
        except FileNotFoundError:
            if name == 'capture_collect' and args['kind'] == 'describe':
                return describe_absent(server)
            raise ProtocolError(-32001, 'Known capture directory is absent') from None
        check_capture_entries(fd)
        manifest = spool_manifest(fd)
        control_identity(server, manifest)
        if 'expectedManifestDigest' in args and args['expectedManifestDigest'] != manifest['digest']:
            raise ProtocolError(-32602, 'Capture manifest changed')
        try:
            receipt = purge_record(fd, manifest)
        except FileNotFoundError:
            receipt = None
        validate_retention_prefix(fd, manifest, receipt)
        if name == 'capture_collect':
            if args['kind'] == 'describe':
                return {**server.metadata(), 'operation': name, 'kind': 'describe', 'availability': 'present', 'absence': None,
                        'manifest': manifest, 'producerQuiescent': True, 'purgeReceipt': receipt}
            if receipt is not None:
                raise ProtocolError(-32001, 'Capture retention has already started')
            prefix = capture_prefix(fd, manifest)
            if type(args['offset']) is not int or not 0 <= args['offset'] <= len(prefix) \
                    or type(args['maxBytes']) is not int or not 1 <= args['maxBytes'] <= COLLECT_PAGE:
                raise ProtocolError(-32602, 'Capture range is invalid')
            page = prefix[args['offset']:args['offset'] + args['maxBytes']]
            return {**server.metadata(), 'operation': name, 'kind': 'read', 'manifestDigest': manifest['digest'],
                    'offset': args['offset'], 'bytes': len(page), 'base64': base64.b64encode(page).decode('ascii'),
                    'eof': args['offset'] + len(page) == len(prefix)}
        identifier(args['purgeReceiptId'])
        previous_id, previous_digest = args['previousReceiptId'], args['previousReceiptDigest']
        if (previous_id is None) != (previous_digest is None):
            raise ProtocolError(-32602, 'Retention predecessor must be exact')
        if previous_id is not None:
            identifier(previous_id)
            if previous_id == args['purgeReceiptId'] or not sha_text(previous_digest):
                raise ProtocolError(-32602, 'Retention predecessor is invalid')
        body = manifest['body']
        if receipt is None:
            if previous_id is not None:
                raise ProtocolError(-32602, 'Expected retention predecessor is absent')
            capture_prefix(fd, manifest)
        elif receipt['purgeReceiptId'] == args['purgeReceiptId']:
            if receipt.get('previousReceiptId') != previous_id or receipt.get('previousReceiptDigest') != previous_digest:
                raise ProtocolError(-32602, 'Repeated retention binding changed')
            # Replay observes the receipt. It never resumes a partial deletion.
            return {**server.metadata(), 'operation': name, 'receipt': receipt}
        elif receipt['state'] != 'intent' or receipt['purgeReceiptId'] != previous_id or hashlib.sha256(canonical(receipt)).hexdigest() != previous_digest:
            raise ProtocolError(-32602, 'Partial retention predecessor changed')
        receipt = {'schema': 'openkoda.kali-spool-purge.v2', 'manifestDigest': manifest['digest'],
                   'purgeReceiptId': args['purgeReceiptId'], 'state': 'intent', 'previousReceiptId': previous_id, 'previousReceiptDigest': previous_digest,
                   **{key: body[key] for key in ('captureId', 'producerSessionId', 'environmentId', 'assessmentId', 'preservedBytes', 'prefixSha256', 'ownerProfileFingerprint')}}
        # Recover only a known receipt temporary, never an arbitrary entry.
        try:
            pending = purge_record(fd, manifest, 'purge.json.next')
            # Only bounded metadata of this exact capture may be discarded;
            # its process family was proved stopped before the new host intent.
            os.unlink('purge.json.next', dir_fd=fd)
            os.fsync(fd)
        except FileNotFoundError:
            pass
        if receipt['state'] != 'purged':
            atomic_owned(fd, 'purge.json', receipt)
            for filename in ('stdout.bin', 'manifest.json.next'):
                try:
                    if filename == 'stdout.bin':
                        capture_prefix(fd, manifest)
                    else:
                        read_owned(fd, filename, 4096)
                    os.unlink(filename, dir_fd=fd)
                except FileNotFoundError:
                    pass
            os.fsync(fd)
            receipt = {**receipt, 'state': 'purged'}
            atomic_owned(fd, 'purge.json', receipt)
        return {**server.metadata(), 'operation': name, 'receipt': receipt}
    finally:
        if fd is not None:
            os.close(fd)
        os.close(root)


def paired_receipt(identity, digest, forbidden=()):
    if (identity is None) != (digest is None):
        raise ProtocolError(-32602, 'Receipt predecessor must be an exact pair')
    if identity is not None:
        identifier(identity)
        if identity in forbidden or not sha_text(digest):
            raise ProtocolError(-32602, 'Receipt predecessor is invalid')


def validate_retired_capture(value):
    exact(value, ['schema', 'proof', 'attempt'], ['schema', 'proof', 'attempt'])
    if value['schema'] != 'openkoda.kali-retired-capture.v1' or len(canonical(value)) > 1024:
        raise ProtocolError(-32001, 'Retired capture commitment exceeds its contract')
    proof, attempt = value['proof'], value['attempt']
    proof_keys = ['captureId', 'attemptId', 'ownerProfileFingerprint', 'manifestDigest', 'purgeReceiptId', 'purgeReceiptSha256']
    exact(proof, proof_keys, proof_keys)
    for key in ('captureId', 'attemptId', 'purgeReceiptId'):
        identifier(proof[key])
    for key in ('ownerProfileFingerprint', 'manifestDigest', 'purgeReceiptSha256'):
        if not sha_text(proof[key]):
            raise ProtocolError(-32001, 'Retired capture proof is invalid')
    attempt_keys = ['retireReceiptId', 'previousReceiptId', 'previousReceiptDigest', 'reviewedStateDigest']
    exact(attempt, attempt_keys, attempt_keys)
    identifier(attempt['retireReceiptId'])
    if attempt['retireReceiptId'] == proof['purgeReceiptId'] or not sha_text(attempt['reviewedStateDigest']):
        raise ProtocolError(-32001, 'Retired capture attempt is invalid')
    paired_receipt(attempt['previousReceiptId'], attempt['previousReceiptDigest'],
                   (attempt['retireReceiptId'], proof['purgeReceiptId']))
    return value


def validate_retire_control(control):
    common = ['kind', 'producerSessionId', 'captureId', 'ownerProfileFingerprint', 'producerDispatchDeadline',
              'expectedManifestDigest', 'purgeReceiptId', 'expectedPurgeReceiptSha256', 'pendingAttempt']
    if not isinstance(control, dict) or control.get('kind') not in ('inspect', 'apply'):
        raise ProtocolError(-32602, 'Retire control kind is invalid')
    keys = common + (['retireReceiptId', 'previousReceiptId', 'previousReceiptDigest', 'expectedStateDigest']
                     if control['kind'] == 'apply' else [])
    exact(control, keys, keys)
    for key in ('producerSessionId', 'captureId', 'purgeReceiptId'):
        identifier(control[key])
    for key in ('ownerProfileFingerprint', 'expectedManifestDigest'):
        if not sha_text(control[key]):
            raise ProtocolError(-32602, 'Retire identity digest is invalid')
    if type(control['producerDispatchDeadline']) is not int or not 1 <= control['producerDispatchDeadline'] <= 9007199254740991:
        raise ProtocolError(-32602, 'Retire producer deadline is invalid')
    expected = control['expectedPurgeReceiptSha256']
    if (expected is not None and not sha_text(expected)) or (control['kind'] == 'apply' and expected is None):
        raise ProtocolError(-32602, 'Retire requires the exact purge proof')
    if control['kind'] == 'apply':
        identifier(control['retireReceiptId'])
        if control['retireReceiptId'] == control['purgeReceiptId'] or not sha_text(control['expectedStateDigest']):
            raise ProtocolError(-32602, 'Retire attempt binding is invalid')
        paired_receipt(control['previousReceiptId'], control['previousReceiptDigest'],
                       (control['retireReceiptId'], control['purgeReceiptId']))
    pending = control['pendingAttempt']
    if pending is not None:
        fields = ['retireReceiptId', 'previousReceiptId', 'previousReceiptDigest', 'reviewedStateDigest', 'producerRecordSha256']
        exact(pending, fields, fields)
        identifier(pending['retireReceiptId'])
        if pending['retireReceiptId'] == control['purgeReceiptId'] or not sha_text(pending['reviewedStateDigest']) or not sha_text(pending['producerRecordSha256']):
            raise ProtocolError(-32602, 'Interrupted producer write is not bound')
        paired_receipt(pending['previousReceiptId'], pending['previousReceiptDigest'],
                       (pending['retireReceiptId'], control['purgeReceiptId']))
    return control


def file_identity(info):
    return {'device': str(info.st_dev), 'inode': str(info.st_ino)}


def metadata_file(parent, name, cap=4096):
    """Read one fixed name, checking both its descriptor and directory entry."""
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent)
    try:
        before = private_fd(descriptor)
        if before.st_size > cap:
            raise ProtocolError(-32001, 'Metadata file exceeds its bound')
        raw = bytearray()
        while len(raw) <= cap:
            block = os.read(descriptor, min(4096, cap + 1 - len(raw)))
            if not block:
                break
            raw.extend(block)
        after = private_fd(descriptor)
        named = os.stat(name, dir_fd=parent, follow_symlinks=False)
        def stable(info):
            return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink, info.st_size, info.st_mtime_ns)
        if len(raw) > cap or stable(before) != stable(after) or stable(after) != stable(named):
            raise ProtocolError(-32001, 'Metadata entry changed during observation')
        data = bytes(raw)
        return data, {'name': name, 'identity': file_identity(after), 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
    finally:
        os.close(descriptor)


def validate_terminal_purge(value, server, record, expected_hash):
    fields = ['schema', 'captureId', 'producerSessionId', 'environmentId', 'assessmentId', 'manifestDigest',
              'purgeReceiptId', 'preservedBytes', 'prefixSha256', 'state', 'ownerProfileFingerprint']
    if value.get('schema') == 'openkoda.kali-spool-purge.v2':
        fields += ['previousReceiptId', 'previousReceiptDigest']
    exact(value, fields, fields)
    control = server.control
    expected = {'environmentId': server.environment_id, 'assessmentId': server.assessment_id,
                'producerSessionId': control['producerSessionId'], 'captureId': control['captureId'],
                'ownerProfileFingerprint': control['ownerProfileFingerprint'], 'manifestDigest': control['expectedManifestDigest'],
                'purgeReceiptId': control['purgeReceiptId']}
    if value['schema'] not in ('openkoda.kali-spool-purge.v1', 'openkoda.kali-spool-purge.v2') or value['state'] != 'purged' \
            or any(value[key] != item for key, item in expected.items()) or not sha_text(value['prefixSha256']) \
            or type(value['preservedBytes']) is not int or not 0 <= value['preservedBytes'] <= MAX_SCAN_OUTPUT:
        raise ProtocolError(-32001, 'Terminal purge proof does not match the known capture')
    paired_receipt(value.get('previousReceiptId'), value.get('previousReceiptDigest'), (value['purgeReceiptId'],))
    digest = hashlib.sha256(canonical(value)).hexdigest()
    if expected_hash is not None and expected_hash != digest:
        raise ProtocolError(-32001, 'Terminal purge proof changed')
    return {'schema': 'openkoda.kali-purge-proof.v1', 'receipt': value, 'sha256': digest}


def retirement_proof(server, record, purge_hash):
    capture = record.get('capture')
    if capture is None or capture['captureId'] != server.control['captureId']:
        raise ProtocolError(-32001, 'Producer does not own this capture')
    return {'captureId': capture['captureId'], 'attemptId': capture['attemptId'],
            'ownerProfileFingerprint': server.control['ownerProfileFingerprint'],
            'manifestDigest': server.control['expectedManifestDigest'], 'purgeReceiptId': server.control['purgeReceiptId'],
            'purgeReceiptSha256': purge_hash}


def retirement_commitment(proof, attempt):
    result = {'schema': 'openkoda.kali-retired-capture.v1', 'proof': proof,
              'attempt': {key: attempt[key] for key in ('retireReceiptId', 'previousReceiptId', 'previousReceiptDigest', 'reviewedStateDigest')}}
    return validate_retired_capture(result)


@contextlib.contextmanager
def retirement_handles(server):
    """Lock order: sessions -> capture root -> existing quota.lock -> producer -> capture.

    Even inspection only opens existing names. It creates no lock or directory.
    """
    import fcntl
    parent = root = lock = None
    try:
        parent = directory_fd(server.sessions)
        private_fd(parent, True)
        root = capture_root(server.sessions)
        lock = os.open('quota.lock', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=root)
        private_fd(lock)
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield parent, root
    finally:
        for descriptor in (lock, root, parent):
            if descriptor is not None:
                os.close(descriptor)


def named_directory(root, capture_id, expected):
    named = os.stat(capture_id, dir_fd=root, follow_symlinks=False)
    if not stat.S_ISDIR(named.st_mode) or named.st_uid != os.geteuid() or named.st_mode & 0o077 \
            or file_identity(named) != expected:
        raise ProtocolError(-32001, 'Capture directory was replaced')


def confirm_retired_durability(parent, root, producer_name, journal_entry, capture_id):
    """Sync only existing verified objects; never create or advance an intention."""
    descriptor = os.open(producer_name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent)
    try:
        if file_identity(private_fd(descriptor)) != journal_entry['identity']:
            raise ProtocolError(-32001, 'Retired producer journal was replaced')
        exact_metadata_entry(parent, journal_entry)
        os.fsync(descriptor)
        os.fsync(parent)
        os.fsync(root)
        exact_metadata_entry(parent, journal_entry)
        try:
            os.stat(capture_id, dir_fd=root, follow_symlinks=False)
        except FileNotFoundError:
            return
        raise ProtocolError(-32001, 'Retired capture directory reappeared')
    finally:
        os.close(descriptor)


def observe_retirement(server, parent, root):
    control = server.control
    producer_name = control['producerSessionId'] + '.json'
    raw, journal_entry = metadata_file(parent, producer_name)
    record = validate_session_record(raw, server.environment_id, control['producerSessionId'], server.assessment_id)
    record_hash = hashlib.sha256(canonical(record)).hexdigest()
    commitment = record.get('retiredCapture')
    quiescence = inspect_session(server.sessions, server.environment_id, control['producerSessionId'],
                                 control['producerDispatchDeadline'], server.assessment_id)
    if quiescence.get('known') is not True or quiescence.get('active') is not False:
        raise ProtocolError(-32001, 'Capture producer cleanup is unconfirmed')
    fd = None
    entries, values = [], {}
    directory = None
    try:
        try:
            fd = open_capture(root, control['captureId'])
        except FileNotFoundError:
            if commitment is None:
                raise ProtocolError(-32001, 'Missing metadata has no retirement commitment') from None
        if fd is not None:
            directory = file_identity(private_fd(fd, True))
            named_directory(root, control['captureId'], directory)
            names = sorted(os.listdir(fd))
            if len(names) > 4 or any(name not in ('manifest.json', 'purge.json', 'manifest.json.next', 'purge.json.next') for name in names):
                raise ProtocolError(-32001, 'Terminal capture contains raw bytes or unknown entries')
            for name in names:
                data, entry = metadata_file(fd, name)
                entries.append(entry)
                values[name] = strict_json(data)
        expected_hash = control['expectedPurgeReceiptSha256']
        if commitment is not None:
            if expected_hash is None or commitment['proof'] != retirement_proof(server, record, expected_hash):
                raise ProtocolError(-32001, 'Retirement commitment differs from the durable host proof')
        purge_proof = None
        for name in ('purge.json', 'purge.json.next'):
            if name in values:
                proof = validate_terminal_purge(values[name], server, record, expected_hash)
                if purge_proof is not None and purge_proof != proof:
                    raise ProtocolError(-32001, 'Terminal purge temporary differs')
                purge_proof, expected_hash = proof, proof['sha256']
        if commitment is None and (purge_proof is None or 'purge.json' not in values or 'manifest.json' not in values):
            raise ProtocolError(-32001, 'Terminal capture lacks its original purge and manifest')
        proof = retirement_proof(server, record, expected_hash)
        for name in ('manifest.json', 'manifest.json.next'):
            if name in values:
                # Reuse the exact historical schema checks without substituting current catalog/source identity.
                manifest = values[name]
                with_manifest = validate_spool_manifest(manifest)
                body = with_manifest['body']
                if with_manifest['digest'] != proof['manifestDigest'] or any(body[key] != proof[key] for key in ('captureId', 'attemptId', 'ownerProfileFingerprint')) \
                        or body['environmentId'] != server.environment_id or body['assessmentId'] != server.assessment_id \
                        or body['producerSessionId'] != control['producerSessionId'] \
                        or any(record[key] != body['producer'][key] for key in ('pid', 'startTicks', 'bootId')):
                    raise ProtocolError(-32001, 'Terminal manifest changed identity')
                if purge_proof is not None and any(body[key] != purge_proof['receipt'][key] for key in ('preservedBytes', 'prefixSha256')):
                    raise ProtocolError(-32001, 'Terminal manifest and purge disagree')
        pending_observation = pending_entry = None
        pending_name = producer_name + '.next'
        try:
            pending_raw, pending_entry = metadata_file(parent, pending_name)
        except FileNotFoundError:
            pass
        else:
            pending = control['pendingAttempt']
            if pending is None or pending['producerRecordSha256'] != record_hash:
                raise ProtocolError(-32001, 'Producer temporary has no exact durable host intent')
            candidate_commitment = retirement_commitment(proof, pending)
            if commitment is not None:
                previous = candidate_commitment['attempt']
                if previous['previousReceiptId'] != commitment['attempt']['retireReceiptId'] \
                        or previous['previousReceiptDigest'] != hashlib.sha256(canonical(commitment)).hexdigest():
                    raise ProtocolError(-32001, 'Producer temporary does not supersede the known commitment')
            elif pending['previousReceiptId'] is not None:
                raise ProtocolError(-32001, 'Producer temporary predecessor is absent')
            candidate = canonical({**record, 'retiredCapture': candidate_commitment})
            if len(candidate) > 4096 or len(pending_raw) > len(candidate) or not candidate.startswith(pending_raw):
                raise ProtocolError(-32001, 'Producer temporary is not the exact interrupted write')
            pending_observation = {key: pending_entry[key] for key in ('identity', 'bytes', 'sha256')}
            pending_observation.update(attemptId=pending['retireReceiptId'], complete=len(pending_raw) == len(candidate))
        observation = {'schema': 'openkoda.kali-retire-observation.v1',
            'environmentId': server.environment_id, 'assessmentId': server.assessment_id,
            'producerSessionId': control['producerSessionId'], **proof,
            'producerRecordSha256': record_hash, 'directory': directory, 'entries': entries,
            'pendingProducerWrite': pending_observation, 'commitment': commitment,
            'state': 'retired' if directory is None else ('metadata_remaining' if commitment is not None else 'ready'),
            'metadataBytes': sum(item['bytes'] for item in entries) + (pending_entry['bytes'] if pending_entry is not None else 0),
            'slotPresent': directory is not None}
        if directory is None:
            if pending_entry is not None:
                raise ProtocolError(-32001, 'Retired capture has an unresolved producer write')
            confirm_retired_durability(parent, root, producer_name, journal_entry, control['captureId'])
        return {'observation': observation, 'stateDigest': hashlib.sha256(canonical(observation)).hexdigest(),
                'linuxPurgeProof': purge_proof, 'record': record, 'journalEntry': journal_entry,
                'pendingEntry': pending_entry}
    finally:
        if fd is not None:
            os.close(fd)


def retirement_response(server, snapshot):
    return {**server.metadata(), 'operation': 'capture_retire', 'kind': server.control['kind'],
            'producerQuiescent': True,
            **{key: snapshot[key] for key in ('observation', 'stateDigest', 'linuxPurgeProof')}}


def exact_metadata_entry(parent, expected):
    _, current = metadata_file(parent, expected['name'])
    if current != expected:
        raise ProtocolError(-32001, 'Reviewed metadata entry was replaced or changed')


def exact_remaining_metadata(parent, expected):
    names = [entry['name'] for entry in expected]
    if sorted(os.listdir(parent)) != names:
        raise ProtocolError(-32001, 'Reviewed capture entries changed')
    for entry in expected:
        exact_metadata_entry(parent, entry)
    if sorted(os.listdir(parent)) != names:
        raise ProtocolError(-32001, 'Reviewed capture entries changed')


def retire_control(server, arguments):
    validate_retire_control(arguments)
    if arguments != server.control or server.sessions is None:
        raise ProtocolError(-32602, 'Retire arguments differ from the private bootstrap')
    with retirement_handles(server) as (parent, root):
        before = observe_retirement(server, parent, root)
        if arguments['kind'] == 'inspect':
            return retirement_response(server, before)
        observation, record = before['observation'], before['record']
        commitment = observation['commitment']
        attempt = {'retireReceiptId': arguments['retireReceiptId'], 'previousReceiptId': arguments['previousReceiptId'],
                   'previousReceiptDigest': arguments['previousReceiptDigest'], 'reviewedStateDigest': arguments['expectedStateDigest']}
        # Replay is validated against its original binding before comparing the mutable state digest.
        if commitment is not None and commitment['attempt']['retireReceiptId'] == arguments['retireReceiptId']:
            if commitment['attempt'] != attempt:
                raise ProtocolError(-32602, 'Repeated retirement attempt changed')
            return retirement_response(server, before)
        pending = arguments['pendingAttempt']
        if pending is not None and pending['retireReceiptId'] == arguments['retireReceiptId']:
            if any(pending[key] != value for key, value in attempt.items()):
                raise ProtocolError(-32602, 'Repeated interrupted retirement changed')
            return retirement_response(server, before)
        if observation['state'] == 'retired' or before['stateDigest'] != arguments['expectedStateDigest']:
            raise ProtocolError(-32602, 'Reviewed retirement state changed')
        if commitment is None:
            if arguments['previousReceiptId'] is not None:
                raise ProtocolError(-32602, 'Retirement predecessor is absent')
        elif arguments['previousReceiptId'] != commitment['attempt']['retireReceiptId'] \
                or arguments['previousReceiptDigest'] != hashlib.sha256(canonical(commitment)).hexdigest():
            raise ProtocolError(-32602, 'Retirement predecessor changed')
        proof = retirement_proof(server, record, arguments['expectedPurgeReceiptSha256'])
        next_commitment = retirement_commitment(proof, attempt)
        next_record = {**record, 'retiredCapture': next_commitment}
        if len(canonical(next_record)) > 4096:
            raise ProtocolError(-32001, 'Retired producer journal exceeds its limit')
        producer_name = arguments['producerSessionId'] + '.json'
        exact_metadata_entry(parent, before['journalEntry'])
        if before['pendingEntry'] is not None:
            exact_metadata_entry(parent, before['pendingEntry'])
            os.unlink(producer_name + '.next', dir_fd=parent)
            os.fsync(parent)
        atomic_owned(parent, producer_name, next_record)
        committed_raw, committed_entry = metadata_file(parent, producer_name)
        if committed_raw != canonical(next_record):
            raise ProtocolError(-32001, 'Retirement commitment did not persist exactly')
        fd = open_capture(root, arguments['captureId'])
        try:
            named_directory(root, arguments['captureId'], observation['directory'])
            if file_identity(private_fd(fd, True)) != observation['directory']:
                raise ProtocolError(-32001, 'Reviewed capture directory changed')
            for index, entry in enumerate(observation['entries']):
                named_directory(root, arguments['captureId'], observation['directory'])
                exact_metadata_entry(parent, committed_entry)
                exact_remaining_metadata(fd, observation['entries'][index:])
                os.unlink(entry['name'], dir_fd=fd)
            os.fsync(fd)
            if os.listdir(fd):
                raise ProtocolError(-32001, 'Capture gained an unexpected entry')
            named_directory(root, arguments['captureId'], observation['directory'])
            exact_metadata_entry(parent, committed_entry)
            os.rmdir(arguments['captureId'], dir_fd=root)
            os.fsync(root)
        finally:
            os.close(fd)
        return retirement_response(server, observe_retirement(server, parent, root))


def collect_child(pid, input_fd, output_fd, raw, cancel_fd, deadline, cap, spool=None):
    """Bounded nonblocking duplex IO. Never logs stdout or stderr."""
    os.set_blocking(input_fd, False)
    os.set_blocking(output_fd, False)
    output = bytearray()
    offset = 0
    sent = False
    termination = 'completed'
    cancel = select.poll()
    if cancel_fd is not None:
        cancel.register(cancel_fd, select.POLLHUP | select.POLLERR)
    while True:
        if cancel.poll(0):
            termination = 'canceled'
            break
        if time.monotonic() >= deadline:
            termination = 'timed_out'
            break
        if not sent:
            try:
                if offset < len(raw):
                    offset += os.write(input_fd, raw[offset:offset + 4096])
                if offset == len(raw):
                    os.close(input_fd)
                    sent = True
            except BlockingIOError:
                pass
            except BrokenPipeError:
                os.close(input_fd)
                sent = True
        try:
            data = os.read(output_fd, 4096)
            if not data:
                if spool is not None:
                    spool.reached_eof = True
                break
            available = cap - len(output)
            if spool is not None:
                spool.append(data[:available])
            output.extend(data[:available])
            if len(data) > available:
                termination = 'output_limited'
                break
        except BlockingIOError:
            time.sleep(0.01)
    return bytes(output), termination, sent


def stop_and_reap(pid, deadline):
    # The child is not reaped until after kill, so its PID/group cannot be recycled.
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    # It may not have reached setsid yet; kill the still-unreaped PID as well.
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    while time.monotonic() < deadline:
        observed, status = os.waitpid(pid, os.WNOHANG)
        if observed == pid:
            return status
        time.sleep(0.01)
    raise RuntimeError('Child reaping was not confirmed')


def run_fixed(server, binary_fd, mode, raw, cancel_fd=None):
    if server.mode != 'execute':
        raise ProtocolError(-32001, 'Control sessions cannot execute producers')
    if mode not in ('version', 'scan') or len(raw) > MAX_BYTES or len(server.children) >= 2:
        raise ProtocolError(-32602, 'Invalid fixed parser invocation')
    child = {'state': 'launching', 'pid': None, 'startTicks': None,
             'launchDeadline': int(time.time()) + CHILD_SECONDS}
    server.children.append(child)
    server.journal()  # durable intent BEFORE fork
    pipes = [os.pipe() for _ in range(3)]
    input_read, input_write = pipes[0]
    output_read, output_write = pipes[1]
    gate_read, gate_write = pipes[2]
    descriptors = {fd for pair in pipes for fd in pair}
    pid = None
    parent_pid = os.getpid()
    cleaned = False
    reaped = False
    try:
        pid = os.fork()
        if pid == 0:
            try:
                child_exec(binary_fd, mode, input_read, output_write, gate_read, parent_pid, child['launchDeadline'])
            except BaseException:
                os._exit(123)
            os._exit(124)
        for fd in (input_read, output_write, gate_read):
            os.close(fd)
            descriptors.remove(fd)
        child.update(pid=pid, startTicks=process_start(pid), state='running')
        server.journal()  # no exec token before durable PID/start identity
        if mode == 'scan' and server.spool is not None:
            server.spool.update(child={'pid': pid, 'startTicks': child['startTicks']})
        os.write(gate_write, b'G')
        os.close(gate_write)
        descriptors.remove(gate_write)
        output, termination, input_closed = collect_child(pid, input_write, output_read, raw, cancel_fd,
            time.monotonic() + CHILD_SECONDS, 4096 if mode == 'version' else MAX_SCAN_OUTPUT,
            server.spool if mode == 'scan' else None)
        if input_closed:
            descriptors.remove(input_write)
        # EOF is not proof that a process exited. Reap normally, then kill/wait if needed.
        observed, status = os.waitpid(pid, os.WNOHANG)
        if observed != pid:
            if termination == 'completed':
                # rg may close stdout just before exit; allow a bounded graceful exit.
                until = time.monotonic() + 0.2
                while observed != pid and time.monotonic() < until:
                    time.sleep(0.01)
                    observed, status = os.waitpid(pid, os.WNOHANG)
            if observed != pid:
                status = stop_and_reap(pid, time.monotonic() + 1)
        reaped = True
        cleaned = True
        if not children_quiescent([child], server.session_id):
            cleaned = False
            raise RuntimeError('Parser process group remains unverified')
        child['state'] = 'reaped'
        server.journal()
        exit_code = os.waitstatus_to_exitcode(status)
        if termination == 'completed' and exit_code not in ((0,) if mode == 'version' else (0, 1)):
            termination = 'parser_error'
        return output, termination
    finally:
        for fd in descriptors:
            try:
                os.close(fd)
            except OSError:
                pass
        if pid and not reaped:
            try:
                stop_and_reap(pid, time.monotonic() + 1)
                cleaned = children_quiescent([child], server.session_id)
            except (OSError, RuntimeError):
                cleaned = False
        if not cleaned:
            server.cleanup_uncertain = True
            child['state'] = 'unknown'
            server.journal()


def summarize_signals(output, termination):
    candidates = []
    counts = {rule: 0 for rule in RULE_LABELS}
    lines = output.splitlines()
    if termination != 'completed' and output and not output.endswith(b'\n'):
        lines = lines[:-1]
    for raw_line in lines:
        try:
            number, token = raw_line.decode('utf-8').split(':', 1)
            line = int(number)
            if not 1 <= line <= MAX_BYTES + 1 or re.fullmatch(SCAN_PATTERN, token) is None:
                raise ValueError()
        except (ValueError, UnicodeError):
            raise ProtocolError(-32001, 'Parser output was invalid') from None
        rule = 'private-key-marker' if token.startswith('-----BEGIN ') else 'dynamic-evaluation' if re.match(r'^(eval|exec)\s*\(', token) else 'credential-identifier'
        counts[rule] += 1
        if len(candidates) < MAX_CANDIDATES:
            candidates.append({'ruleId': rule, 'line': line, 'label': RULE_LABELS[rule], 'status': 'candidate'})
    total = sum(counts.values())
    return {'candidates': candidates, 'counts': counts, 'observedSignals': total,
            'omittedCandidates': total - len(candidates), 'countsComplete': termination == 'completed',
            'partial': termination != 'completed' or total > MAX_CANDIDATES, 'termination': termination}


class Server:
    def __init__(self, environment_id, session_id, assessment_id, runtime, sessions=None,
                 mode='execute', capture_binding=None, control=None):
        self.environment_id = identifier(environment_id)
        self.session_id = identifier(session_id)
        self.assessment_id = identifier(assessment_id)
        self.runtime = runtime
        self.sessions = sessions
        self.initialized = False
        self.ready = False
        self.seen = set()
        self.children = []
        self.cleanup_uncertain = False
        self.scan_used = False
        self.cancel_fd = None
        if mode not in ('execute', 'inspect', 'collect', 'purge', 'retire') or (capture_binding is not None and mode != 'execute'):
            raise ProtocolError(-32001, 'Invalid host session mode')
        self.mode, self.capture_binding, self.control = mode, capture_binding, control
        self.spool = None
        if mode == 'retire':
            validate_retire_control(control)
            if control['producerSessionId'] == self.session_id:
                raise ProtocolError(-32001, 'Control cannot target itself')
        elif mode in ('collect', 'purge'):
            keys = ['producerSessionId', 'captureId', 'ownerProfileFingerprint', 'producerDispatchDeadline'] + (['expectedManifestDigest', 'purgeReceiptId', 'previousReceiptId', 'previousReceiptDigest'] if mode == 'purge' else [])
            exact(control, keys, keys)
            identifier(control['producerSessionId'])
            identifier(control['captureId'])
            if not sha_text(control['ownerProfileFingerprint']):
                raise ProtocolError(-32001, 'Invalid owner profile')
            if type(control['producerDispatchDeadline']) is not int or not 1 <= control['producerDispatchDeadline'] <= 9007199254740991:
                raise ProtocolError(-32001, 'Invalid producer deadline')
            if control['producerSessionId'] == self.session_id:
                raise ProtocolError(-32001, 'Control cannot target itself')
            if mode == 'purge':
                identifier(control['purgeReceiptId'])
                if not sha_text(control['expectedManifestDigest']):
                    raise ProtocolError(-32001, 'Invalid purge digest')
                previous_id, previous_digest = control['previousReceiptId'], control['previousReceiptDigest']
                if (previous_id is None) != (previous_digest is None):
                    raise ProtocolError(-32001, 'Invalid purge predecessor')
                if previous_id is not None:
                    identifier(previous_id)
                    if previous_id == control['purgeReceiptId'] or not sha_text(previous_digest):
                        raise ProtocolError(-32001, 'Invalid purge predecessor')
        elif control is not None:
            raise ProtocolError(-32001, 'Unexpected control binding')

    def capture_ref(self):
        return None if self.capture_binding is None else {key: self.capture_binding[key] for key in ('captureId', 'attemptId')}

    def journal(self):
        if self.sessions is None:
            raise ProtocolError(-32001, 'Durable session journal is required for parser execution')
        record_session(self.sessions, self.metadata(), 'running', self.children, self.capture_ref())

    def scan(self, raw, digest):
        if self.mode != 'execute' or self.scan_used or self.sessions is None:
            raise ProtocolError(-32001, 'Only one journaled parser call is permitted per session')
        self.scan_used = True
        if b'\0' in raw:
            raise ProtocolError(-32602, 'The parser accepts UTF-8 text without NUL only')
        try:
            raw.decode('utf-8')
        except UnicodeError:
            raise ProtocolError(-32602, 'The parser accepts UTF-8 text only') from None
        descriptor, binary_hash = open_ripgrep()
        try:
            if self.capture_binding is not None:
                if self.capture_binding.get('inputSha256') != digest:
                    raise ProtocolError(-32001, 'Capture input binding differs')
                self.spool = SpoolWriter(self, binary_hash)
                self.spool.reached_eof = False
            if binary_digest(descriptor) != binary_hash:
                raise ProtocolError(-32001, 'System parser changed before dispatch')
            version, end = run_fixed(self, descriptor, 'version', b'', self.cancel_fd)
            first = version.splitlines()[0].decode('ascii') if version else ''
            if end != 'completed' or not re.fullmatch(r'ripgrep [0-9]+\.[0-9]+\.[0-9]+(?:[ -][A-Za-z0-9.()+_-]+)*', first):
                raise ProtocolError(-32001, 'Fixed parser version could not be verified')
            if binary_digest(descriptor) != binary_hash:
                raise ProtocolError(-32001, 'System parser changed before dispatch')
            output, termination = run_fixed(self, descriptor, 'scan', raw, self.cancel_fd)
            if self.spool is not None:
                self.spool.captured(termination, self.spool.reached_eof)
            summary = summarize_signals(output, termination)
            if self.spool is not None:
                self.spool.seal()
            return {**self.metadata(), 'operation': 'scan_bytes', 'adapter': ADAPTER, 'rulesVersion': RULES_VERSION,
                    'inputSha256': digest, 'bytes': len(raw), 'tool': {'name': 'ripgrep', 'version': first, 'binarySha256': binary_hash},
                    **summary, 'effects': 'Fixed offline parsing of selected bytes; no target execution, paths or network',
                    'limitations': 'Lexical candidates can be false positives; absence is not proof of security'}
        finally:
            if self.spool is not None:
                try:
                    if self.spool.body['phase'] not in ('sealed', 'interrupted') and not self.cleanup_uncertain:
                        self.spool.seal('parser_error' if self.spool.body['phase'] == 'captured' else 'interrupted')
                finally:
                    self.spool.close()
            os.close(descriptor)

    def metadata(self):
        return {'environmentId': self.environment_id, 'sessionId': self.session_id,
                'assessmentId': self.assessment_id}

    def bind(self, params):
        meta = exact(params.get('_meta'), ['openkoda'], ['openkoda'])
        supplied = exact(meta['openkoda'], ['environmentId', 'sessionId', 'assessmentId'],
                         ['environmentId', 'sessionId', 'assessmentId'])
        if supplied != self.metadata():
            raise ProtocolError(-32001, 'Request belongs to another environment or session')

    def dispatch(self, message):
        request_id = message.get('id') if isinstance(message, dict) else None
        try:
            exact(message, ['jsonrpc', 'id', 'method', 'params'], ['jsonrpc', 'method'])
            if message['jsonrpc'] != '2.0' or not isinstance(message['method'], str):
                raise ProtocolError(-32600, 'Invalid JSON-RPC envelope')
            if 'id' in message and (isinstance(request_id, bool) or not isinstance(request_id, (str, int))):
                raise ProtocolError(-32600, 'Invalid request identity')
            if isinstance(request_id, str) and len(request_id) > 128:
                raise ProtocolError(-32600, 'Request identity exceeds limit')
            if 'id' in message:
                request_key = (type(request_id).__name__, request_id)
                if request_key in self.seen:
                    raise ProtocolError(-32600, 'Duplicate request identity')
                self.seen.add(request_key)
            params = message.get('params', {})
            if not isinstance(params, dict):
                raise ProtocolError(-32602, 'Params must be an object')
            self.bind(params)
            method = message['method']
            if method == 'initialize':
                if self.initialized or 'id' not in message:
                    raise ProtocolError(-32600, 'Initialize must be the first request')
                exact(params, ['protocolVersion', 'clientInfo', 'capabilities', '_meta'],
                      ['protocolVersion', 'clientInfo', 'capabilities', '_meta'])
                if not isinstance(params['protocolVersion'], str) or not isinstance(params['capabilities'], dict):
                    raise ProtocolError(-32602, 'Invalid initialization capabilities')
                info = exact(params['clientInfo'], ['name', 'version'], ['name', 'version'])
                if any(not isinstance(v, str) or not 1 <= len(v) <= 128 for v in info.values()):
                    raise ProtocolError(-32602, 'Invalid client information')
                self.initialized = True
                result = {'protocolVersion': PROTOCOL, 'capabilities': {'tools': {'listChanged': False}},
                          'serverInfo': {'name': 'openkoda-kali-bounded', 'version': '0.1.0'},
                          '_meta': {'openkoda': self.metadata(), 'executorSourceSha256': self.runtime.get('executorSourceSha256'),
                                    'catalogVersion': CATALOG_VERSION, 'catalogSha256': catalog_sha256()}}
            elif method == 'notifications/initialized':
                exact(params, ['_meta'], ['_meta'])
                if not self.initialized or 'id' in message:
                    raise ProtocolError(-32600, 'Unexpected initialization notification')
                self.ready = True
                return None
            else:
                if not self.ready or 'id' not in message:
                    raise ProtocolError(-32002, 'Complete initialization before tool requests')
                if method == 'ping':
                    exact(params, ['_meta'], ['_meta'])
                    result = {}
                elif method == 'tools/list':
                    exact(params, ['_meta'], ['_meta'])
                    result = {'tools': tool_catalog()}
                elif method == 'tools/call':
                    exact(params, ['name', 'arguments', '_meta'], ['name', 'arguments', '_meta'])
                    try:
                        observed = self.call(params['name'], params['arguments'])
                        result = {'content': [{'type': 'text', 'text': 'Owned capture control result' if params['name'] in ('capture_collect', 'capture_purge', 'capture_retire') else json.dumps(observed, ensure_ascii=True)}],
                                  'structuredContent': observed, 'isError': False}
                    except ProtocolError as error:
                        result = {'content': [{'type': 'text', 'text': str(error)}], 'isError': True}
                else:
                    raise ProtocolError(-32601, 'Method unavailable')
            return {'jsonrpc': '2.0', 'id': request_id, 'result': result}
        except ProtocolError as error:
            if isinstance(message, dict) and 'id' not in message:
                return None
            return {'jsonrpc': '2.0', 'id': request_id,
                    'error': {'code': error.code, 'message': str(error)}}

    def call(self, name, arguments):
        if self.mode == 'retire':
            if name != 'capture_retire':
                raise ProtocolError(-32601, 'Retire session accepts only its bound operation')
            return retire_control(self, arguments)
        if self.mode in ('collect', 'purge'):
            if name != ('capture_collect' if self.mode == 'collect' else 'capture_purge'):
                raise ProtocolError(-32601, 'Control session accepts only its bound operation')
            return capture_control(self, name, arguments)
        if self.mode == 'inspect' and name != 'runtime_diagnostics':
            raise ProtocolError(-32601, 'Inspection cannot execute a producer')
        if name == 'runtime_diagnostics':
            exact(arguments, ['inspectSessionId', 'dispatchDeadline'])
            if 'dispatchDeadline' in arguments and 'inspectSessionId' not in arguments:
                raise ProtocolError(-32602, 'Deadline requires a recorded session')
            result = {**self.runtime, **self.metadata(), 'tools': ['runtime_diagnostics', 'hash_bytes', 'scan_bytes']}
            if 'inspectSessionId' in arguments:
                try:
                    identifier(arguments['inspectSessionId'])
                    if self.sessions is None:
                        raise ValueError('Session records unavailable')
                    result['inspectedSession'] = inspect_session(self.sessions, self.environment_id, arguments['inspectSessionId'], arguments.get('dispatchDeadline', 0), self.assessment_id)
                except (ValueError, OSError):
                    raise ProtocolError(-32602, 'Session identity or record is unavailable') from None
            return result
        if name not in ('hash_bytes', 'scan_bytes'):
            raise ProtocolError(-32601, 'Tool unavailable; arbitrary commands are not accepted')
        exact(arguments, ['base64', 'expectedSha256'], ['base64', 'expectedSha256'])
        encoded, expected = arguments['base64'], arguments['expectedSha256']
        if not isinstance(encoded, str) or len(encoded) > 4 * ((MAX_BYTES + 2) // 3):
            raise ProtocolError(-32602, 'Byte input exceeds limit')
        if not isinstance(expected, str) or not re.fullmatch(r'[0-9a-f]{64}', expected):
            raise ProtocolError(-32602, 'Expected SHA-256 is required')
        try:
            raw = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error):
            raise ProtocolError(-32602, 'Input is not canonical base64') from None
        if len(raw) > MAX_BYTES or base64.b64encode(raw).decode('ascii') != encoded:
            raise ProtocolError(-32602, 'Input is not canonical bounded base64')
        observed = hashlib.sha256(raw).hexdigest()
        if observed != expected:
            raise ProtocolError(-32602, 'Input digest differs from host-approved bytes')
        if name == 'scan_bytes':
            return self.scan(raw, observed)
        return {'sha256': observed, 'bytes': len(raw), **self.metadata(),
                'operation': 'hash_bytes', 'effects': 'No input execution or filesystem access'}


def serve(reader, writer, server):
    count = total_input = total_output = 0
    started = time.monotonic()
    while count < MAX_MESSAGES and time.monotonic() - started < 300:
        line = reader.readline(MAX_LINE + 1)
        if not line:
            return 0
        total_input += len(line)
        if len(line) > MAX_LINE or not line.endswith(b'\n') or total_input > MAX_TOTAL_INPUT:
            return 2
        count += 1
        try:
            message = json.loads(line.decode('utf-8'), parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
        except (ValueError, UnicodeError, RecursionError):
            reply = {'jsonrpc': '2.0', 'id': None, 'error': {'code': -32700, 'message': 'Invalid JSON frame'}}
        else:
            reply = server.dispatch(message)
        if reply is not None:
            data = json.dumps(reply, ensure_ascii=True, separators=(',', ':')).encode('ascii') + b'\n'
            total_output += len(data)
            if len(data) > 8192 or total_output > (MAX_COLLECT_OUTPUT if server.mode == 'collect' else MAX_TOTAL_OUTPUT):
                return 2
            writer.write(data)
            writer.flush()
    return 2


def main():
    parser = argparse.ArgumentParser(description='Private bounded OpenKoda MCP executor')
    parser.add_argument('--environment-id', required=True, type=identifier)
    parser.add_argument('--session-id', required=True, type=identifier)
    parser.add_argument('--assessment-id', required=True, type=identifier)
    args = parser.parse_args()
    runtime = verify_runtime(args.environment_id)
    runtime['executorSourceSha256'] = globals().get('__openkoda_source_sha256__') or hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()
    import resource
    import signal
    resource.setrlimit(resource.RLIMIT_CPU, (15, 15))
    resource.setrlimit(resource.RLIMIT_AS, (128 * 1024 * 1024, 128 * 1024 * 1024))
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))
    def interrupted(_signum, _frame):
        raise TimeoutError('MCP session lifetime exceeded')
    signal.signal(signal.SIGALRM, interrupted)
    signal.alarm(300)
    server = Server(args.environment_id, args.session_id, args.assessment_id, runtime, session_directory(),
                    globals().get('__openkoda_mode__', 'execute'), globals().get('__openkoda_capture__'), globals().get('__openkoda_control__'))
    server.cancel_fd = sys.stdin.fileno()
    server.journal()
    code = 2
    try:
        code = serve(sys.stdin.buffer, sys.stdout.buffer, server)
        return code
    finally:
        if server.cleanup_uncertain or not children_quiescent(server.children, server.session_id):
            record_session(server.sessions, server.metadata(), 'unknown', server.children, server.capture_ref())
            raise RuntimeError('Child cleanup was not confirmed')
        record_session(server.sessions, server.metadata(), 'closed', server.children, server.capture_ref())
        message = {'jsonrpc': '2.0', 'method': 'notifications/openkoda/closed',
                   'params': {**server.metadata(), 'code': code, 'childrenQuiescent': True, 'executorSourceSha256': runtime['executorSourceSha256']}}
        sys.stdout.write(json.dumps(message, separators=(',', ':')) + '\n')
        sys.stdout.flush()


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, TimeoutError):
        print('OpenKoda executor stopped: runtime, identity or resource check failed.', file=sys.stderr)
        raise SystemExit(2) from None
