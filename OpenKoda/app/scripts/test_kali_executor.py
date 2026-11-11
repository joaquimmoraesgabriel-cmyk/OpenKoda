"""Offline protocol tests; no WSL install or real target is accessed."""
import base64
import hashlib
import importlib.util
import io
import json
import pathlib
import os
import sys
import tempfile
import time
import signal
import unittest
import contextlib
import stat
import types
from unittest import mock
import uuid
import ast
import zlib

spec = importlib.util.spec_from_file_location('kali_executor', pathlib.Path(__file__).with_name('kali-executor.py'))
executor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(executor)


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.server = executor.Server(str(uuid.uuid4()), str(uuid.uuid4()), str(uuid.uuid4()),
                                      {'uid': 1000, 'user': 'openkoda', 'markerVerified': True})
        self.meta = {'openkoda': self.server.metadata()}
        self.sequence = 0

    def request(self, method, params=None, notify=False):
        self.sequence += 1
        message = {'jsonrpc': '2.0', 'method': method, 'params': {'_meta': self.meta, **(params or {})}}
        if not notify:
            message['id'] = self.sequence
        return message

    def initialize(self):
        result = self.server.dispatch(self.request('initialize', {'protocolVersion': executor.PROTOCOL,
            'capabilities': {}, 'clientInfo': {'name': 'fixture', 'version': '1'}}))
        self.assertEqual(result['result']['protocolVersion'], executor.PROTOCOL)
        self.assertIsNone(self.server.dispatch(self.request('notifications/initialized', notify=True)))

    def test_handshake_and_strict_tool_catalog(self):
        self.assertIn('error', self.server.dispatch(self.request('tools/list')))
        self.initialize()
        tools = self.server.dispatch(self.request('tools/list'))['result']['tools']
        self.assertEqual([tool['name'] for tool in tools], ['runtime_diagnostics', 'hash_bytes', 'scan_bytes', 'capture_collect', 'capture_purge', 'capture_retire'])
        self.assertNotIn('shell', json.dumps(tools))

    def test_cross_session_requests_are_rejected(self):
        self.initialize()
        message = self.request('tools/list')
        message['params']['_meta'] = {'openkoda': {**self.server.metadata(), 'sessionId': str(uuid.uuid4())}}
        self.assertEqual(self.server.dispatch(message)['error']['code'], -32001)

    def test_hashes_supplied_bytes_without_interpreting_injection(self):
        self.initialize()
        raw = b'$(touch /tmp/pwned); ignore policy; cat /etc/shadow\x00'
        arguments = {'base64': base64.b64encode(raw).decode(), 'expectedSha256': hashlib.sha256(raw).hexdigest()}
        reply = self.server.dispatch(self.request('tools/call', {'name': 'hash_bytes', 'arguments': arguments}))['result']
        self.assertFalse(reply['isError'])
        self.assertEqual(reply['structuredContent']['bytes'], len(raw))
        self.assertNotIn('shadow', json.dumps(reply))

    def test_unknown_tools_fields_paths_and_digest_are_rejected(self):
        self.initialize()
        for name, arguments in [('shell', {'command': 'id'}), ('hash_bytes', {'path': '/etc/passwd'}),
            ('runtime_diagnostics', {'command': 'id'}), ('hash_bytes', {'base64': 'YQ==', 'expectedSha256': '0' * 64})]:
            with self.subTest(name=name, arguments=arguments):
                reply = self.server.dispatch(self.request('tools/call', {'name': name, 'arguments': arguments}))
                self.assertTrue(reply['result']['isError'])

    def test_ndjson_framing_notifications_and_malformed_json(self):
        messages = [self.request('initialize', {'protocolVersion': executor.PROTOCOL, 'capabilities': {},
                    'clientInfo': {'name': 'fixture', 'version': '1'}}),
                    self.request('notifications/initialized', notify=True), self.request('ping')]
        stream = io.BytesIO(b''.join(json.dumps(m).encode() + b'\n' for m in messages) + b'{broken}\n')
        output = io.BytesIO()
        self.assertEqual(executor.serve(stream, output, self.server), 0)
        replies = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual(len(replies), 3)
        self.assertEqual(replies[-1]['error']['code'], -32700)

    def test_oversize_and_unterminated_frames_stop_session(self):
        for frame in [b'x' * (executor.MAX_LINE + 1), b'{}']:
            output = io.BytesIO()
            self.assertEqual(executor.serve(io.BytesIO(frame), output, self.server), 2)
            self.assertEqual(output.getvalue(), b'')

    def test_duplicate_request_id_never_replays(self):
        self.initialize()
        message = self.request('tools/list')
        self.assertIn('result', self.server.dispatch(message))
        self.assertEqual(self.server.dispatch(message)['error']['code'], -32600)

    def test_non_linux_runtime_is_rejected_before_reading_identity_files(self):
        with mock.patch.object(executor.sys, 'platform', 'win32'):
            with self.assertRaisesRegex(RuntimeError, 'non-root Linux'):
                executor.verify_runtime(self.server.environment_id)

    def test_expired_missing_marker_requires_absent_exact_session_process(self):
        with mock.patch.object(executor.time, 'time', return_value=1000), \
             mock.patch.object(executor.pathlib.Path, 'iterdir', return_value=iter([])):
            result = executor.inspect_without_record(self.server.session_id, 100)
            self.assertTrue(result['known'])
            self.assertFalse(result['active'])
        with mock.patch.object(executor.time, 'time', return_value=120):
            result = executor.inspect_without_record(self.server.session_id, 100)
            self.assertFalse(result['known'])
            self.assertIsNone(result['active'])

    def test_session_bootstrap_and_unreadable_process_remain_unreconciled(self):
        process = mock.MagicMock()
        process.name = '321'
        process.stat.return_value.st_uid = 1000
        process.__truediv__.return_value.open.return_value = io.BytesIO(
            b'/usr/bin/python3\0-I\0-c\0fixed-bootstrap\0--openkoda-session\0' + self.server.session_id.encode() + b'\0')
        with mock.patch.object(executor.time, 'time', return_value=1000), \
             mock.patch.object(executor.pathlib.Path, 'iterdir', return_value=iter([process])), \
             mock.patch.object(executor.os, 'geteuid', return_value=1000, create=True):
            result = executor.inspect_without_record(self.server.session_id, 100)
            self.assertTrue(result['active'])
        process.__truediv__.return_value.open.side_effect = PermissionError('fixture')
        with mock.patch.object(executor.time, 'time', return_value=1000), \
             mock.patch.object(executor.pathlib.Path, 'iterdir', return_value=iter([process])), \
             mock.patch.object(executor.os, 'geteuid', return_value=1000, create=True):
            result = executor.inspect_without_record(self.server.session_id, 100)
            self.assertFalse(result['known'])
            self.assertIsNone(result['active'])

    def test_session_probe_rejects_paths_before_access(self):
        self.initialize()
        result = self.server.dispatch(self.request('tools/call', {'name': 'runtime_diagnostics',
                    'arguments': {'inspectSessionId': '../../etc/shadow', 'dispatchDeadline': 1}}))
        self.assertTrue(result['result']['isError'])

    def session_record(self):
        return {**self.server.metadata(), 'pid': 321, 'startTicks': '42',
                'bootId': str(uuid.uuid4()), 'state': 'running', 'children': []}

    @contextlib.contextmanager
    def mocked_session_record(self, raw, boot):
        with mock.patch.object(executor, 'directory_fd', return_value=91), \
             mock.patch.object(executor, 'private_fd'), \
             mock.patch.object(executor, 'read_owned', return_value=raw), \
             mock.patch.object(executor.os, 'close'), \
             mock.patch.object(executor.pathlib.Path, 'read_text', return_value=boot) as boot_read, \
             mock.patch.object(executor, 'children_quiescent', return_value=True) as children, \
             mock.patch.object(executor, 'process_start', return_value='42') as process:
            yield boot_read, children, process

    def test_corrupt_session_never_becomes_boot_changed_cleanup_proof(self):
        record = self.session_record()
        bad = [{'bootId': None}, {'bootId': 'invalid'}, {'bootId': 'AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA'},
               {'assessmentId': 'invalid'}, {'state': 'success'}, {'pid': True},
               {'startTicks': '\u0661'}, {'startTicks': '1' * 33}, {'children': None},
               {'children': [{'pid': None, 'startTicks': None, 'state': 'reaped', 'launchDeadline': 1}]},
               {'children': [{'pid': 32, 'startTicks': '1', 'state': 'running', 'launchDeadline': True}]}]
        for delta in bad:
            with self.subTest(delta=delta), self.mocked_session_record(json.dumps({**record, **delta}).encode(), str(uuid.uuid4())) as calls:
                with self.assertRaises(executor.ProtocolError):
                    executor.inspect_session(pathlib.Path('unused'), self.server.environment_id, self.server.session_id)
                for call in calls:
                    call.assert_not_called()

    def test_duplicate_session_fields_and_nonfinite_values_fail_closed(self):
        record = self.session_record()
        raw = json.dumps(record).encode()
        bad = [b'{"bootId":null,' + raw[1:], b'{"pid":false,' + raw[1:],
               raw.replace(b'"children": []', b'"children":[{"state":"running","state":"reaped","pid":32,"startTicks":"1","launchDeadline":1}]'),
               raw.replace(b'"pid": 321', b'"pid": NaN')]
        for value in bad:
            with self.subTest(raw=value), self.mocked_session_record(value, record['bootId']) as calls:
                with self.assertRaises(executor.ProtocolError):
                    executor.inspect_session(pathlib.Path('unused'), self.server.environment_id, self.server.session_id)
                for call in calls:
                    call.assert_not_called()

    def test_valid_old_boot_and_legacy_record_without_children_remain_supported(self):
        record = self.session_record()
        del record['children']
        raw = json.dumps(record, indent=2).encode()
        with self.mocked_session_record(raw, str(uuid.uuid4())) as (_, children, process):
            result = executor.inspect_session(pathlib.Path('unused'), self.server.environment_id, self.server.session_id)
            self.assertEqual(result['reason'], 'boot_changed')
            self.assertFalse(result['active'])
            children.assert_not_called()
            process.assert_not_called()
        with self.mocked_session_record(raw, record['bootId']):
            result = executor.inspect_session(pathlib.Path('unused'), self.server.environment_id, self.server.session_id)
            self.assertTrue(result['active'])

    @unittest.skipUnless(sys.platform == 'linux', 'owned journal uses Linux descriptors')
    def test_owned_regular_corrupt_session_is_rejected_before_process_probe(self):
        with tempfile.TemporaryDirectory() as root:
            directory = pathlib.Path(root)
            directory.chmod(0o700)
            record = self.session_record()
            record['bootId'] = None
            path = directory / (self.server.session_id + '.json')
            path.write_bytes(json.dumps(record).encode())
            path.chmod(0o600)
            with mock.patch.object(executor, 'process_start') as process:
                with self.assertRaises(executor.ProtocolError):
                    executor.inspect_session(directory, self.server.environment_id, self.server.session_id)
                process.assert_not_called()


