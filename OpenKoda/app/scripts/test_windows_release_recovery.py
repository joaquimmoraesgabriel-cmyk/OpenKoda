import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from windows_release_recovery import Recovery, Refused, Release, file_sha, guarded_file, read_json, registration_fields, save


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fake_release(root, version):
    directory = root / version
    directory.mkdir()
    source = f"source:{version}".encode()
    payload = f"payload:{version}".encode()
    setup = directory / f"OpenKoda_{version}_x64-setup.exe"
    setup.write_bytes(f"disposable NSIS fixture {version}".encode())
    install_hash = file_sha(setup)
    (directory / "receipts").mkdir()
    receipt = directory / "receipts/packageExtraction.json"
    save(receipt, {"passed": True, "mode": "ExtractOnly",
                   "newRelease": {"version": version, "installerSha256": install_hash, "sourceBinarySha256": sha(source)},
                   "newPayload": {"entry": "openkoda.exe", "verification": {"verified": True, "sourceSha256": sha(source),
                                                                            "sha256": sha(payload)}}})
    manifest = directory / "release.json"
    save(manifest, {"version": version, "schema": 8, "installer": str(setup), "sha256": install_hash,
                    "sourceBinarySha256": sha(source), "receipts": {"packageExtraction": {"sha256": file_sha(receipt)}}})
    return Release.load({"manifest": str(manifest), "sha256": file_sha(manifest)})


