"""Temporary directories and descriptors only; no registrations or real Linux state."""
import importlib.util
import os
import pathlib
import sys
import tempfile
import unittest
import uuid
from unittest.mock import patch


def local_module(name):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@unittest.skipUnless(sys.platform == 'linux', 'Uses Linux descriptor and ownership semantics')
class ArchiveHelperTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix='openkoda-archive-helper-')
        self.addCleanup(temp.cleanup)
        self.home = pathlib.Path(temp.name)
        self.environment = str(uuid.uuid4())
        self.executor = local_module('kali-executor')
        self.inspector = local_module('kali-archive-inspect')
        self.marker = local_module('kali-restore-marker')
        self.probe = self.enterContext(patch.object(self.executor, 'verify_runtime'))
        self.session = self.home / '.local/state/openkoda-sessions'
        for path in (self.home / '.local', self.home / '.local/state', self.session):
            path.mkdir(mode=0o700)

    def inventory(self):
        return self.inspector.classify(vars(self.executor), self.environment, self.home)

    def root(self):
        spool = self.session / 'captures-v1'
        spool.mkdir(mode=0o700)
        (spool / 'quota.lock').touch(mode=0o600)
        slot = spool / str(uuid.uuid4())
        slot.mkdir(mode=0o700)
        return slot

    def test_missing_spool_is_a_read_without_creating_any_name(self):
        self.assertEqual(self.inventory(), {'rawCaptures': 0, 'terminalCaptures': 0, 'sessionRecords': 0})
        self.probe.assert_called_once_with(self.environment)
        self.assertEqual(list(self.session.iterdir()), [])

    def test_raw_capture_does_not_return_any_content(self):
        slot = self.root()
        (slot / 'stdout.bin').write_bytes(b'synthetic unapproved bytes')
        self.assertEqual(self.inventory(), {'rawCaptures': 1, 'terminalCaptures': 0, 'sessionRecords': 0})

    def test_unconfirmed_metadata_refuses_the_inventory(self):
        (self.root() / 'manifest.json').write_bytes(b'partial')
        with self.assertRaisesRegex(ValueError, 'Incomplete'):
            self.inventory()

    def test_real_terminal_producer_and_purge_have_only_typed_metadata(self):
        from test_kali_executor import LinuxRetireMetadataTests
        f = LinuxRetireMetadataTests('test_terminal_metadata_retire_preserves_original_proof_and_replay_is_readonly')
        f.setUp()
        self.addCleanup(f.doCleanups)
        # Move only fixtures produced under a new Linux TemporaryDirectory.
        for item in f.directory.iterdir():
            item.rename(self.session / item.name)
        self.environment = f.environment
        with patch.object(self.executor, 'inspect_session', return_value={'known': True, 'active': False}):
            self.assertEqual(self.inventory(), {'rawCaptures': 0, 'terminalCaptures': 1, 'sessionRecords': 1})

    def test_marker_changes_only_the_new_copy_and_rejects_replay(self):
        directory = os.open(self.home, os.O_RDONLY | os.O_DIRECTORY)
        self.addCleanup(os.close, directory)
        new = str(uuid.uuid4())
        marker = self.home / 'environment-id'
        marker.write_text(self.environment + '\n')
        marker.chmod(0o644)
        self.marker.replace_marker(directory, self.environment, new, os.geteuid())
        self.assertEqual(marker.read_text(), new + '\n')
        with self.assertRaisesRegex(RuntimeError, 'differs'):
            self.marker.replace_marker(directory, self.environment, new, os.geteuid())
        self.assertEqual(marker.read_text(), new + '\n')

    def test_linked_marker_preserves_the_substitute(self):
        path = self.home / 'other'
        path.write_text(self.environment + '\n')
        os.link(path, self.home / 'environment-id')
        directory = os.open(self.home, os.O_RDONLY | os.O_DIRECTORY)
        self.addCleanup(os.close, directory)
        with self.assertRaisesRegex(RuntimeError, 'Unsafe marker'):
            self.marker.replace_marker(directory, self.environment, str(uuid.uuid4()), os.geteuid())
        self.assertEqual(path.read_text(), self.environment + '\n')


if __name__ == '__main__':
    unittest.main()