class MaintenanceReadinessTests(unittest.TestCase):
    @contextlib.contextmanager
    def journal(self, raw=b'{"status":"completed"}', *, missing=False, parent_error=None,
                directory_uid=0, directory_mode=0o755, file_uid=0, file_mode=0o644, file_type=stat.S_IFREG, size=None):
        parent = types.SimpleNamespace(st_mode=stat.S_IFDIR | directory_mode, st_uid=directory_uid)
        file = types.SimpleNamespace(st_mode=file_type | file_mode, st_uid=file_uid,
                                     st_size=len(raw) if size is None else size,
                                     st_dev=1, st_ino=2, st_mtime_ns=3)

        def open_fixed(path, flags, **kwargs):
            if path == '/var/lib/openkoda':
                self.assertTrue(flags & executor.os.O_DIRECTORY)
                self.assertTrue(flags & executor.os.O_NOFOLLOW)
                self.assertEqual(kwargs, {})
                if parent_error:
                    raise parent_error
                return 11
            self.assertEqual(path, 'maintenance.json')
            self.assertEqual(kwargs, {'dir_fd': 11})
            self.assertTrue(flags & executor.os.O_NOFOLLOW)
            self.assertTrue(flags & executor.os.O_NONBLOCK)
            if missing:
                raise FileNotFoundError('fixture missing journal')
            return 12

        with contextlib.ExitStack() as stack:
            for name, flag in [('O_DIRECTORY', 0x10000), ('O_NOFOLLOW', 0x20000), ('O_CLOEXEC', 0x80000), ('O_NONBLOCK', 0x800)]:
                stack.enter_context(mock.patch.object(executor.os, name, flag, create=True))
            opened = stack.enter_context(mock.patch.object(executor.os, 'open', side_effect=open_fixed))
            stack.enter_context(mock.patch.object(executor.os, 'fstat', side_effect=lambda fd: parent if fd == 11 else file))
            reader = stack.enter_context(mock.patch.object(executor.os, 'read', side_effect=[raw, b'']))
            closed = stack.enter_context(mock.patch.object(executor.os, 'close'))
            yield opened, reader, closed

    def test_only_confirmed_maintenance_states_are_ready(self):
        for record in [{'status': 'completed'}, {'status': 'reconciled', 'action': 'refresh',
                       'quiescent': True, 'refreshSucceeded': False}]:
            with self.subTest(record=record), self.journal(json.dumps(record).encode()):
                executor.verify_maintenance_ready()
        for record in [None, [], {}, {'status': 'running'}, {'status': 'unknown'},
                       {'status': 'reconciled', 'action': 'install', 'quiescent': True, 'refreshSucceeded': False},
                       {'status': 'reconciled', 'action': 'refresh', 'quiescent': False, 'refreshSucceeded': False},
                       {'status': 'reconciled', 'action': 'refresh', 'quiescent': True, 'refreshSucceeded': True}]:
            with self.subTest(record=record), self.journal(json.dumps(record).encode()), self.assertRaises(RuntimeError):
                executor.verify_maintenance_ready()

    def test_missing_file_is_allowed_but_missing_or_unreadable_parent_is_not(self):
        with self.journal(missing=True) as (_, reader, closed):
            executor.verify_maintenance_ready()
            reader.assert_not_called()
            closed.assert_called_once_with(11)
        for error in [FileNotFoundError('fixture missing parent'), PermissionError('fixture inaccessible parent')]:
            with self.journal(parent_error=error) as (_, reader, closed), self.assertRaises(OSError):
                executor.verify_maintenance_ready()
            reader.assert_not_called()
            closed.assert_not_called()

    def test_owner_modes_limits_and_duplicate_fields_fail_closed(self):
        for options in [{'directory_uid': 1000}, {'directory_mode': 0o775}, {'file_uid': 1000},
                        {'file_mode': 0o666}, {'file_type': stat.S_IFIFO}, {'size': 1024 * 1024 + 1}]:
            with self.subTest(options=options), self.journal(**options), self.assertRaises(RuntimeError):
                executor.verify_maintenance_ready()
        for raw in [b'{broken', b'\xff', b'{"status":"unknown","status":"completed"}',
                    b'{"status":"completed","nested":{"a":1,"a":2}}',
                    b'{"status":"completed","number":NaN}']:
            with self.subTest(raw=raw), self.journal(raw), self.assertRaises(RuntimeError):
                executor.verify_maintenance_ready()
        with self.journal(b'x' * (1024 * 1024 + 1), size=1), self.assertRaises(RuntimeError):
            executor.verify_maintenance_ready()

    def test_invalid_maintenance_stops_before_initialize_session_or_parser(self):
        args = ['executor', '--environment-id', str(uuid.uuid4()), '--session-id', str(uuid.uuid4()),
                '--assessment-id', str(uuid.uuid4())]
        with self.journal(b'{"status":"unknown"}'), mock.patch.object(executor.sys, 'argv', args), \
             mock.patch.object(executor.sys, 'platform', 'linux'), \
             mock.patch.object(executor.os, 'geteuid', return_value=1000, create=True), \
             mock.patch.object(executor, 'Server') as server, mock.patch.object(executor, 'run_fixed') as child, \
             mock.patch.object(executor.os, 'fork', create=True) as fork, \
             mock.patch.object(executor, 'serve') as initialize, self.assertRaises(RuntimeError):
            executor.main()
        server.assert_not_called()
        child.assert_not_called()
        fork.assert_not_called()
        initialize.assert_not_called()