class FixtureHost:
    def __init__(self, root):
        self.reg = root / "fake-registry.json"
        self.state = root / "journal"
        self.default_dest = root / "test user" / "OpenKoda"
        self.default_dest.parent.mkdir()
        self.roots = (root / "profiles", root / "legacy")
        self.roots[0].mkdir()
        (self.roots[0] / "profiles.lock").touch()
        (self.roots[0] / "openkoda.sqlite3").write_bytes(b"disposable user database")
        self.calls = []
        self.fail = None
        self.busy = False

    def registration(self, dest):
        if not self.reg.exists():
            return None
        value = read_json(self.reg)
        if Path(value["destination"]) != dest:
            raise Refused("Registro de outro destino.")
        if "fields" in value:
            return registration_fields(dest, value["fields"])
        return value["version"]

    def ready(self):
        if self.busy:
            raise Refused("Processo ativo.")

    def run(self, release, dest, uninstall_sha=None):
        self.ready()
        action = "remove" if uninstall_sha else "setup"
        self.calls.append((action, release.version))
        if self.fail == action + "-before":
            raise Refused("Falha injetada antes da operação.")
        if uninstall_sha:
            if file_sha(dest / "uninstall.exe") != uninstall_sha:
                raise Refused("Desinstalador diferente.")
            (dest / "openkoda.exe").unlink(missing_ok=True)
            if self.fail == "remove-partial":
                raise Refused("Falha do desinstalador ainda registrado.")
            self.reg.unlink()
        else:
            if file_sha(release.setup) != release.setup_sha:
                raise Refused("Instalador diferente.")
            dest.mkdir(exist_ok=True)
            (dest / "openkoda.exe").write_bytes(f"payload:{release.version}".encode())
            if self.fail == "setup-partial":
                raise Refused("Falha antes de registrar.")
            (dest / "uninstall.exe").write_bytes(f"uninstall:{release.version}".encode())
            (dest / "avisos").mkdir(exist_ok=True)
            (dest / "avisos/AVISOS.txt").write_text(release.version)
            save(self.reg, {"version": release.version, "destination": str(dest)})
        if self.fail == action + "-after":
            raise Refused("Falha depois da alteração, antes de gravar o recibo.")


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        fixture_root = Path(__file__).resolve().parents[1] / ".test-work"
        fixture_root.mkdir(exist_ok=True)
        temp = tempfile.TemporaryDirectory(prefix="descarte-", dir=fixture_root)
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        self.old = fake_release(root, "0.1.6")
        self.new = fake_release(root, "0.1.7")
        self.host = FixtureHost(root)
        self.dest = self.host.default_dest
        self.tool = Recovery(self.host)
        self.host.run(self.old, self.dest)
        self.host.calls.clear()

    def plan(self, op, old=None, new=None):
        if op == "rollback":
            old, new = self.new, self.old
        elif op == "repair":
            old = old or self.old
            new = new or old
        return self.tool.prepare(op, old or self.old, new or self.new, self.dest)[0]

    def apply(self, token):
        with self.tool.session():
            return self.tool.apply(token)

    def upgrade(self):
        self.apply(self.plan("upgrade"))

    def test_full_roundtrip_and_resume_completed_without_second_install(self):
        self.upgrade()
        token = self.plan("rollback")
        receipt = self.apply(token)
        self.assertEqual(receipt, self.apply(token))
        self.assertEqual(self.host.calls, [("setup", "0.1.7"), ("remove", "0.1.7"), ("setup", "0.1.6")])
        self.assertEqual(self.host.registration(self.dest), "0.1.6")
        self.assertEqual((self.dest/"avisos/AVISOS.txt").read_text(), "0.1.6")
        self.assertEqual((self.host.roots[0]/"openkoda.sqlite3").read_bytes(), b"disposable user database")
        self.assertFalse(self.tool.active.exists())

    def test_after_upgrade_profile_writes_including_new_file_block_before_uninstall(self):
        self.upgrade()
        token = self.plan("rollback")
        (self.host.roots[0]/"openkoda.sqlite3-wal").write_bytes(b"new evidence")
        with self.assertRaises(Refused):
            self.apply(token)
        with self.assertRaises(Refused):
            self.plan("rollback")
        self.assertEqual(self.host.calls, [("setup", "0.1.7")])
        self.assertEqual((self.host.roots[0]/"openkoda.sqlite3-wal").read_bytes(), b"new evidence")

    def test_fresh_interruption_before_registry_can_resume(self):
        self.host.reg.unlink()
        for path in sorted(self.dest.rglob("*"), key=lambda x: len(x.parts), reverse=True):
            path.rmdir() if path.is_dir() else path.unlink()
        self.dest.rmdir()
        token = self.plan("install", self.old, self.old)
        self.host.fail = "setup-partial"
        with self.assertRaises(Refused):
            self.apply(token)
        self.assertIsNone(self.host.registration(self.dest))
        self.host.fail = None
        self.assertTrue(self.apply(token)["dataPreserved"])

    def test_repair_rerun_even_after_prior_completed_repair_same_profile(self):
        self.apply(self.plan("repair"))
        (self.dest/"openkoda.exe").unlink()
        self.apply(self.plan("repair"))
        self.assertEqual(file_sha(self.dest/"openkoda.exe"), self.old.payload_sha)

    def test_manual_interruption_when_registry_is_gone_but_payload_matches(self):
        self.host.reg.unlink()
        self.apply(self.plan("repair"))
        self.assertEqual(self.host.registration(self.dest), self.old.version)

    def test_no_manual_repair_over_unidentified_unregistered_payload(self):
        self.host.reg.unlink()
        (self.dest/"openkoda.exe").write_bytes(b"unknown binary")
        with self.assertRaises(Refused):
            self.plan("repair")
        self.assertFalse(self.host.calls)

    def test_manual_partial_registry_requires_matching_payload_version_and_destination(self):
        save(self.host.reg, {"destination": str(self.dest), "fields": {"DisplayVersion": "0.1.7"}})
        with self.assertRaises(Refused):
            self.plan("repair", self.old)
        save(self.host.reg, {"destination": str(self.dest), "fields": {"UninstallString": "unexpected.exe"}})
        with self.assertRaises(Refused):
            self.plan("repair", self.old)
        save(self.host.reg, {"destination": str(self.dest), "fields": {"InstallLocation": str(self.dest)}})
        self.assertEqual(self.apply(self.plan("repair"))["version"], self.old.version)

    def test_accompanied_setup_recovers_even_when_registry_was_only_partially_written(self):
        token = self.plan("upgrade")
        self.host.fail = "setup-partial"
        with self.assertRaises(Refused):
            self.apply(token)
        save(self.host.reg, {"destination": str(self.dest), "fields": {"InstallLocation": str(self.dest),
                                                                      "DisplayVersion": "0.1.7.0"}})
        self.host.fail = None
        self.assertEqual(self.apply(token)["version"], self.new.version)

    def test_interrupted_registration_and_binary_cutovers_resume(self):
        self.upgrade()
        token = self.plan("rollback")
        for point in ("remove-partial", "remove-after", "setup-partial", "setup-after"):
            self.host.fail = point
            with self.assertRaises(Refused):
                self.apply(token)
            self.assertTrue(self.tool.active.exists())
        self.host.fail = None
        self.assertEqual(self.apply(token)["version"], self.old.version)

    def test_newer_repair_recovers_interrupted_rollback_after_profile_changes(self):
        self.upgrade()
        token = self.plan("rollback")
        self.host.fail = "remove-after"
        with self.assertRaises(Refused):
            self.apply(token)
        (self.host.roots[0]/"openkoda.sqlite3").write_bytes(b"new format")
        with self.assertRaises(Refused):
            self.apply(token)
        self.host.fail = None
        newer = self.plan("repair", self.new)
        self.assertEqual(self.apply(newer)["version"], self.new.version)
        self.assertEqual((self.host.roots[0]/"openkoda.sqlite3").read_bytes(), b"new format")
        with self.assertRaises(Refused):
            self.plan("rollback")

    def test_absent_checkpoint_and_standalone_older_repair_cannot_downgrade(self):
        self.host.run(self.new, self.dest)
        with self.assertRaises(Refused):
            self.plan("rollback")
        with self.assertRaises(Refused):
            self.plan("repair", self.old)

    def test_after_partial_upgrade_newer_repair_preserves_a_changed_profile(self):
        token = self.plan("upgrade")
        self.host.fail = "setup-partial"
        with self.assertRaises(Refused):
            self.apply(token)
        (self.host.roots[0]/"openkoda.sqlite3").write_bytes(b"newer-format")
        with self.assertRaises(Refused):
            self.plan("repair", self.old)
        rescue = self.plan("repair", self.new)
        self.host.fail = None
        self.assertEqual(self.apply(rescue)["version"], self.new.version)
        self.assertEqual((self.host.roots[0]/"openkoda.sqlite3").read_bytes(), b"newer-format")
        with self.assertRaises(Refused):
            self.plan("rollback")

    def test_compact_release_receipt_is_bound_to_package_and_payload_digests(self):
        path = self.new.manifest.parent/"receipts/packageExtraction.json"
        receipt = read_json(path)
        del receipt["newPayload"]["entry"], receipt["newRelease"]["version"]
        save(path, receipt)
        manifest = read_json(self.new.manifest)
        manifest["receipts"]["packageExtraction"]["sha256"] = file_sha(path)
        save(self.new.manifest, manifest)
        ref = {"manifest": str(self.new.manifest), "sha256": file_sha(self.new.manifest)}
        self.assertEqual(Release.load(ref).payload_sha, self.new.payload_sha)
        receipt["newRelease"]["installerSha256"] = "0"*64
        save(path, receipt)
        manifest["receipts"]["packageExtraction"]["sha256"] = file_sha(path)
        save(self.new.manifest, manifest)
        with self.assertRaises(Refused):
            Release.load({**ref, "sha256": file_sha(self.new.manifest)})

    def test_changed_uninstaller_blocks(self):
        self.upgrade()
        token = self.plan("rollback")
        (self.dest/"uninstall.exe").write_bytes(b"unexpected executable")
        with self.assertRaises(Refused):
            self.apply(token)
        self.assertEqual(self.host.calls, [("setup", "0.1.7")])

    def test_review_rejects_wrong_destination_and_unowned_files(self):
        with self.assertRaises(Refused):
            self.tool.prepare("upgrade", self.old, self.new, self.dest.with_name("outro"))
        (self.dest/"pessoal.txt").write_bytes(b"preserve")
        with self.assertRaises(Refused):
            self.plan("upgrade")
        self.assertFalse(self.host.calls)

    def test_pending_package_or_plan_tamper_cannot_launch(self):
        token = self.plan("upgrade")
        original = self.new.setup.read_bytes()
        self.new.setup.write_bytes(b"changed")
        with self.assertRaises(Refused):
            self.apply(token)
        self.new.setup.write_bytes(original)
        planfile = self.tool.state / (token + ".json")
        save(planfile, {**read_json(planfile), "destination": str(self.dest.parent)})
        with self.assertRaises(Refused):
            self.apply(token)
        self.assertFalse(self.host.calls)

    def test_partial_upgrade_retries_without_committing_checkpoint_too_early(self):
        token = self.plan("upgrade")
        self.host.fail = "setup-after"
        with self.assertRaises(Refused):
            self.apply(token)
        self.assertFalse(self.tool.previous.exists())
        self.host.fail = None
        self.apply(token)
        self.assertTrue(self.tool.previous.exists())

    def test_freshly_started_app_or_preexisting_unrelated_registration_blocks(self):
        token = self.plan("upgrade")
        self.host.busy = True
        with self.assertRaises(Refused):
            self.apply(token)
        self.host.busy = False
        save(self.host.reg, {"version": "2.0.0", "destination": str(self.dest)})
        with self.assertRaises(Refused):
            self.apply(token)
        self.assertFalse(self.host.calls)

    def test_open_profile_lock_and_recovery_lock_are_exclusive_on_windows(self):
        if os.name != "nt":
            self.skipTest("Exclusividade CreateFileW depende do Windows.")
        with guarded_file(self.host.roots[0]/"profiles.lock", exclusive=True):
            with self.assertRaises((Refused, OSError)):
                self.plan("upgrade")
        with self.tool.session():
            with self.assertRaises((Refused, OSError)):
                with self.tool.session():
                    self.fail("double locked")

    def test_rejects_hardlink_in_profile_without_following_it(self):
        data = self.host.roots[0]/"file"
        data.write_bytes(b"disposable private bytes")
        os.link(data, self.host.roots[0]/"alias")
        with self.assertRaises(Refused):
            self.plan("upgrade")
        self.assertFalse(self.host.calls)

    def test_no_profile_content_saved_in_journal(self):
        self.upgrade()
        for path in self.host.state.glob("*.json"):
            self.assertNotIn(b"disposable user database", path.read_bytes())
            self.assertNotIn(b"openkoda.sqlite3", path.read_bytes())


if __name__ == "__main__":
    unittest.main(verbosity=2)

