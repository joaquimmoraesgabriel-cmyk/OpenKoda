"""Ensaio NSIS isolado. Sem --execute, somente consulta as pré-condições."""
import argparse
from contextlib import closing
import ctypes
import json
import os
from pathlib import Path
import sqlite3
import uuid
import winreg as wr
import zipfile

from windows_release_recovery import (Recovery, Refused, Release, WindowsHost, absolute, file_sha, fingerprint,
                                      installed, owned_directory, read_json, require, save)


PRODUCT = r"Software\openkoda\OpenKoda"
UNINSTALL = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda"
RUN = r"Software\Microsoft\Windows\CurrentVersion\Run"


def shared():
    registration = []
    autostart = []
    for hive in (wr.HKEY_CURRENT_USER, wr.HKEY_LOCAL_MACHINE):
        for view in (wr.KEY_WOW64_32KEY, wr.KEY_WOW64_64KEY):
            for name in (PRODUCT, UNINSTALL):
                try:
                    with wr.OpenKey(hive, name, 0, wr.KEY_READ | view):
                        registration.append([int(hive), view, name])
                except FileNotFoundError:
                    pass
            try:
                with wr.OpenKey(hive, RUN, 0, wr.KEY_READ | view) as key:
                    wr.QueryValueEx(key, "OpenKoda")
                    autostart.append([int(hive), view])
            except FileNotFoundError:
                pass
    shortcuts = []
    for folder in (0x10, 0x19, 2, 0x17):  # DesktopDirectory, CommonDesktopDirectory, Programs, CommonPrograms
        path = ctypes.create_unicode_buffer(32768)
        require(ctypes.windll.shell32.SHGetFolderPathW(None, folder, None, 0, path) == 0 and path.value,
                "Não foi possível verificar a pasta dos atalhos.")
        if (Path(path.value)/"OpenKoda.lnk").exists():
            shortcuts.append(folder)
    return {"registration": registration, "autostart": autostart, "shortcuts": shortcuts}


def clear_owned_location(dest):
    # NSIS can retain a hint for the next install. The preflight proved all these keys absent.
    for view in (wr.KEY_WOW64_32KEY, wr.KEY_WOW64_64KEY):
        try:
            key = wr.OpenKey(wr.HKEY_CURRENT_USER, PRODUCT, 0, wr.KEY_READ | view)
        except FileNotFoundError:
            continue
        with key:
            children, values, _ = wr.QueryInfoKey(key)
            names = {wr.EnumValue(key, i)[0] for i in range(values)}
            require(children == 0 and names <= {"", "Installer Language"}
                    and absolute(wr.QueryValueEx(key, "")[0]) == dest, "Registro residual inesperado.")
        wr.DeleteKeyEx(wr.HKEY_CURRENT_USER, PRODUCT, view)


class RehearsalHost(WindowsHost):
    def __init__(self, run):
        # Do not resolve or read the normal OpenKoda user-data directories in this harness.
        self.roots = (run / "synthetic-profile", run / "synthetic-legacy")
        self.default_dest = run / "program files" / "OpenKoda"
        self.state = run / "journal"
        self.interrupt_after_uninstall = False
        self.events = []

    def run(self, release, dest, uninstall_sha=None):
        require(dest == self.default_dest, "Destino fora do ensaio.")
        super().run(release, dest, uninstall_sha)
        self.events.append({"operation": "uninstall" if uninstall_sha else "install", "version": release.version,
                            "nsisExit": 0, "observedRegistration": self.registration(dest)})
        state = shared()
        require(not state["shortcuts"] and not state["autostart"], "O estado compartilhado de inicialização mudou.")
        if uninstall_sha and self.interrupt_after_uninstall:
            self.interrupt_after_uninstall = False
            raise Refused("injected_after_successful_uninstall")