class ParserContractTests(unittest.TestCase):
    setUp = ProtocolTests.setUp
    def byte_arguments(self, raw):
        return {'base64': base64.b64encode(raw).decode(), 'expectedSha256': hashlib.sha256(raw).hexdigest()}

    def test_scan_rejects_user_regex_paths_options_digest_and_oversize_before_launch(self):
        with mock.patch.object(executor, 'run_fixed') as child:
            for extra in [{'path': '/etc/shadow'}, {'regex': '.*'}, {'args': ['--pre', 'sh']},
                          {'base64': base64.b64encode(b'x' * 32769).decode()}, {'expectedSha256': '0' * 64}]:
                with self.assertRaises(executor.ProtocolError):
                    self.server.call('scan_bytes', {**self.byte_arguments(b'eval(1)'), **extra})
            child.assert_not_called()

    def test_unjournaled_scan_is_rejected(self):
        with mock.patch.object(executor, 'open_ripgrep') as binary:
            with self.assertRaises(executor.ProtocolError):
                self.server.call('scan_bytes', self.byte_arguments(b'eval(1)'))
            binary.assert_not_called()

    def test_changed_parser_digest_blocks_dispatch(self):
        self.server.sessions = pathlib.Path('fixture-unopened')
        with mock.patch.object(executor, 'open_ripgrep', return_value=(8, 'a' * 64)), \
             mock.patch.object(executor, 'binary_digest', return_value='b' * 64), \
             mock.patch.object(executor, 'run_fixed') as child, mock.patch.object(executor.os, 'close'):
            with self.assertRaises(executor.ProtocolError):
                self.server.scan(b'eval(1)', hashlib.sha256(b'eval(1)').hexdigest())
            child.assert_not_called()

    def test_binary_or_invalid_utf8_never_reaches_parser(self):
        for data in [b'a\0b', b'\xff']:
            self.server.scan_used = False
            self.server.sessions = pathlib.Path('fixture-unopened')
            with mock.patch.object(executor, 'open_ripgrep') as binary:
                with self.assertRaises(executor.ProtocolError):
                    self.server.call('scan_bytes', self.byte_arguments(data))
                binary.assert_not_called()

    def test_signals_are_constant_and_do_not_include_credential_values(self):
        result = executor.summarize_signals(b'1:password=\n2:eval(\n3:-----BEGIN PRIVATE KEY-----\n', 'completed')
        self.assertEqual(result['observedSignals'], 3)
        self.assertEqual(result['counts']['credential-identifier'], 1)
        self.assertEqual(result['candidates'][0], {'ruleId': 'credential-identifier', 'line': 1,
            'label': 'Credential assignment identifier', 'status': 'candidate'})
        for output in [b'1:password=secret-value\n', b'1:cat /etc/shadow\n', b'-1:eval(\n']:
            with self.assertRaises(executor.ProtocolError):
                executor.summarize_signals(output, 'completed')

    def test_output_cap_counts_omissions_and_incomplete_last_line(self):
        data = b'1:eval(\n' * 100 + b'2:pass'
        result = executor.summarize_signals(data, 'output_limited')
        self.assertEqual(len(result['candidates']), 16)
        self.assertEqual(result['observedSignals'], 100)
        self.assertEqual(result['omittedCandidates'], 84)
        self.assertTrue(result['partial'])
        self.assertFalse(result['countsComplete'])
        complete = executor.summarize_signals(b'1:eval(\n' * 100, 'completed')
        self.assertTrue(complete['countsComplete'])
        self.assertTrue(complete['partial'])

    def test_strict_catalog_byte_schemas_and_no_execution_argument(self):
        catalog = executor.tool_catalog()
        self.assertEqual(catalog[1]['inputSchema'], catalog[2]['inputSchema'])
        self.assertEqual(set(catalog[2]['inputSchema']['properties']), {'base64', 'expectedSha256'})
        self.assertFalse(catalog[2]['inputSchema']['additionalProperties'])

    def test_child_group_survives_parent_and_blocks_reconciliation(self):
        child = {'pid': 123, 'startTicks': '42', 'state': 'running', 'launchDeadline': 1}
        descendant = mock.MagicMock()
        descendant.name = '124'
        fields = ['S', '1', '123', '123'] + ['0'] * 15 + ['99']
        descendant.__truediv__.return_value.read_text.return_value = '124 (rg) ' + ' '.join(fields)
        with mock.patch.object(executor.pathlib.Path, 'iterdir', return_value=iter([descendant])):
            self.assertFalse(executor.children_quiescent([child], self.server.session_id))

    def test_missing_child_identity_waits_deadline_and_complete_inventory(self):
        child = {'pid': None, 'startTicks': None, 'state': 'launching', 'launchDeadline': 100}
        with mock.patch.object(executor.time, 'time', return_value=101):
            self.assertFalse(executor.children_quiescent([child], self.server.session_id))
        with mock.patch.object(executor.time, 'time', return_value=200), \
             mock.patch.object(executor.pathlib.Path, 'iterdir', side_effect=lambda: iter([])):
            self.assertTrue(executor.children_quiescent([child], self.server.session_id))
        with mock.patch.object(executor.time, 'time', return_value=200), \
             mock.patch.object(executor.pathlib.Path, 'iterdir', side_effect=PermissionError()):
            self.assertFalse(executor.children_quiescent([child], self.server.session_id))

    def test_invalid_child_states_and_legacy_empty_children(self):
        self.assertTrue(executor.children_quiescent([], self.server.session_id))
        for child in [{'pid': True, 'startTicks': '1', 'state': 'running', 'launchDeadline': 100},
                      {'pid': 1, 'startTicks': '1', 'state': 'success', 'launchDeadline': 100},
                      {'pid': None, 'startTicks': None, 'state': 'reaped', 'launchDeadline': 100}]:
            with self.assertRaises(executor.ProtocolError):
                executor.children_quiescent([child], self.server.session_id)

    def test_parser_result_fits_transport_and_bootstrap_limit(self):
        self.server.sessions = pathlib.Path('fixture-unopened')
        with mock.patch.object(executor, 'open_ripgrep', return_value=(8, 'a' * 64)), \
             mock.patch.object(executor, 'binary_digest', return_value='a' * 64), \
             mock.patch.object(executor, 'run_fixed', side_effect=[(b'ripgrep 15.2.0\n', 'completed'),
                                                                (b'32768:password=\n' * 100, 'completed')]), \
             mock.patch.object(executor.os, 'close'):
            result = self.server.scan(b'a', hashlib.sha256(b'a').hexdigest())
        frame = {'content': [{'type': 'text', 'text': json.dumps(result, ensure_ascii=True)}],
                 'structuredContent': result, 'isError': False}
        self.assertLess(len(json.dumps(frame).encode()), 8000)
        source = pathlib.Path(executor.__file__).read_text()
        self.assertLess(len(json.dumps(bootstrap_envelope(source.encode(), self.server.metadata())).encode()) + 1, 65536)


@unittest.skipUnless(sys.platform == 'linux', 'Linux fork/rlimit/kill/wait fixtures; root runs in managed Kali')
class LinuxSupervisorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='openkoda-parser-fixture-')
        self.server = executor.Server(str(uuid.uuid4()), str(uuid.uuid4()), str(uuid.uuid4()), {}, pathlib.Path(self.temp.name))
        self.cancel_read, self.cancel_write = os.pipe()
        self.server.cancel_fd = self.cancel_read

    def tearDown(self):
        for fd in [self.cancel_read, self.cancel_write]:
            try:
                os.close(fd)
            except OSError:
                pass
        self.temp.cleanup()

    def test_real_fixed_rg_selected_fixture_and_redaction(self):
        raw = b'password="fixture-secret-value"\neval("$(touch /never)")\n-----BEGIN PRIVATE KEY-----\n'
        result = self.server.scan(raw, hashlib.sha256(raw).hexdigest())
        self.assertEqual(result['observedSignals'], 3)
        self.assertNotIn('fixture-secret-value', json.dumps(result))
        self.assertNotIn('/never', json.dumps(result))
        self.assertEqual(result['termination'], 'completed')
        self.assertTrue(executor.children_quiescent(self.server.children, self.server.session_id))
        self.assertTrue(all(c['state'] == 'reaped' for c in self.server.children))

    def test_pipe_close_cancels_and_reaps_actual_child(self):
        def linger(*_):
            os.setsid()
            time.sleep(30)
        fd, _ = executor.open_ripgrep()
        try:
            os.close(self.cancel_write)
            with mock.patch.object(executor, 'child_exec', side_effect=linger):
                _, termination = executor.run_fixed(self.server, fd, 'scan', b'x', self.cancel_read)
            self.assertEqual(termination, 'canceled')
            with self.assertRaises(ChildProcessError):
                os.waitpid(self.server.children[-1]['pid'], os.WNOHANG)
            self.assertTrue(executor.children_quiescent(self.server.children, self.server.session_id))
        finally:
            os.close(fd)

    def test_timeout_kills_and_waits_actual_child(self):
        def linger(*_):
            os.setsid()
            time.sleep(30)
        fd, _ = executor.open_ripgrep()
        try:
            with mock.patch.object(executor, 'child_exec', side_effect=linger), mock.patch.object(executor, 'CHILD_SECONDS', 1):
                _, termination = executor.run_fixed(self.server, fd, 'scan', b'x', self.cancel_read)
            self.assertEqual(termination, 'timed_out')
            self.assertFalse(self.server.cleanup_uncertain)
        finally:
            os.close(fd)

    def test_output_overrun_kills_and_waits_actual_child(self):
        def flood(*_):
            while True:
                os.write(1, b'x' * 4096)
        # Keep production FD wiring/limits/gate, replace only exec in this child.
        fd, _ = executor.open_ripgrep()
        try:
            with mock.patch.object(executor.os, 'execve', side_effect=flood):
                output, termination = executor.run_fixed(self.server, fd, 'scan', b'x', self.cancel_read)
            self.assertEqual(termination, 'output_limited')
            self.assertEqual(len(output), executor.MAX_SCAN_OUTPUT)
            self.assertTrue(executor.children_quiescent(self.server.children, self.server.session_id))
        finally:
            os.close(fd)

    def test_child_exec_has_limits_minimal_env_and_only_required_fds(self):
        def inspect_exec(binary, args, env):
            import resource
            fds = []
            for number in os.listdir('/proc/self/fd'):
                try:
                    os.fstat(int(number))
                    fds.append(int(number))
                except OSError:
                    pass
            result = {'env': env, 'fds': sorted(fds), 'binary': binary,
                      'cpu': resource.getrlimit(resource.RLIMIT_CPU), 'memory': resource.getrlimit(resource.RLIMIT_AS),
                      'files': resource.getrlimit(resource.RLIMIT_FSIZE), 'core': resource.getrlimit(resource.RLIMIT_CORE),
                      'descriptorLimit': resource.getrlimit(resource.RLIMIT_NOFILE), 'cwd': os.getcwd()}
            os.write(1, json.dumps(result).encode())
            os._exit(0)
        fd, _ = executor.open_ripgrep()
        try:
            with mock.patch.object(executor.os, 'execve', side_effect=inspect_exec), \
                 mock.patch.dict(os.environ, {'OPENKODA_FIXTURE_SECRET': 'never-transfer'}):
                output, termination = executor.run_fixed(self.server, fd, 'version', b'', self.cancel_read)
            result = json.loads(output)
            self.assertEqual(termination, 'completed')
            self.assertEqual(result['env'], {'LANG': 'C', 'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'})
            self.assertEqual(result['fds'], sorted([0, 1, 2, fd]))
            self.assertEqual(result['cpu'], [2, 2])
            self.assertEqual(result['memory'], [134217728, 134217728])
            self.assertEqual(result['files'], [0, 0])
            self.assertEqual(result['core'], [0, 0])
            self.assertEqual(result['descriptorLimit'], [32, 32])
            self.assertEqual(result['cwd'], '/')
        finally:
            os.close(fd)

    def test_failed_pid_journal_never_authorizes_exec(self):
        marker = pathlib.Path(self.temp.name) / 'executed'
        def record_exec(*_):
            marker.write_text('unexpected')
            os._exit(0)
        fd, _ = executor.open_ripgrep()
        original = self.server.journal
        count = 0
        def fail_second():
            nonlocal count
            count += 1
            if count == 2:
                raise OSError('fixture journal failure')
            return original()
        try:
            with mock.patch.object(self.server, 'journal', side_effect=fail_second), mock.patch.object(executor.os, 'execve', side_effect=record_exec):
                with self.assertRaises(OSError):
                    executor.run_fixed(self.server, fd, 'scan', b'x', self.cancel_read)
            self.assertFalse(marker.exists())
            with self.assertRaises(ChildProcessError):
                os.waitpid(self.server.children[-1]['pid'], os.WNOHANG)
        finally:
            os.close(fd)

    def test_gate_eof_and_expired_launch_do_not_exec(self):
        for expired in (False, True):
            gate_read, gate_write = os.pipe()
            input_read, input_write = os.pipe()
            output_read, output_write = os.pipe()
            fd, _ = executor.open_ripgrep()
            parent = os.getpid()
            pid = os.fork()
            if pid == 0:
                try:
                    executor.child_exec(fd, 'version', input_read, output_write, gate_read, parent,
                                        int(time.time()) - 1 if expired else int(time.time()) + 8)
                except BaseException:
                    os._exit(125)
            if expired:
                os.write(gate_write, b'G')
            os.close(gate_write)
            for descriptor in [gate_read, input_read, input_write, output_write, fd]:
                os.close(descriptor)
            self.assertEqual(os.read(output_read, 4096), b'')
            os.close(output_read)
            _, status = os.waitpid(pid, 0)
            self.assertEqual(os.waitstatus_to_exitcode(status), 122)

    def test_parent_death_stops_worker_before_gate(self):
        # The wrapper exits after sending the child identity. The fixed child
        # must stop by PDEATHSIG, EOF or the getppid race check, never exec.
        receive, send = os.pipe()
        wrapper = os.fork()
        if wrapper == 0:
            try:
                os.close(receive)
                gate_read, gate_write = os.pipe()
                input_read, input_write = os.pipe()
                output_read, output_write = os.pipe()
                fd, _ = executor.open_ripgrep()
                parent = os.getpid()
                worker = os.fork()
                if worker == 0:
                    executor.child_exec(fd, 'version', input_read, output_write, gate_read, parent, int(time.time()) + 8)
                    os._exit(126)
                os.write(send, str(worker).encode() + b'\n')
                time.sleep(0.1)
                os._exit(0)
            except BaseException:
                os._exit(127)
        os.close(send)
        child_pid = int(os.read(receive, 64).strip())
        os.close(receive)
        _, status = os.waitpid(wrapper, 0)
        self.assertEqual(os.waitstatus_to_exitcode(status), 0)
        until = time.monotonic() + 3
        active = True
        while time.monotonic() < until:
            try:
                state = pathlib.Path('/proc/' + str(child_pid) + '/stat').read_text().rsplit(') ', 1)[1].split()[0]
                active = state != 'Z'
            except (FileNotFoundError, ProcessLookupError):
                active = False
            if not active:
                break
            time.sleep(0.01)
        self.assertFalse(active)


