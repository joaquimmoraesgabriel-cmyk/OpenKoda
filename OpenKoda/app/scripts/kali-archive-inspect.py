"""Read-only classification of the *managed* spool before exporting an entire disk."""
import contextlib
import fcntl
import json
import os
import pathlib
import pwd
import stat
import sys


def classify(linux, environment, home=None):
    # Trusted helper bytes arrive directly from the host. No source or arguments
    # come from the disk being inspected.
    linux['verify_runtime'](environment)
    if home is None:
        home = pathlib.Path(pwd.getpwuid(os.geteuid()).pw_dir)
    root = {'rawCaptures': 0, 'terminalCaptures': 0, 'sessionRecords': 0}
    with contextlib.ExitStack() as handles:
        def keep(fd):
            handles.callback(os.close, fd)
            return fd
        try:
            parent = keep(linux['directory_fd'](home))
            for name in ('.local', 'state', 'openkoda-sessions'):
                meta = os.fstat(parent)
                if meta.st_uid != os.geteuid() or meta.st_mode & 0o022:
                    raise ValueError('Session ancestor ownership differs')
                parent = keep(os.open(name, os.O_DIRECTORY | os.O_NOFOLLOW | os.O_RDONLY, dir_fd=parent))
        except FileNotFoundError:
            return root
        linux['private_fd'](parent, True)
        with os.scandir(parent) as rows:
            for row in rows:
                if row.name == 'captures-v1':
                    continue
                if not row.name.endswith('.json') or root['sessionRecords'] >= 100000:
                    raise ValueError('Unknown session metadata')
                sid = row.name[:-5]
                linux['identifier'](sid)
                record = linux['validate_session_record'](linux['read_owned'](parent, row.name, 4096), environment, sid)
                # A persisted running state can reflect an interrupted old
                # session, even across boots. Only a confirmed closed one is
                # considered a terminal record for this archive contract.
                if record['state'] != 'closed':
                    if linux['inspect_session'](home / '.local/state/openkoda-sessions', environment, sid)['active'] is not False:
                        raise ValueError('Unresolved session')
                root['sessionRecords'] += 1
        try:
            spool = keep(os.open('captures-v1', os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY, dir_fd=parent))
        except FileNotFoundError:
            return root
        linux['private_fd'](spool, True)
        lock = keep(os.open('quota.lock', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=spool))
        linux['private_fd'](lock)
        fcntl.flock(lock, fcntl.LOCK_SH | fcntl.LOCK_NB)
        with os.scandir(spool) as rows:
            for row in rows:
                if row.name == 'quota.lock':
                    continue
                if root['terminalCaptures'] + root['rawCaptures'] >= linux['SPOOL_COUNT']:
                    raise ValueError('Spool inventory exceeds its bound')
                capture = linux['open_capture'](spool, row.name)
                try:
                    names = set()
                    with os.scandir(capture) as entries:
                        for entry in entries:
                            names.add(entry.name)
                            if len(names) > 5:
                                raise ValueError('Unexpected capture contents')
                    if 'stdout.bin' in names:
                        root['rawCaptures'] += 1
                        continue
                    if names != {'manifest.json', 'purge.json'}:
                        raise ValueError('Incomplete or unexpected capture metadata')
                    manifest = linux['spool_manifest'](capture)
                    if manifest['body']['captureId'] != row.name or manifest['body']['environmentId'] != environment:
                        raise ValueError('Capture belongs to another environment')
                    if linux['purge_record'](capture, manifest)['state'] != 'purged':
                        raise ValueError('Raw removal has not been confirmed')
                    root['terminalCaptures'] += 1
                finally:
                    os.close(capture)
    return root


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise ValueError('Missing environment identity')
    source = sys.stdin.buffer.read(1024 * 1024 + 1)
    if len(source) > 1024 * 1024:
        raise ValueError('Unexpected helper size')
    module = {'__name__': 'openkoda_archive_library'}
    exec(compile(source, 'private-kali-protocol', 'exec'), module)
    print(json.dumps(classify(module, sys.argv[1]), separators=(',', ':')))
