"""Portable unit tests for deterministic maintenance parsing; no WSL or apt runs."""
import importlib.util
import pathlib
import json
import sys
import tempfile
import types
import unittest
from unittest.mock import patch, mock_open

# Windows lacks these Linux modules; tests exercise pure parsing with no syscalls.
for module in ('fcntl', 'pwd'):
    try:
        __import__(module)
    except ImportError:
        sys.modules[module] = types.ModuleType(module)

source = pathlib.Path(__file__).with_name('environment-helper.py')
spec = importlib.util.spec_from_file_location('environment_helper', source)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


class SimulationTests(unittest.TestCase):
    def test_managed_catalog_downloads_package_indices_without_contents_or_translations(self):
        # The managed inventory needs package versions/dependencies, not apt-file
        # Contents indices. Options apply only to our dedicated APT invocation.
        for value in ('Acquire::Languages=none',
                      'Acquire::IndexTargets::deb::Contents-deb::DefaultEnabled=false',
                      'Acquire::IndexTargets::deb::Contents-deb-legacy::DefaultEnabled=false',
                      'Acquire::IndexTargets::deb::Contents-udeb::DefaultEnabled=false'):
            self.assertIn(value, helper.APT_OPTIONS)
        self.assertIn('APT::Get::AllowUnauthenticated=false', helper.APT_OPTIONS)

    def test_resolves_and_pins_dependencies_and_upgrades(self):
        output = 'Inst curl [8.0] (8.1-1 Kali:rolling [amd64])\nInst libcurl4 (8.1-1 Kali:rolling [amd64])\n'
        with patch.object(helper, 'execute', return_value=output) as execute, patch.object(helper, 'fingerprint', return_value='digest'):
            result = helper.simulate(['curl'])
        self.assertEqual(result['resolved'], ['curl=8.1-1', 'libcurl4=8.1-1'])
        argv = execute.call_args.args[0]
        self.assertIn('--simulate', argv)
        self.assertIn('Dir::Etc::sourceparts=-', argv)
        self.assertNotIn('shell', execute.call_args.kwargs)

    def test_removal_is_never_applied_from_install_plan(self):
        with patch.object(helper, 'execute', return_value='Remv important-package [1.0]\n'):
            with self.assertRaisesRegex(RuntimeError, 'removes packages'):
                helper.simulate(['curl'])

    def test_malformed_version_is_not_passed_to_apt(self):
        with patch.object(helper, 'execute', return_value='Inst curl (8.0;payload Kali)\n'):
            with self.assertRaisesRegex(RuntimeError, 'Unexpected package identity'):
                helper.simulate(['curl'])

    def test_no_changes_remains_a_valid_empty_plan(self):
        with patch.object(helper, 'execute', return_value='curl is already the newest version.\n'), patch.object(helper, 'fingerprint', return_value='digest'):
            self.assertEqual(helper.simulate(['curl'])['resolved'], [])

    def test_package_names_reject_options_and_shell_metacharacters(self):
        for name in ('--allow-unauthenticated', 'curl;id', '../package', 'curl=1.0', 'curl\nwhoami'):
            self.assertIsNone(helper.PACKAGE.fullmatch(name), name)
        self.assertIsNotNone(helper.PACKAGE.fullmatch('libssl3:amd64'))

    def test_changed_catalog_blocks_apply_before_install(self):
        plan = {'id': 'test', 'packages': ['curl'], 'resolved': ['curl=1.0'], 'fingerprint': 'before'}
        stub_fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, flock=lambda *args: None)
        scratch = source.parent.parent / '.local'
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as directory, patch.object(helper, 'ROOT', pathlib.Path(directory)), \
                patch.object(helper, 'fcntl', stub_fcntl), patch.object(helper, 'require_identity'), \
                patch.object(helper.os, 'geteuid', return_value=0, create=True), \
                patch.object(helper.sys, 'argv', ['helper', 'apply', 'identity', json.dumps(plan)]), \
                patch.object(helper, 'simulate', return_value={'resolved': ['curl=2.0'], 'fingerprint': 'after'}), \
                patch.object(helper, 'execute') as execute:
            with self.assertRaisesRegex(RuntimeError, 'changed'):
                helper.main()
            execute.assert_not_called()

    def test_process_start_parses_names_with_parentheses(self):
        # /proc parsing is deterministic and does not mistake a process name with
        # spaces/parentheses for stat fields.
        stat = '123 (name with (parentheses)) ' + ' '.join(['S'] + ['0'] * 18 + ['4567'] + ['0'] * 3)
        with patch.object(helper.pathlib.Path, 'read_text', return_value=stat):
            self.assertEqual(helper.process_start(123), '4567')

    def test_wsl_configuration_preserves_other_options_and_exact_key_case(self):
        original = '[network]\nhostname = custom\ngenerateResolvConf = false\n[interop]\nappendwindowspath = true\ncustomSetting = retained\n'
        result = helper.wsl_configuration(original)
        self.assertIn('appendWindowsPath = false', result)
        self.assertNotIn('appendwindowspath', result)
        self.assertIn('generateResolvConf = false', result)
        self.assertIn('hostname = custom', result)
        self.assertIn('customSetting = retained', result)
        self.assertEqual(helper.wsl_configuration(result), result)

    def test_kernel_interop_disabled_registration_is_not_enabled(self):
        self.assertFalse(helper.interop_enabled(None))
        self.assertFalse(helper.interop_enabled('disabled\ninterpreter /init'))
        self.assertTrue(helper.interop_enabled('enabled\ninterpreter /init'))
        self.assertTrue(helper.interop_enabled('unrecognized'))

    def test_detects_drvfs_and_9p_host_mounts(self):
        table = '/dev/sdc / ext4 rw 0 0\nC:\\134 /mnt/c 9p rw,aname=drvfs;path=C:\\134 0 0\nD: /mnt/d drvfs rw 0 0\n'
        self.assertEqual(helper.windows_mounts(table), ['/mnt/c', '/mnt/d'])