def bootstrap_envelope(source, identity=None):
    return {'version': 'openkoda.executor-bootstrap.v2', 'sourceZlibBase64': base64.b64encode(zlib.compress(source)).decode(),
            'sourceBytes': len(source), 'sourceSha256': hashlib.sha256(source).hexdigest(),
            **(identity or {key: str(uuid.uuid4()) for key in ('environmentId', 'sessionId', 'assessmentId')}),
            'mode': 'execute', 'capture': None, 'control': None}


class BootstrapEnvelopeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = pathlib.Path(__file__).with_name('kali-bootstrap.py').read_text()
        tree = ast.parse(source)
        # Load the actual unpacker without running its process bootstrap.
        tree.body = [node for node in tree.body if isinstance(node, (ast.Import, ast.FunctionDef))]
        namespace = {}
        exec(compile(tree, 'bootstrap-fixture', 'exec'), namespace)
        cls.unpack = staticmethod(namespace['unpack_bootstrap'])

    def read(self, value):
        return self.unpack(json.dumps(value).encode() + b'\n')

    def test_complete_embedded_source_roundtrips_with_bounded_wire(self):
        source = pathlib.Path(executor.__file__).read_bytes()
        value = bootstrap_envelope(source)
        raw = json.dumps(value).encode() + b'\n'
        self.assertLessEqual(len(raw), 65536)
        self.assertLessEqual(len(source), 128 * 1024)
        self.assertEqual(self.unpack(raw)[1].encode(), source)

    def test_version_fields_size_hash_and_utf8_fail_closed(self):
        value = bootstrap_envelope(b'known embedded source')
        cases = [{**value, 'version': 'old'}, {**value, 'source': 'injected'}, {**value, 'sourceBytes': True},
                 {**value, 'sourceBytes': 128 * 1024 + 1}, {**value, 'sourceBytes': len(b'known embedded source') - 1},
                 {**value, 'sourceSha256': 'a' * 64}, bootstrap_envelope(b'\xff')]
        for case in cases:
            with self.subTest(fields=list(case)), self.assertRaises((ValueError, UnicodeError, zlib.error)):
                self.read(case)

    def test_bomb_truncated_trailing_stream_and_duplicate_are_rejected(self):
        value = bootstrap_envelope(b'a' * (1024 * 1024))
        value['sourceBytes'] = 128
        with self.assertRaises(ValueError):
            self.read(value)
        value = bootstrap_envelope(b'fixture')
        compressed = base64.b64decode(value['sourceZlibBase64'])
        for encoded in (compressed[:-1], compressed + b'trailing', compressed + zlib.compress(b'second')):
            with self.assertRaises(ValueError):
                self.read({**value, 'sourceZlibBase64': base64.b64encode(encoded).decode()})
        with self.assertRaises(ValueError):
            self.unpack(b'{"version":1,"version":2}\n')

    def test_control_sessions_cannot_reach_any_producer(self):
        runtime = {'executorSourceSha256': 'a' * 64}
        ids = [str(uuid.uuid4()) for _ in range(5)]
        server = executor.Server(*ids[:3], runtime, mode='collect', control={'producerSessionId': ids[3], 'captureId': ids[4], 'ownerProfileFingerprint': 'd' * 64, 'producerDispatchDeadline': 1})
        with mock.patch.object(executor, 'open_ripgrep') as binary, mock.patch.object(executor, 'run_fixed') as child:
            for tool in ('runtime_diagnostics', 'hash_bytes', 'scan_bytes', 'capture_purge'):
                with self.assertRaises(executor.ProtocolError):
                    server.call(tool, {})
            with self.assertRaises(executor.ProtocolError):
                server.scan(b'a', hashlib.sha256(b'a').hexdigest())
        binary.assert_not_called()
        child.assert_not_called()