def verify_contents(release, dest):
    require(installed(dest, release), "Payload instalado não confere.")
    manifest = read_json(release.manifest, release.anchor)
    archive = release.manifest.parent / Path(manifest["sourceArchive"]).name
    require(file_sha(archive) == manifest["sourceArchiveSha256"], "Arquivo de fontes divergente.")
    import hashlib
    with zipfile.ZipFile(archive) as package:
        for file in ("AVISOS.txt", "COPYRIGHT-library.html", "fontes-MPL.zip"):
            with package.open("docs/avisos/" + file) as source:
                expected = hashlib.sha256(source.read()).hexdigest()
            require(file_sha(dest / "avisos" / file) == expected, "Recurso instalado divergente.")


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    for name in ("older", "newer"):
        cli.add_argument("--" + name, required=True, help="Caminho absoluto ao release.json")
        cli.add_argument("--" + name + "-sha256", required=True)
    cli.add_argument("--execute", action="store_true")
    args = cli.parse_args()
    root = absolute(Path(__file__).resolve().parents[1] / ".native-runs")
    run = root / ("run-" + uuid.uuid4().hex)
    host = RehearsalHost(run)
    old = Release.load({"manifest": args.older, "sha256": args.older_sha256})
    new = Release.load({"manifest": args.newer, "sha256": args.newer_sha256})
    before = shared()
    require(not any(before.values()), "Existe instalação, cadastro, atalho ou entrada de inicialização. Estado preservado.")
    host.ready()
    print("Preflight aprovado. Perfis sintéticos, registro e atalhos livres.", flush=True)
    if not args.execute:
        return
    root.mkdir(exist_ok=True)
    run.mkdir(exist_ok=False)
    host.default_dest.parent.mkdir()
    host.roots[0].mkdir()
    (host.roots[0] / "profiles.lock").touch()
    db = host.roots[0] / "openkoda.sqlite3"
    with closing(sqlite3.connect(db)) as connection:
        connection.execute("CREATE TABLE evidence (value TEXT)")
        connection.execute("INSERT INTO evidence VALUES ('synthetic marker')")
        connection.commit()
    before_data = fingerprint(host.roots)
    tool = Recovery(host)
    result = {"passed": False, "scope": "NSIS installed programs; application never launched",
              "personalProfileOpened": False, "beforeShared": before, "oldManifestSha256": old.anchor,
              "newManifestSha256": new.anchor, "events": host.events, "programDirectory": str(host.default_dest)}
    try:
        with tool.session():
            dest = host.default_dest
            def do(op, current, target):
                token, _ = tool.prepare(op, current, target, dest)
                return tool.apply(token)

            result["stage"] = "install"
            result["initial"] = do("install", old, old)
            verify_contents(old, dest)
            result["stage"] = "repair"
            displaced = run/"intentionally-displaced-old.exe"
            require(not displaced.exists(), "Destino de injeção já existe.")
            os.rename(dest/"openkoda.exe", displaced)
            result["repaired"] = do("repair", old, old)
            verify_contents(old, dest)
            result["stage"] = "upgrade"
            result["upgraded"] = do("upgrade", old, new)
            verify_contents(new, dest)
            result["stage"] = "incompatible data"
            new_evidence = host.roots[0]/"new-version-only.json"
            new_evidence.write_bytes(b'{"synthetic":"new version"}')
            try:
                tool.prepare("rollback", new, old, dest)
            except Refused as error:
                require("dados diferem" in str(error), "Bloqueio inesperado para dados modificados.")
            else:
                raise Refused("Retorno indevido com dados modificados.")
            require(installed(dest, new) and new_evidence.is_file(), "Dados modificados ou binário perdidos.")
            result["incompatibleBlocked"] = True
            os.rename(new_evidence, run/"preserved-new-version-only.json")
            result["stage"] = "interrupt and resume"
            token, _ = tool.prepare("rollback", new, old, dest)
            host.interrupt_after_uninstall = True
            try:
                tool.apply(token)
            except Refused as error:
                require(str(error) == "injected_after_successful_uninstall", "Falha inesperada no desinstalador.")
            else:
                raise Refused("A interrupção não ocorreu.")
            require(host.registration(dest) is None and tool.active.exists(), "Diário ou limite de retomada incorreto.")
            result["interruption"] = "after verified real uninstaller exited and before next installer"
            result["rollback"] = tool.apply(token)
            verify_contents(old, dest)
            require(fingerprint(host.roots) == before_data, "A base sintética foi alterada.")
            result["stage"] = "cleanup"
            owned_directory(dest)
            require(host.registration(dest) == old.version and installed(dest, old), "A instalação de ensaio não confere.")
            host.run(old, dest, file_sha(dest/"uninstall.exe"))
            require(host.registration(dest) is None and not (dest/"openkoda.exe").exists(), "Desinstalação incompleta.")
            clear_owned_location(dest)
            host.ready()
            result["afterShared"] = shared()
            require(result["afterShared"] == before and fingerprint(host.roots) == before_data,
                    "Estado final diferente do inicial.")
            result["stage"] = "complete"
            result["passed"] = True
    except BaseException as failure:
        result["failure"] = str(failure)
        raise
    finally:
        save(run/"result.json", result)
        print(run/"result.json", flush=True)


if __name__ == "__main__":
    main()