class MaintenanceOutcomeTests(unittest.TestCase):
    def setUp(self):
        scratch = source.parent.parent / '.local'
        scratch.mkdir(exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.directory.cleanup)
        self.root = pathlib.Path(self.directory.name)
        self.root_patch = patch.object(helper, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.current_boot = '11111111-1111-4111-8111-111111111111'
        self.boot_patch = patch.object(helper, 'boot_id', return_value=self.current_boot)
        self.boot_patch.start()
        self.addCleanup(self.boot_patch.stop)

    def journal(self):
        return json.loads((self.root / 'maintenance.json').read_text())

    def test_refresh_requires_all_indices_and_records_success_only_on_zero_exit(self):
        with patch.object(helper, 'execute', return_value='indices acquired') as execute, \
                patch.object(helper, 'fingerprint', return_value='fresh'):
            result = helper.maintenance_action('refresh')
        self.assertIn('--error-on=any', execute.call_args.args[0])
        self.assertEqual(result['fingerprint'], 'fresh')
        self.assertEqual(self.journal()['status'], 'completed')
        self.assertIs(self.journal()['refreshSucceeded'], True)

    def test_failed_or_unsupported_refresh_never_claims_complete(self):
        failures = [helper.CommandFailed(100, '', 'Failed to fetch index'),
                    helper.CommandFailed(100, '', 'Command line option --error-on is not understood'),
                    RuntimeError('Maintenance limited: timeout')]
        for failure in failures:
            with self.subTest(failure=str(failure)), patch.object(helper, 'execute', side_effect=failure), \
                    patch.object(helper, 'fingerprint') as fingerprint:
                with self.assertRaises(RuntimeError):
                    helper.maintenance_action('refresh')
                fingerprint.assert_not_called()
                self.assertEqual(self.journal()['status'], 'unknown')
                self.assertIs(self.journal()['refreshSucceeded'], False)
                self.assertEqual(self.journal()['error'], str(failure))

    def test_only_missing_package_exit_is_classified_absent(self):
        with patch.object(helper, 'execute', side_effect=helper.CommandFailed(1, '', 'no packages found')):
            self.assertEqual(helper.package_status('curl'), {'package': 'curl', 'installed': False, 'version': None})
        for failure in [helper.CommandFailed(2, '', 'database unreadable'),
                        helper.CommandFailed(1, 'ii \t1.0', 'partial query'),
                        RuntimeError('Maintenance limited: output_limit'), OSError('process unavailable')]:
            with self.subTest(failure=str(failure)), patch.object(helper, 'execute', side_effect=failure):
                with self.assertRaises(type(failure)):
                    helper.package_status('curl')

    def test_held_or_pending_removal_packages_are_still_installed(self):
        for state in ('ii ', 'hi ', 'ri '):
            with self.subTest(state=state), patch.object(helper, 'execute', return_value=state + '\t8.1-1'):
                self.assertEqual(helper.package_status('curl'), {'package': 'curl', 'installed': True, 'version': '8.1-1'})
        for state in ('un ', 'rc '):
            with self.subTest(state=state), patch.object(helper, 'execute', return_value=state + '\t8.1-1'):
                self.assertIs(helper.package_status('curl')['installed'], False)

    def test_incomplete_or_malformed_package_state_is_never_absent(self):
        for value in ('iU \t8.1', 'iHR\t8.1', 'ii \t', 'garbage', 'ii \t8.1\nii \t8.2'):
            with self.subTest(value=value), patch.object(helper, 'execute', return_value=value):
                with self.assertRaises(RuntimeError):
                    helper.package_status('curl')

    def test_catalog_propagates_database_failure_instead_of_false_absence(self):
        with patch.object(helper.sys, 'argv', ['helper', 'catalog', 'identity', 'curl']), \
                patch.object(helper, 'execute', side_effect=helper.CommandFailed(2, '', 'database failed')):
            with self.assertRaises(helper.CommandFailed):
                helper.maintenance_action('catalog')

    def test_refresh_reconciliation_proves_quiescence_without_replaying_or_claiming_freshness(self):
        helper.atomic_json(self.root / 'maintenance.json', {'action': 'refresh', 'status': 'unknown', 'error': 'network failed',
                                                          'processPid': 123, 'processStart': '456', 'bootId': self.current_boot})
        opened = mock_open()
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lambda *args: None)
        with patch('builtins.open', opened), patch.object(helper, 'fcntl', fcntl), \
                patch.object(helper, 'process_start', return_value=None), \
                patch.object(helper, 'execute', return_value='') as execute:
            result = helper.maintenance_action('reconcile')
        self.assertIs(result['clean'], True)
        self.assertEqual(self.journal()['status'], 'reconciled')
        self.assertIs(self.journal()['quiescent'], True)
        self.assertIs(self.journal()['refreshSucceeded'], False)
        self.assertEqual(self.journal()['error'], 'network failed')
        self.assertEqual(execute.call_args_list[0].args[0], ['/usr/bin/dpkg', '--audit'])
        self.assertEqual(execute.call_count, 1)  # Never repeats apt-get update.
        self.assertEqual([call.args[0] for call in opened.call_args_list],
                         ['/var/lib/dpkg/lock-frontend', '/var/lib/dpkg/lock', str(self.root / 'apt/lists/lock')])

    def test_busy_apt_lists_lock_prevents_false_quiescence(self):
        helper.atomic_json(self.root / 'maintenance.json', {'action': 'refresh', 'status': 'unknown',
                                                          'processPid': 123, 'processStart': '456', 'bootId': self.current_boot})
        from unittest.mock import Mock
        lockf = Mock(side_effect=[None, None, BlockingIOError()])
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lockf)
        with patch('builtins.open', mock_open()), patch.object(helper, 'fcntl', fcntl), \
                patch.object(helper, 'process_start', return_value=None), \
                patch.object(helper, 'execute') as execute:
            with self.assertRaisesRegex(RuntimeError, 'still holds a lock'):
                helper.maintenance_action('reconcile')
            execute.assert_not_called()
        self.assertEqual(self.journal()['status'], 'unknown')

    def test_live_recorded_process_prevents_reconciliation(self):
        journal = {'action': 'refresh', 'status': 'unknown', 'processPid': 123, 'processStart': '456', 'bootId': self.current_boot}
        helper.atomic_json(self.root / 'maintenance.json', journal)
        with patch.object(helper, 'process_start', return_value='456'), \
                patch('builtins.open', mock_open()) as opened, patch.object(helper, 'execute') as execute:
            with self.assertRaisesRegex(RuntimeError, 'still running'):
                helper.maintenance_action('reconcile')
            opened.assert_not_called()
            execute.assert_not_called()
        self.assertEqual(self.journal()['status'], 'unknown')

    def test_partial_installation_cannot_be_reconciled_complete(self):
        helper.atomic_json(self.root / 'maintenance.json', {'action': 'install', 'status': 'unknown', 'resolved': ['curl=8.1'],
                                                          'processPid': 123, 'processStart': '456', 'bootId': self.current_boot})
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lambda *args: None)
        with patch('builtins.open', mock_open()), patch.object(helper, 'fcntl', fcntl), \
                patch.object(helper, 'process_start', return_value=None), \
                patch.object(helper, 'execute', side_effect=['', 'iU \t8.1']):
            with self.assertRaisesRegex(RuntimeError, 'repair'):
                helper.maintenance_action('reconcile')
        self.assertEqual(self.journal()['status'], 'unknown')

    def test_child_started_before_identity_record_cannot_be_reconciled_in_same_boot(self):
        def spawn(argv, **options):
            self.assertEqual(self.journal()['bootId'], self.current_boot)
            self.assertIsNone(self.journal()['processPid'])
            self.assertIsNone(self.journal()['processStart'])
            return types.SimpleNamespace(pid=123)

        # No child is actually started: the fixture interrupts the exact point
        # after Popen returned and before its PID/start identity became durable.
        with patch.object(helper.subprocess, 'Popen', side_effect=spawn) as popen, \
                patch.object(helper, 'process_start', side_effect=RuntimeError('interrupted after spawn')):
            with self.assertRaisesRegex(RuntimeError, 'interrupted after spawn'):
                helper.maintenance_action('refresh')
        self.assertEqual(popen.call_count, 1)
        journal = self.journal()
        self.assertEqual(journal['status'], 'unknown')
        with patch('builtins.open', mock_open()) as opened, patch.object(helper, 'execute') as execute:
            with self.assertRaisesRegex(RuntimeError, 'identity is incomplete'):
                helper.maintenance_action('reconcile')
            opened.assert_not_called()
            execute.assert_not_called()
        self.assertEqual(self.journal(), journal)

        # A new Linux boot proves the unrecorded child cannot remain alive.
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lambda *args: None)
        with patch.object(helper, 'boot_id', return_value='22222222-2222-4222-8222-222222222222'), \
                patch('builtins.open', mock_open()), patch.object(helper, 'fcntl', fcntl), \
                patch.object(helper, 'execute', return_value='') as execute:
            result = helper.maintenance_action('reconcile')
        self.assertIs(result['clean'], True)
        self.assertEqual(self.journal()['status'], 'reconciled')
        self.assertIs(self.journal()['refreshSucceeded'], False)
        self.assertEqual(self.journal()['error'], journal['error'])
        execute.assert_called_once_with(['/usr/bin/dpkg', '--audit'])

    def test_incomplete_or_legacy_identity_never_uses_free_locks_as_termination_proof(self):
        for identity in ({}, {'bootId': 'invalid'}, {'bootId': self.current_boot},
                         {'bootId': self.current_boot, 'processPid': 123, 'processStart': None},
                         {'bootId': self.current_boot, 'processPid': True, 'processStart': '456'}):
            journal = {'action': 'refresh', 'status': 'unknown', **identity}
            helper.atomic_json(self.root / 'maintenance.json', journal)
            with self.subTest(identity=identity), patch('builtins.open', mock_open()) as opened, \
                    patch.object(helper, 'execute') as execute:
                with self.assertRaisesRegex(RuntimeError, 'identity is incomplete'):
                    helper.maintenance_action('reconcile')
                opened.assert_not_called()
                execute.assert_not_called()
            self.assertEqual(self.journal(), journal)

    def test_uncaught_crash_after_spawn_preserves_durable_boot_and_blocks_reconciliation(self):
        class SimulatedCrash(BaseException):
            pass

        with patch.object(helper.subprocess, 'Popen', return_value=types.SimpleNamespace(pid=123)), \
                patch.object(helper, 'process_start', side_effect=SimulatedCrash):
            with self.assertRaises(SimulatedCrash):
                helper.maintenance_action('refresh')
        journal = self.journal()
        self.assertEqual(journal['status'], 'running')
        self.assertEqual(journal['bootId'], self.current_boot)
        self.assertIsNone(journal['processPid'])
        with patch('builtins.open', mock_open()) as opened, patch.object(helper, 'execute') as execute:
            with self.assertRaisesRegex(RuntimeError, 'identity is incomplete'):
                helper.maintenance_action('reconcile')
            opened.assert_not_called()
            execute.assert_not_called()
        self.assertEqual(self.journal(), journal)

    def test_launch_is_not_started_when_pre_dispatch_journal_cannot_be_saved(self):
        with patch.object(helper, 'atomic_json', side_effect=OSError('journal unavailable')), \
                patch.object(helper.subprocess, 'Popen') as popen:
            with self.assertRaisesRegex(OSError, 'journal unavailable'):
                helper.execute(['fixture'], journal={'action': 'refresh', 'status': 'running'})
            popen.assert_not_called()

    def repair_fixture(self, journal=None):
        journal = journal or {'action': 'refresh', 'status': 'unknown', 'error': 'legacy interrupted update'}
        helper.atomic_json(self.root / 'maintenance.json', journal)
        raw = (self.root / 'maintenance.json').read_bytes()
        identity = '33333333-3333-4333-8333-333333333333'
        receipt = {'repairId': '44444444-4444-4444-8444-444444444444',
                   'planId': '55555555-5555-4555-8555-555555555555', 'environmentId': identity,
                   'distro': 'OpenKoda-Kali', 'maintenanceSha256': helper.hashlib.sha256(raw).hexdigest(),
                   'terminationVerified': True, 'verifiedAtUnixSeconds': int(helper.time.time())}
        return journal, raw, receipt, ['helper', 'repair', identity, json.dumps(receipt)]

    def test_verified_distro_stop_recovers_legacy_same_boot_without_repeating_apt(self):
        original, raw, receipt, argv = self.repair_fixture()
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lambda *args: None)
        with patch.object(helper.sys, 'argv', argv), patch('builtins.open', mock_open()), \
                patch.object(helper, 'fcntl', fcntl), patch.object(helper, 'process_start', return_value='789'), \
                patch.object(helper, 'execute', return_value='') as execute:
            result = helper.maintenance_action('repair')
        self.assertIs(result['clean'], True)
        self.assertEqual(result['repairId'], receipt['repairId'])
        self.assertEqual(self.journal()['status'], 'reconciled')
        self.assertIs(self.journal()['refreshSucceeded'], False)
        self.assertEqual(self.journal()['bootId'], self.current_boot)
        self.assertEqual(self.journal()['witnessRole'], 'repair-audit-helper')
        self.assertEqual(self.journal()['error'], original['error'])
        history = json.loads((self.root / 'maintenance-history' / (receipt['repairId'] + '.json')).read_text())
        self.assertEqual(history['originalRaw'].encode(), raw)
        self.assertEqual(history['originalSha256'], receipt['maintenanceSha256'])
        execute.assert_called_once_with(['/usr/bin/dpkg', '--audit'])

    def test_untrusted_expired_or_changed_receipt_never_reaches_locks_or_audit(self):
        original, _, receipt, argv = self.repair_fixture()
        for fields in ({'terminationVerified': False}, {'distro': 'Ubuntu'},
                       {'environmentId': '99999999-9999-4999-8999-999999999999'},
                       {'verifiedAtUnixSeconds': 0}, {'maintenanceSha256': 'a' * 64},
                       {'shell': 'anything'}):
            changed = {**receipt, **fields}
            with self.subTest(fields=fields), patch.object(helper.sys, 'argv', [*argv[:3], json.dumps(changed)]), \
                    patch('builtins.open', mock_open()) as opened, patch.object(helper, 'execute') as execute:
                with self.assertRaises(RuntimeError):
                    helper.maintenance_action('repair')
                opened.assert_not_called()
                execute.assert_not_called()
            self.assertEqual(self.journal(), original)
            self.assertFalse((self.root / 'maintenance-history').exists())

    def test_repair_audit_failure_retains_original_and_consumes_receipt_without_retry(self):
        original, raw, receipt, argv = self.repair_fixture()
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lambda *args: None)
        with patch.object(helper.sys, 'argv', argv), patch('builtins.open', mock_open()), \
                patch.object(helper, 'fcntl', fcntl), patch.object(helper, 'execute', return_value='unfinished package'):
            with self.assertRaisesRegex(RuntimeError, 'repair required'):
                helper.maintenance_action('repair')
        self.assertEqual((self.root / 'maintenance.json').read_bytes(), raw)
        self.assertTrue((self.root / 'maintenance-history' / (receipt['repairId'] + '.json')).exists())
        with patch.object(helper.sys, 'argv', argv), patch.object(helper, 'execute') as execute:
            with self.assertRaises(FileExistsError):
                helper.maintenance_action('repair')
            execute.assert_not_called()
        self.assertEqual(self.journal(), original)

    def test_busy_package_lock_after_stop_is_not_reported_repaired(self):
        original, _, _, argv = self.repair_fixture()
        def busy(*args):
            raise BlockingIOError()
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=busy)
        with patch.object(helper.sys, 'argv', argv), patch('builtins.open', mock_open()), \
                patch.object(helper, 'fcntl', fcntl), patch.object(helper, 'execute') as execute:
            with self.assertRaisesRegex(RuntimeError, 'still holds a lock'):
                helper.maintenance_action('repair')
            execute.assert_not_called()
        self.assertEqual(self.journal(), original)

    def test_partial_installation_is_preserved_after_confirmed_stop(self):
        original, raw, _, argv = self.repair_fixture({'action': 'install', 'status': 'unknown', 'resolved': ['curl=8.1']})
        fcntl = types.SimpleNamespace(LOCK_EX=1, LOCK_NB=2, lockf=lambda *args: None)
        with patch.object(helper.sys, 'argv', argv), patch('builtins.open', mock_open()), \
                patch.object(helper, 'fcntl', fcntl), patch.object(helper, 'execute', side_effect=['', 'ii \t8.0']):
            with self.assertRaisesRegex(RuntimeError, 'partial'):
                helper.maintenance_action('repair')
        self.assertEqual((self.root / 'maintenance.json').read_bytes(), raw)
        self.assertEqual(self.journal(), original)

    def test_snapshot_hash_binds_original_bytes_not_reserialized_json(self):
        original = b'{ "action" : "refresh", "status" : "unknown" }\n'
        (self.root / 'maintenance.json').write_bytes(original)
        value, digest, raw = helper.maintenance_snapshot()
        self.assertEqual(digest, helper.hashlib.sha256(original).hexdigest())
        self.assertEqual(raw, original)
        self.assertEqual(value['action'], 'refresh')

    def test_fast_exit_output_limit_still_blocks_success(self):
        def spawn(argv, **options):
            options['stdout'].write(b'x' * (helper.MAX_OUTPUT + 1))
            return types.SimpleNamespace(returncode=0, poll=lambda: 0)
        with patch.object(helper.subprocess, 'Popen', side_effect=spawn):
            with self.assertRaisesRegex(RuntimeError, 'output_limit'):
                helper.execute(['fixture'])

    def test_completed_nonzero_exit_retains_machine_readable_code(self):
        def spawn(argv, **options):
            options['stderr'].write(b'no packages found')
            return types.SimpleNamespace(returncode=1, poll=lambda: 1)
        with patch.object(helper.subprocess, 'Popen', side_effect=spawn):
            with self.assertRaises(helper.CommandFailed) as caught:
                helper.execute(['fixture'])
        self.assertEqual(caught.exception.returncode, 1)
        self.assertEqual(caught.exception.stderr, 'no packages found')


if __name__ == '__main__':
    unittest.main()