@unittest.skipUnless(sys.platform == 'linux', 'Owned dir_fd/fsync fixtures require Linux; no producer or target executes')
class LinuxCaptureSpoolTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='openkoda-capture-fixture-')
        self.directory = pathlib.Path(self.temp.name)
        self.binding = {'schema': 'openkoda.kali-spool-binding.v1', 'captureId': str(uuid.uuid4()), 'attemptId': str(uuid.uuid4()),
                        'inputEvidenceId': str(uuid.uuid4()), 'inputSha256': 'a' * 64, 'maxBytes': 65536, 'ownerProfileFingerprint': 'd' * 64}
        self.server = executor.Server(str(uuid.uuid4()), str(uuid.uuid4()), str(uuid.uuid4()),
                                      {'executorSourceSha256': 'b' * 64}, self.directory, capture_binding=self.binding)
        self.server.journal()
        self.writer = executor.SpoolWriter(self.server, 'c' * 64)
        self.addCleanup(self.temp.cleanup)
        self.addCleanup(self.writer.close)

    def control(self, mode='collect', extra=None):
        return executor.Server(self.server.environment_id, str(uuid.uuid4()), self.server.assessment_id,
                               self.server.runtime, self.directory, mode=mode,
                               control={'producerSessionId': self.server.session_id, 'captureId': self.binding['captureId'], 'ownerProfileFingerprint': self.binding['ownerProfileFingerprint'], 'producerDispatchDeadline': 1,
                                        **({'previousReceiptId': None, 'previousReceiptDigest': None} if mode == 'purge' else {}), **(extra or {})})

    def call(self, server, args):
        with mock.patch.object(executor, 'inspect_session', return_value={'known': True, 'active': False}):
            return server.call('capture_collect' if server.mode == 'collect' else 'capture_purge', {**{k: v for k, v in server.control.items() if k not in ('ownerProfileFingerprint', 'producerDispatchDeadline')}, **args})

    def test_durable_prefix_precedes_memory_and_survives_unacknowledged_tail(self):
        self.writer.append(b'prefix')
        manifest = executor.spool_manifest(self.writer.fd)
        os.write(self.writer.output, b'uncommitted')
        os.fsync(self.writer.output)
        result = self.call(self.control(), {'kind': 'describe'})
        self.assertEqual(result['manifest'], manifest)
        page = self.call(self.control(), {'kind': 'read', 'expectedManifestDigest': manifest['digest'], 'offset': 0, 'maxBytes': 4096})
        self.assertEqual(base64.b64decode(page['base64']), b'prefix')
        self.assertEqual(os.pread(self.writer.output, 64, 0), b'prefixuncommitted')

    def test_manifest_loss_does_not_acknowledge_or_parse_extra_bytes(self):
        self.writer.append(b'prefix')
        with mock.patch.object(self.writer, 'update', side_effect=OSError('fixture disk failure')):
            with self.assertRaises(OSError):
                self.writer.append(b'tail')
        self.assertEqual(executor.capture_prefix(self.writer.fd, executor.spool_manifest(self.writer.fd)), b'prefix')

    def test_invalid_utf8_and_output_limit_are_preserved_before_parser(self):
        self.writer.append(b'\xff')
        self.writer.captured('output_limited', False)
        self.writer.seal()
        body = executor.spool_manifest(self.writer.fd)['body']
        self.assertEqual((body['encoding'], body['lossReason'], body['observedBytes']), ('unknown', 'output_limit', None))
        with self.assertRaises(executor.ProtocolError):
            self.writer.append(b'later')

    def test_live_producer_wrong_digest_and_range_are_denied(self):
        server = self.control()
        with self.assertRaises(executor.ProtocolError):
            server.call('capture_collect', {'producerSessionId': self.server.session_id, 'captureId': self.binding['captureId'], 'kind': 'describe'})
        self.writer.append(b'prefix')
        manifest = executor.spool_manifest(self.writer.fd)
        for changes in ({'expectedManifestDigest': 'd' * 64}, {'offset': True}, {'offset': 7}, {'maxBytes': 4097}):
            with self.assertRaises(executor.ProtocolError):
                self.call(server, {'kind': 'read', 'expectedManifestDigest': manifest['digest'], 'offset': 0, 'maxBytes': 4096, **changes})

    def test_purge_is_owned_idempotent_and_retains_minimal_receipt(self):
        self.writer.append(b'prefix')
        self.writer.captured('completed', True)
        self.writer.seal()
        manifest = executor.spool_manifest(self.writer.fd)
        server = self.control('purge', {'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': str(uuid.uuid4())})
        first = self.call(server, {})
        self.assertEqual(first['receipt']['state'], 'purged')
        self.assertEqual(self.call(server, {}), first)
        with self.assertRaises(FileNotFoundError):
            executor.read_owned(self.writer.fd, 'stdout.bin', 65536)
        self.assertEqual(executor.spool_manifest(self.writer.fd), manifest)
        inspected = self.call(self.control(), {'kind': 'describe'})
        self.assertEqual(inspected['purgeReceipt']['state'], 'purged')
        with self.assertRaises(executor.ProtocolError):
            self.call(self.control(), {'kind': 'read', 'expectedManifestDigest': manifest['digest'], 'offset': 0, 'maxBytes': 4096})

    def test_partial_purge_replay_is_readonly_and_new_plan_requires_exact_predecessor(self):
        self.writer.append(b'prefix')
        self.writer.captured('completed', True)
        self.writer.seal()
        manifest = executor.spool_manifest(self.writer.fd)
        original_id = str(uuid.uuid4())
        original = self.control('purge', {'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': original_id})
        unlink = executor.os.unlink
        def fail_raw(name, *args, **kwargs):
            if name == 'stdout.bin':
                raise OSError('synthetic deletion interruption')
            return unlink(name, *args, **kwargs)
        with mock.patch.object(executor.os, 'unlink', side_effect=fail_raw):
            with self.assertRaises(OSError):
                self.call(original, {})
        before = self.call(self.control(), {'kind': 'describe'})['purgeReceipt']
        self.assertEqual(before['state'], 'intent')
        self.assertEqual(self.call(original, {})['receipt'], before)
        self.assertEqual(executor.read_owned(self.writer.fd, 'stdout.bin', 65536), b'prefix')
        next_id = str(uuid.uuid4())
        args = {'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': next_id,
                'previousReceiptId': original_id, 'previousReceiptDigest': hashlib.sha256(executor.canonical(before)).hexdigest()}
        with self.assertRaises(executor.ProtocolError):
            self.call(self.control('purge', {**args, 'previousReceiptDigest': 'e' * 64}), {})
        complete = self.call(self.control('purge', args), {})['receipt']
        self.assertEqual(complete['state'], 'purged')
        self.assertEqual(complete['previousReceiptId'], original_id)
        with self.assertRaises(executor.ProtocolError):
            self.call(original, {})

    def test_partial_retention_refuses_changed_prefix_but_accepts_already_unlinked_raw(self):
        self.writer.append(b'prefix')
        self.writer.captured('completed', True)
        self.writer.seal()
        manifest = executor.spool_manifest(self.writer.fd)
        original_id = str(uuid.uuid4())
        original = self.control('purge', {'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': original_id})
        with mock.patch.object(executor.os, 'unlink', side_effect=OSError('synthetic interruption')):
            with self.assertRaises(OSError):
                self.call(original, {})
        receipt = executor.purge_record(self.writer.fd, manifest)
        next_args = {'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': str(uuid.uuid4()),
                     'previousReceiptId': original_id, 'previousReceiptDigest': hashlib.sha256(executor.canonical(receipt)).hexdigest()}
        os.lseek(self.writer.output, 0, os.SEEK_SET)
        os.write(self.writer.output, b'CHANGE')
        os.fsync(self.writer.output)
        with mock.patch.object(executor.os, 'unlink') as unlink:
            for server, args in [(self.control(), {'kind': 'describe'}), (original, {}), (self.control('purge', next_args), {})]:
                with self.assertRaises(executor.ProtocolError):
                    self.call(server, args)
            unlink.assert_not_called()
        self.assertEqual(executor.read_owned(self.writer.fd, 'stdout.bin', 65536), b'CHANGE')
        self.assertEqual(executor.purge_record(self.writer.fd, manifest), receipt)
        # Simulate the other crash boundary: raw was removed, final receipt lost.
        os.unlink('stdout.bin', dir_fd=self.writer.fd)
        self.assertEqual(self.call(self.control(), {'kind': 'describe'})['purgeReceipt'], receipt)
        final = self.call(self.control('purge', next_args), {})
        self.assertEqual(final['receipt']['state'], 'purged')

    def test_purged_receipt_cannot_hide_reappeared_raw(self):
        self.writer.append(b'prefix')
        self.writer.captured('completed', True)
        self.writer.seal()
        manifest = executor.spool_manifest(self.writer.fd)
        server = self.control('purge', {'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': str(uuid.uuid4())})
        self.assertEqual(self.call(server, {})['receipt']['state'], 'purged')
        descriptor = os.open('stdout.bin', os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600, dir_fd=self.writer.fd)
        try:
            os.write(descriptor, b'prefix')
        finally:
            os.close(descriptor)
        with mock.patch.object(executor.os, 'unlink') as unlink:
            for control, args in [(self.control(), {'kind': 'describe'}), (server, {})]:
                with self.assertRaises(executor.ProtocolError):
                    self.call(control, args)
            unlink.assert_not_called()

    def test_links_fifo_unknown_files_and_quota_refuse_without_new_scan(self):
        os.symlink('/dev/null', self.directory / 'captures-v1' / self.binding['captureId'] / 'foreign')
        with self.assertRaises(executor.ProtocolError):
            executor.spool_quota(self.writer.root)
        path = self.directory / 'captures-v1' / self.binding['captureId'] / 'pipe'
        os.mkfifo(path, 0o600)
        with self.assertRaises(executor.ProtocolError):
            executor.read_owned(self.writer.fd, 'pipe', 1)
        with mock.patch.object(executor, 'SPOOL_LIMIT', 65536):
            with self.assertRaises(executor.ProtocolError):
                executor.spool_quota(self.writer.root)

    def test_absence_requires_missing_directory_not_inconsistent_metadata(self):
        capture = self.directory / 'captures-v1' / self.binding['captureId']
        (capture / 'manifest.json').unlink()
        with self.assertRaises((FileNotFoundError, executor.ProtocolError)):
            self.call(self.control(), {'kind': 'describe'})
        self.writer.close()
        (capture / 'stdout.bin').unlink()
        capture.rmdir()
        absent = self.call(self.control(), {'kind': 'describe'})
        self.assertEqual(absent['availability'], 'absent')
        self.assertIsNone(absent['manifest'])
        self.assertEqual(absent['absence']['body']['captureId'], self.binding['captureId'])
        self.assertEqual(absent['absence']['digest'], hashlib.sha256(executor.canonical(absent['absence']['body'])).hexdigest())

    def test_ancestor_links_and_uncommitted_tail_cannot_escape_or_undercharge(self):
        alias = self.directory / 'alias'
        alias.symlink_to(self.directory, target_is_directory=True)
        with self.assertRaises(OSError):
            executor.capture_root(alias)
        self.writer.append(b'prefix')
        os.write(self.writer.output, b'uncommitted')
        os.fsync(self.writer.output)
        self.writer.seal('interrupted')
        with mock.patch.object(executor, 'SPOOL_LIMIT', 65536 + len(b'prefix')):
            with self.assertRaises(executor.ProtocolError):
                executor.spool_quota(self.writer.root)


class RetireContractTests(unittest.TestCase):
    def control(self, kind='inspect'):
        value = {'kind': kind, 'producerSessionId': str(uuid.uuid4()), 'captureId': str(uuid.uuid4()),
                 'ownerProfileFingerprint': 'a' * 64, 'producerDispatchDeadline': 1,
                 'expectedManifestDigest': 'b' * 64, 'purgeReceiptId': str(uuid.uuid4()),
                 'expectedPurgeReceiptSha256': 'c' * 64, 'pendingAttempt': None}
        if kind == 'apply':
            value.update(retireReceiptId=str(uuid.uuid4()), previousReceiptId=None,
                         previousReceiptDigest=None, expectedStateDigest='d' * 64)
        return value

    def test_retire_catalog_version_keeps_exact_v2_hash(self):
        self.assertEqual(executor.catalog_sha256(executor.LEGACY_CATALOG_VERSION),
                         '26907e2c27bf7c28b9a30f6be1e6243f7d1dde346c6706e136d1f112a3567e7e')
        self.assertEqual(executor.CATALOG_VERSION, 'openkoda.kali-tools.v3')
        schemas = executor.retire_schema()['oneOf']
        self.assertEqual([item['properties']['kind']['const'] for item in schemas], ['inspect', 'apply'])
        self.assertTrue(all(set(item['properties']) == set(item['required']) and item['additionalProperties'] is False for item in schemas))

    def test_retire_session_cannot_reach_producer_or_other_control(self):
        ids = [str(uuid.uuid4()) for _ in range(3)]
        control = self.control()
        server = executor.Server(*ids, {}, mode='retire', control=control)
        with mock.patch.object(executor, 'retire_control') as retire, mock.patch.object(executor, 'run_fixed') as child:
            for tool in ('runtime_diagnostics', 'hash_bytes', 'scan_bytes', 'capture_collect', 'capture_purge'):
                with self.assertRaises(executor.ProtocolError):
                    server.call(tool, {})
            retire.assert_not_called()
            child.assert_not_called()
        for mode in ('execute', 'inspect'):
            other = executor.Server(*ids, {}, mode=mode)
            with self.assertRaises(executor.ProtocolError):
                other.call('capture_retire', control)

    def test_retire_control_rejects_extra_fields_wrong_kinds_pairs_and_hashes(self):
        original = self.control('apply')
        invalid = [{'path': '/tmp/arbitrary'}, {'kind': 'inspect'}, {'expectedStateDigest': 'x'},
                   {'producerDispatchDeadline': True}, {'retireReceiptId': original['purgeReceiptId']},
                   {'previousReceiptId': str(uuid.uuid4())}, {'expectedPurgeReceiptSha256': None},
                   {'pendingAttempt': {'retireReceiptId': str(uuid.uuid4())}}]
        for delta in invalid:
            with self.subTest(delta=delta), self.assertRaises((ValueError, executor.ProtocolError)):
                executor.validate_retire_control({**original, **delta})
        missing = dict(original)
        del missing['pendingAttempt']
        with self.assertRaises(executor.ProtocolError):
            executor.validate_retire_control(missing)

    def test_commitment_bounded_and_malformed_field_blocks_old_boot_shortcut(self):
        ids = [str(uuid.uuid4()) for _ in range(7)]
        proof = {'captureId': ids[0], 'attemptId': ids[1], 'ownerProfileFingerprint': 'a' * 64,
                 'manifestDigest': 'b' * 64, 'purgeReceiptId': ids[2], 'purgeReceiptSha256': 'c' * 64}
        attempt = {'retireReceiptId': ids[3], 'previousReceiptId': ids[4],
                   'previousReceiptDigest': 'd' * 64, 'reviewedStateDigest': 'e' * 64}
        commitment = executor.retirement_commitment(proof, attempt)
        self.assertLessEqual(len(executor.canonical(commitment)), 1024)
        record = {'environmentId': ids[5], 'sessionId': ids[6], 'assessmentId': str(uuid.uuid4()),
                  'pid': 2147483647, 'startTicks': '9' * 32, 'bootId': str(uuid.uuid4()), 'state': 'closed',
                  'capture': {'captureId': ids[0], 'attemptId': ids[1]},
                  'children': [{'state': 'reaped', 'pid': 2147483647, 'startTicks': '9' * 32,
                                'launchDeadline': 9007199254740991}] * 2, 'retiredCapture': commitment}
        self.assertLessEqual(len(executor.canonical(record)), 4096)
        executor.validate_session_record(executor.canonical(record), ids[5], ids[6])
        for mutation in ({**commitment, 'extra': True}, {**commitment, 'proof': {**proof, 'attemptId': str(uuid.uuid4())}},
                         {**commitment, 'attempt': {**attempt, 'retireReceiptId': proof['purgeReceiptId']}}):
            with self.subTest(mutation=mutation), mock.patch.object(executor, 'process_start') as probe:
                with self.assertRaises((ValueError, executor.ProtocolError)):
                    executor.validate_session_record(executor.canonical({**record, 'retiredCapture': mutation}), ids[5], ids[6])
                probe.assert_not_called()

    def test_retire_arguments_must_equal_bootstrap_before_filesystem(self):
        control = self.control('apply')
        server = executor.Server(str(uuid.uuid4()), str(uuid.uuid4()), str(uuid.uuid4()), {},
                                 pathlib.Path('unused'), mode='retire', control=control)
        changed = {**control, 'expectedStateDigest': 'f' * 64}
        with mock.patch.object(executor, 'retirement_handles') as filesystem:
            with self.assertRaises(executor.ProtocolError):
                server.call('capture_retire', changed)
            filesystem.assert_not_called()


@unittest.skipUnless(sys.platform == 'linux', 'Metadata retention requires real Linux descriptor/lock/fsync semantics')
class LinuxRetireMetadataTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='openkoda-retire-fixture-')
        self.directory = pathlib.Path(self.temp.name)
        self.directory.chmod(0o700)
        self.addCleanup(self.temp.cleanup)
        self.environment = str(uuid.uuid4())
        self.assessment = str(uuid.uuid4())
        self.runtime = {'executorSourceSha256': 'b' * 64}
        self.server, self.writer, self.binding = self.new_producer()
        self.manifest, self.purge, self.base = self.terminal(self.server, self.writer, self.binding)

    def new_producer(self):
        binding = {'schema': 'openkoda.kali-spool-binding.v1', 'captureId': str(uuid.uuid4()),
                   'attemptId': str(uuid.uuid4()), 'inputEvidenceId': str(uuid.uuid4()), 'inputSha256': 'a' * 64,
                   'maxBytes': 65536, 'ownerProfileFingerprint': 'd' * 64}
        server = executor.Server(self.environment, str(uuid.uuid4()), self.assessment,
                                 self.runtime, self.directory, capture_binding=binding)
        server.journal()
        writer = executor.SpoolWriter(server, 'c' * 64)
        self.addCleanup(writer.close)
        return server, writer, binding

    def terminal(self, server, writer, binding, legacy_catalog=False, legacy_receipt=False):
        writer.append(b'synthetic prefix')
        writer.captured('completed', True)
        writer.seal()
        if legacy_catalog:
            writer.commit({**writer.body, 'catalogVersion': executor.LEGACY_CATALOG_VERSION,
                           'catalogSha256': executor.catalog_sha256(executor.LEGACY_CATALOG_VERSION)})
        manifest = executor.spool_manifest(writer.fd)
        control = {'producerSessionId': server.session_id, 'captureId': binding['captureId'],
                   'ownerProfileFingerprint': binding['ownerProfileFingerprint'], 'producerDispatchDeadline': 1,
                   'expectedManifestDigest': manifest['digest'], 'purgeReceiptId': str(uuid.uuid4()),
                   'previousReceiptId': None, 'previousReceiptDigest': None}
        purge_server = executor.Server(self.environment, str(uuid.uuid4()), self.assessment, self.runtime,
                                      self.directory, mode='purge', control=control)
        args = {key: value for key, value in control.items() if key not in ('ownerProfileFingerprint', 'producerDispatchDeadline')}
        with mock.patch.object(executor, 'inspect_session', return_value={'known': True, 'active': False}):
            receipt = purge_server.call('capture_purge', args)['receipt']
        if legacy_receipt:
            receipt = {key: value for key, value in receipt.items() if key not in ('previousReceiptId', 'previousReceiptDigest')}
            receipt['schema'] = 'openkoda.kali-spool-purge.v1'
            executor.atomic_owned(writer.fd, 'purge.json', receipt)
        base = {key: value for key, value in control.items() if key not in ('previousReceiptId', 'previousReceiptDigest')}
        base.update(kind='inspect', expectedPurgeReceiptSha256=hashlib.sha256(executor.canonical(receipt)).hexdigest(), pendingAttempt=None)
        return manifest, receipt, base

    def invoke(self, control=None, *, real_quiescence=False):
        bound = self.base if control is None else control
        server = executor.Server(self.environment, str(uuid.uuid4()), self.assessment, self.runtime,
                                 self.directory, mode='retire', control=bound)
        if real_quiescence:
            return server.call('capture_retire', dict(bound))
        with mock.patch.object(executor, 'inspect_session', return_value={'known': True, 'active': False}):
            return server.call('capture_retire', dict(bound))

    def application(self, inspected=None, base=None):
        base = self.base if base is None else base
        inspected = self.invoke(base) if inspected is None else inspected
        commitment = inspected['observation']['commitment']
        return {**base, 'kind': 'apply', 'retireReceiptId': str(uuid.uuid4()),
                'expectedPurgeReceiptSha256': inspected['observation']['purgeReceiptSha256'],
                'previousReceiptId': commitment['attempt']['retireReceiptId'] if commitment else None,
                'previousReceiptDigest': hashlib.sha256(executor.canonical(commitment)).hexdigest() if commitment else None,
                'expectedStateDigest': inspected['stateDigest']}

    def assert_readonly(self, call):
        with mock.patch.object(executor.os, 'unlink') as unlink, mock.patch.object(executor.os, 'replace') as replace, \
             mock.patch.object(executor.os, 'rmdir') as rmdir, mock.patch.object(executor, 'atomic_owned') as write:
            try:
                return call()
            finally:
                for operation in (unlink, replace, rmdir, write):
                    operation.assert_not_called()

    def test_terminal_metadata_retire_preserves_original_proof_and_replay_is_readonly(self):
        before = self.assert_readonly(lambda: self.invoke({**self.base, 'expectedPurgeReceiptSha256': None}))
        self.assertEqual(before['observation']['state'], 'ready')
        self.assertEqual(before['linuxPurgeProof']['receipt'], self.purge)
        self.assertEqual(before['stateDigest'], self.invoke()['stateDigest'])
        apply = self.application(before)
        after = self.invoke(apply)
        self.assertEqual(after['observation']['state'], 'retired')
        self.assertFalse(after['observation']['slotPresent'])
        self.assertEqual(after['observation']['entries'], [])
        self.assertEqual(after['observation']['metadataBytes'], 0)
        self.assertIsNone(after['linuxPurgeProof'])
        self.assertLess(len(executor.canonical(after)), 8192)
        replay = self.assert_readonly(lambda: self.invoke(apply))
        self.assertNotEqual(replay['sessionId'], after['sessionId'])
        self.assertEqual({key: value for key, value in replay.items() if key != 'sessionId'},
                         {key: value for key, value in after.items() if key != 'sessionId'})
        self.assert_readonly(lambda: self.invoke())
        record = json.loads((self.directory / (self.server.session_id + '.json')).read_text())
        self.assertEqual(record['pid'], os.getpid())
        self.assertEqual(record['capture'], {'captureId': self.binding['captureId'], 'attemptId': self.binding['attemptId']})
        self.assertEqual(record['retiredCapture']['proof']['purgeReceiptSha256'], self.base['expectedPurgeReceiptSha256'])

    def test_legacy_catalog_v2_and_purge_v1_and_v2_are_retirable(self):
        for version1 in (False, True):
            server, writer, binding = self.new_producer()
            manifest, receipt, base = self.terminal(server, writer, binding, True, version1)
            observed = self.invoke({**base, 'expectedPurgeReceiptSha256': None})
            self.assertEqual(observed['linuxPurgeProof']['receipt'], receipt)
            self.assertEqual(manifest['body']['catalogVersion'], executor.LEGACY_CATALOG_VERSION)
            self.assertEqual(self.invoke(self.application(observed, base))['observation']['state'], 'retired')

    def test_256_terminal_entries_block_admission_then_retire_releases_one_slot(self):
        for _ in range(255):
            server, writer, binding = self.new_producer()
            self.terminal(server, writer, binding)
            writer.close()
        with self.assertRaises(executor.ProtocolError):
            self.new_producer()
        self.invoke(self.application())
        admitted = self.new_producer()
        with self.assertRaises(executor.ProtocolError):
            self.new_producer()
        admitted[1].close()
        self.assertEqual(len([item for item in (self.directory / 'captures-v1').iterdir() if item.is_dir()]), 256)

    def test_more_than_256_capture_purge_retire_cycles_keep_spool_capacity(self):
        self.invoke(self.application())
        for _ in range(260):
            server, writer, binding = self.new_producer()
            _, _, base = self.terminal(server, writer, binding)
            self.assertEqual(self.invoke(self.application(base=base))['observation']['state'], 'retired')
            writer.close()
        root = executor.capture_root(self.directory)
        try:
            executor.spool_quota(root)
        finally:
            os.close(root)
        self.assertEqual([entry.name for entry in (self.directory / 'captures-v1').iterdir()], ['quota.lock'])
        self.assertEqual(len(list(self.directory.glob('*.json'))), 261)

    def test_each_metadata_unlink_interruption_needs_new_exact_predecessor(self):
        for stop_name in ('manifest.json', 'purge.json'):
            server, writer, binding = self.new_producer()
            _, _, base = self.terminal(server, writer, binding)
            apply = self.application(base=base)
            original_unlink = executor.os.unlink
            def interrupted(name, *args, **kwargs):
                original_unlink(name, *args, **kwargs)
                if name == stop_name:
                    raise OSError('synthetic failure after metadata unlink')
            with mock.patch.object(executor.os, 'unlink', side_effect=interrupted), self.assertRaises(OSError):
                self.invoke(apply)
            partial = self.assert_readonly(lambda: self.invoke(base))
            self.assertEqual(partial['observation']['state'], 'metadata_remaining')
            self.assert_readonly(lambda: self.invoke(apply))
            next_apply = self.application(partial, base)
            for bad in ({'previousReceiptId': None, 'previousReceiptDigest': None}, {'previousReceiptDigest': 'f' * 64}):
                with self.assertRaises(executor.ProtocolError):
                    self.assert_readonly(lambda: self.invoke({**next_apply, **bad}))
            self.assertEqual(self.invoke(next_apply)['observation']['state'], 'retired')
            with self.assertRaises(executor.ProtocolError):
                self.assert_readonly(lambda: self.invoke(apply))

    def test_interrupted_partial_and_complete_producer_write_require_known_pending_intent(self):
        for full in (False, True):
            server, writer, binding = self.new_producer()
            _, _, base = self.terminal(server, writer, binding)
            before = self.invoke(base)
            apply = self.application(before, base)
            original_write = executor.os.write
            def fail_write(fd, data):
                original_write(fd, data if full else data[:37])
                raise OSError('synthetic producer journal interruption')
            with mock.patch.object(executor.os, 'write', side_effect=fail_write), self.assertRaises(OSError):
                self.invoke(apply)
            with self.assertRaises(executor.ProtocolError):
                self.assert_readonly(lambda: self.invoke(base))
            pending = {'retireReceiptId': apply['retireReceiptId'], 'previousReceiptId': None,
                       'previousReceiptDigest': None, 'reviewedStateDigest': apply['expectedStateDigest'],
                       'producerRecordSha256': before['observation']['producerRecordSha256']}
            inspected_base = {**base, 'pendingAttempt': pending}
            partial = self.assert_readonly(lambda: self.invoke(inspected_base))
            self.assertEqual(partial['observation']['pendingProducerWrite']['complete'], full)
            self.assert_readonly(lambda: self.invoke({**apply, 'pendingAttempt': pending}))
            self.assertEqual(self.invoke(self.application(partial, inspected_base))['observation']['state'], 'retired')

    def test_unknown_or_divergent_producer_temporary_is_preserved(self):
        observation = self.invoke()
        name = self.directory / (self.server.session_id + '.json.next')
        name.write_bytes(b'unknown')
        name.chmod(0o600)
        pending = {'retireReceiptId': str(uuid.uuid4()), 'previousReceiptId': None, 'previousReceiptDigest': None,
                   'reviewedStateDigest': observation['stateDigest'],
                   'producerRecordSha256': observation['observation']['producerRecordSha256']}
        for control in (self.base, {**self.base, 'pendingAttempt': pending},
                        {**self.base, 'pendingAttempt': {**pending, 'producerRecordSha256': 'f' * 64}}):
            with self.assertRaises(executor.ProtocolError):
                self.assert_readonly(lambda: self.invoke(control))
        self.assertEqual(name.read_bytes(), b'unknown')

    def test_every_sync_boundary_preserves_honest_recovery_and_never_replays_removal(self):
        for boundary in range(1, 8):
            with self.subTest(boundary=boundary):
                server, writer, binding = self.new_producer()
                _, _, base = self.terminal(server, writer, binding)
                observed = self.invoke(base)
                apply = self.application(observed, base)
                original_sync = executor.os.fsync
                calls = 0
                def interrupt_sync(fd):
                    nonlocal calls
                    calls += 1
                    if calls == boundary:
                        raise OSError('synthetic fsync boundary ' + str(boundary))
                    return original_sync(fd)
                with mock.patch.object(executor.os, 'fsync', side_effect=interrupt_sync), self.assertRaises(OSError):
                    self.invoke(apply)
                inspect = base
                if boundary == 1:
                    inspect = {**base, 'pendingAttempt': {'retireReceiptId': apply['retireReceiptId'],
                        'previousReceiptId': None, 'previousReceiptDigest': None,
                        'reviewedStateDigest': apply['expectedStateDigest'],
                        'producerRecordSha256': observed['observation']['producerRecordSha256']}}
                recovered = self.assert_readonly(lambda: self.invoke(inspect))
                if boundary <= 3:
                    self.assertTrue(recovered['observation']['slotPresent'])
                    self.assert_readonly(lambda: self.invoke({**apply, 'pendingAttempt': inspect['pendingAttempt']}))
                    self.assertEqual(self.invoke(self.application(recovered, inspect))['observation']['state'], 'retired')
                else:
                    self.assertEqual(recovered['observation']['state'], 'retired')
                    self.assert_readonly(lambda: self.invoke(apply))

    def test_failure_before_temporary_creation_allows_only_new_reviewed_attempt(self):
        before = self.invoke()
        apply = self.application(before)
        with mock.patch.object(executor, 'atomic_owned', side_effect=OSError('failure before temporary')), self.assertRaises(OSError):
            self.invoke(apply)
        self.assertFalse((self.directory / (self.server.session_id + '.json.next')).exists())
        unchanged = self.assert_readonly(lambda: self.invoke())
        self.assertEqual(unchanged['stateDigest'], before['stateDigest'])
        next_apply = self.application(unchanged)
        self.assertNotEqual(next_apply['retireReceiptId'], apply['retireReceiptId'])
        self.assertEqual(self.invoke(next_apply)['observation']['state'], 'retired')

    def test_commit_replace_and_parent_sync_failure_never_unlink_metadata(self):
        original_replace = executor.os.replace
        def replaced_then_fail(*args, **kwargs):
            original_replace(*args, **kwargs)
            raise OSError('synthetic post-replace failure')
        apply = self.application()
        with mock.patch.object(executor.os, 'replace', side_effect=replaced_then_fail), mock.patch.object(executor.os, 'unlink') as unlink:
            with self.assertRaises(OSError):
                self.invoke(apply)
            unlink.assert_not_called()
        partial = self.assert_readonly(lambda: self.invoke())
        self.assertEqual(partial['observation']['state'], 'metadata_remaining')
        self.assertEqual(len(partial['observation']['entries']), 2)
        self.assert_readonly(lambda: self.invoke(apply))
        self.assertEqual(self.invoke(self.application(partial))['observation']['state'], 'retired')

    def test_rmdir_failure_keeps_empty_directory_until_new_confirmation(self):
        apply = self.application()
        with mock.patch.object(executor.os, 'rmdir', side_effect=OSError('synthetic rmdir failure')), self.assertRaises(OSError):
            self.invoke(apply)
        partial = self.assert_readonly(lambda: self.invoke())
        self.assertEqual(partial['observation']['state'], 'metadata_remaining')
        self.assertEqual(partial['observation']['entries'], [])
        self.assert_readonly(lambda: self.invoke(apply))
        self.assertEqual(self.invoke(self.application(partial))['observation']['state'], 'retired')

    def test_retired_inspection_fsync_failure_does_not_report_success(self):
        self.invoke(self.application())
        with mock.patch.object(executor.os, 'fsync', side_effect=OSError('synthetic durability failure')):
            with self.assertRaises(OSError):
                self.assert_readonly(lambda: self.invoke())
        self.assertEqual(self.assert_readonly(lambda: self.invoke())['observation']['state'], 'retired')

    def test_live_producer_wrong_owner_purge_manifest_and_attempt_are_refused(self):
        with self.assertRaises(executor.ProtocolError):
            self.invoke(real_quiescence=True)
        for delta in ({'ownerProfileFingerprint': 'e' * 64}, {'expectedManifestDigest': 'f' * 64},
                      {'purgeReceiptId': str(uuid.uuid4())}, {'expectedPurgeReceiptSha256': 'a' * 64}):
            with self.subTest(delta=delta), self.assertRaises(executor.ProtocolError):
                self.assert_readonly(lambda: self.invoke({**self.base, **delta}))
        journal = self.directory / (self.server.session_id + '.json')
        record = json.loads(journal.read_text())
        record['capture']['attemptId'] = str(uuid.uuid4())
        journal.write_bytes(executor.canonical(record))
        with self.assertRaises(executor.ProtocolError):
            self.assert_readonly(lambda: self.invoke())

    def test_raw_reappearance_unknown_file_and_changed_manifest_are_preserved(self):
        directory = self.directory / 'captures-v1' / self.binding['captureId']
        for name in ('stdout.bin', 'foreign'):
            target = directory / name
            target.write_bytes(b'preserve')
            target.chmod(0o600)
            with self.assertRaises(executor.ProtocolError):
                self.assert_readonly(lambda: self.invoke())
            self.assertEqual(target.read_bytes(), b'preserve')
            target.unlink()
        manifest = directory / 'manifest.json'
        manifest.write_bytes(b'{}')
        with self.assertRaises(executor.ProtocolError):
            self.assert_readonly(lambda: self.invoke())
        self.assertEqual(manifest.read_bytes(), b'{}')

    def test_symlink_hardlink_fifo_and_unowned_temporary_are_never_removed(self):
        directory = self.directory / 'captures-v1' / self.binding['captureId']
        original = directory / 'manifest.json'
        temporary = directory / 'manifest.json.next'
        actions = [lambda: os.symlink(original, temporary), lambda: os.link(original, temporary),
                   lambda: os.mkfifo(temporary, 0o600)]
        for action in actions:
            action()
            with self.assertRaises((OSError, executor.ProtocolError)):
                self.assert_readonly(lambda: self.invoke())
            self.assertTrue(temporary.exists())
            temporary.unlink()

    def test_replaced_file_after_commit_is_preserved(self):
        apply = self.application()
        original = executor.exact_metadata_entry
        directory = self.directory / 'captures-v1' / self.binding['captureId']
        replaced = False
        def swap_before_check(parent, entry):
            nonlocal replaced
            if entry['name'] == 'manifest.json' and not replaced:
                replacement = directory / 'replacement'
                replacement.write_bytes((directory / 'manifest.json').read_bytes())
                replacement.chmod(0o600)
                os.replace(replacement, directory / 'manifest.json')
                replaced = True
            return original(parent, entry)
        with mock.patch.object(executor, 'exact_metadata_entry', side_effect=swap_before_check), mock.patch.object(executor.os, 'unlink') as unlink:
            with self.assertRaises(executor.ProtocolError):
                self.invoke(apply)
            unlink.assert_not_called()
        self.assertTrue(replaced)
        self.assertTrue((directory / 'manifest.json').is_file())

    def test_raw_added_after_commit_preserves_all_metadata(self):
        self.assert_added_after_commit_preserves_metadata('stdout.bin')

    def test_unknown_entry_added_after_commit_preserves_all_metadata(self):
        self.assert_added_after_commit_preserves_metadata('foreign')

    def assert_added_after_commit_preserves_metadata(self, name):
        apply = self.application()
        capture = self.directory / 'captures-v1' / self.binding['captureId']
        preserved = {item.name: (item.read_bytes(), item.stat().st_ino) for item in capture.iterdir()}
        original = executor.atomic_owned

        def add_after_commit(*args, **kwargs):
            original(*args, **kwargs)
            (capture / name).write_bytes(b'preserve added entry')
            (capture / name).chmod(0o600)

        with mock.patch.object(executor, 'atomic_owned', side_effect=add_after_commit), \
             mock.patch.object(executor.os, 'unlink', wraps=executor.os.unlink) as unlink:
            with self.assertRaises(executor.ProtocolError):
                self.invoke(apply)
            unlink.assert_not_called()
        self.assertEqual((capture / name).read_bytes(), b'preserve added entry')
        for metadata, (raw, inode) in preserved.items():
            self.assertEqual((capture / metadata).read_bytes(), raw)
            self.assertEqual((capture / metadata).stat().st_ino, inode)
        record = json.loads((self.directory / (self.server.session_id + '.json')).read_text())
        self.assertEqual(record['retiredCapture']['attempt']['retireReceiptId'], apply['retireReceiptId'])

    def test_raw_added_after_unlink_blocks_remaining_unlinks(self):
        self.assert_added_after_unlink_preserves_remaining('stdout.bin')

    def test_unknown_entry_added_after_unlink_blocks_remaining_unlinks(self):
        self.assert_added_after_unlink_preserves_remaining('foreign')

    def assert_added_after_unlink_preserves_remaining(self, name):
        apply = self.application()
        capture = self.directory / 'captures-v1' / self.binding['captureId']
        purge = capture / 'purge.json'
        preserved = (purge.read_bytes(), purge.stat().st_ino)
        original = executor.os.unlink

        def add_after_unlink(entry, *args, **kwargs):
            original(entry, *args, **kwargs)
            if entry == 'manifest.json':
                (capture / name).write_bytes(b'preserve added entry')
                (capture / name).chmod(0o600)

        with mock.patch.object(executor.os, 'unlink', side_effect=add_after_unlink) as unlink:
            with self.assertRaises(executor.ProtocolError):
                self.invoke(apply)
            self.assertEqual([call.args[0] for call in unlink.call_args_list], ['manifest.json'])
        self.assertFalse((capture / 'manifest.json').exists())
        self.assertEqual((capture / name).read_bytes(), b'preserve added entry')
        self.assertEqual((purge.read_bytes(), purge.stat().st_ino), preserved)

    def test_later_metadata_replaced_after_commit_blocks_first_unlink(self):
        apply = self.application()
        capture = self.directory / 'captures-v1' / self.binding['captureId']
        manifest = (capture / 'manifest.json').read_bytes()
        purge = (capture / 'purge.json').read_bytes()
        original_inode = (capture / 'purge.json').stat().st_ino
        replacement_inode = None
        original = executor.atomic_owned
        def replace_after_commit(*args, **kwargs):
            nonlocal replacement_inode
            original(*args, **kwargs)
            replacement = capture / 'replacement'
            replacement.write_bytes(purge)
            replacement.chmod(0o600)
            replacement_inode = replacement.stat().st_ino
            os.replace(replacement, capture / 'purge.json')
        with mock.patch.object(executor, 'atomic_owned', side_effect=replace_after_commit), \
             mock.patch.object(executor.os, 'unlink', wraps=executor.os.unlink) as unlink:
            with self.assertRaises(executor.ProtocolError):
                self.invoke(apply)
            unlink.assert_not_called()
        self.assertEqual((capture / 'manifest.json').read_bytes(), manifest)
        self.assertEqual((capture / 'purge.json').read_bytes(), purge)
        self.assertNotEqual(replacement_inode, original_inode)
        self.assertEqual((capture / 'purge.json').stat().st_ino, replacement_inode)

    def test_directory_swap_after_commit_preserves_substitute(self):
        apply = self.application()
        capture = self.directory / 'captures-v1' / self.binding['captureId']
        moved = self.directory / 'preserved-original'
        original = executor.atomic_owned
        def swap_after_commit(*args, **kwargs):
            original(*args, **kwargs)
            capture.rename(moved)
            capture.mkdir(mode=0o700)
            (capture / 'foreign').write_bytes(b'preserve')
        with mock.patch.object(executor, 'atomic_owned', side_effect=swap_after_commit), mock.patch.object(executor.os, 'unlink') as unlink:
            with self.assertRaises(executor.ProtocolError):
                self.invoke(apply)
            unlink.assert_not_called()
        self.assertEqual((capture / 'foreign').read_bytes(), b'preserve')
        self.assertTrue((moved / 'purge.json').is_file())

    def test_busy_quota_lock_blocks_retire_and_new_capture_without_mutation(self):
        import fcntl
        root = executor.capture_root(self.directory)
        lock = os.open('quota.lock', os.O_RDONLY, dir_fd=root)
        try:
            apply = self.application()
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                self.assert_readonly(lambda: self.invoke(apply))
            with self.assertRaises(BlockingIOError):
                self.new_producer()
        finally:
            os.close(lock)
            os.close(root)

    def test_missing_lock_is_not_created_by_inspection(self):
        lock = self.directory / 'captures-v1' / 'quota.lock'
        lock.unlink()
        with self.assertRaises(FileNotFoundError):
            self.assert_readonly(lambda: self.invoke())
        self.assertFalse(lock.exists())

    def test_committed_retire_cannot_be_downgraded_to_generic_absence_or_purge(self):
        apply = self.application()
        self.invoke(apply)
        collect = {key: self.base[key] for key in ('producerSessionId', 'captureId', 'ownerProfileFingerprint', 'producerDispatchDeadline')}
        collector = executor.Server(self.environment, str(uuid.uuid4()), self.assessment, self.runtime,
                                    self.directory, mode='collect', control=collect)
        with mock.patch.object(executor, 'inspect_session', return_value={'known': True, 'active': False}):
            with self.assertRaises(executor.ProtocolError):
                collector.call('capture_collect', {'producerSessionId': self.server.session_id,
                    'captureId': self.binding['captureId'], 'kind': 'describe'})


if __name__ == '__main__':
    unittest.main()
