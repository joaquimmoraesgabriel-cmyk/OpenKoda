"""Recuperação local de instalações NSIS do OpenKoda. Python 3.10+."""
import argparse
from contextlib import contextmanager, ExitStack
import ctypes
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import uuid


class Refused(Exception):
    pass


def require(condition, message):
    if not condition:
        raise Refused(message)


def regular(path, directory=False):
    info = path.lstat()
    require(not info.st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT
            if hasattr(info, "st_file_attributes") else not path.is_symlink(),
            "Link ou ponto de reanálise encontrado.")
    if directory:
        require(stat.S_ISDIR(info.st_mode), "Era esperada uma pasta comum.")
    else:
        require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "Era esperado um arquivo comum de vínculo único.")


def absolute(value):
    path = Path(value)
    require(path.is_absolute() and ".." not in path.parts, "Informe um caminho absoluto, sem '..'.")
    if os.name == "nt":
        require(re.fullmatch(r"[A-Za-z]:", path.drive) and
                all(not re.search(r'[<>:"|?*\x00-\x1f]|[. ]$', part) for part in path.parts[1:]),
                "Use um caminho local do Windows sem caracteres ambíguos.")
    for parent in reversed(path.parents):
        if parent.exists() or parent.is_symlink():
            regular(parent, directory=True)
    if path.exists() or path.is_symlink():
        regular(path, path.is_dir())
    return path


@contextmanager
def guarded_file(path, *, exclusive=False, create=False):
    """Windows denies other writers, renames and removal until this handle closes."""
    absolute(path)
    if os.name != "nt":
        with path.open("a+b" if create else "rb") as stream:
            yield stream
        return
    import msvcrt
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    open_file = kernel.CreateFileW
    open_file.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p,
                          wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    open_file.restype = wintypes.HANDLE
    handle = open_file(str(path), 0x80000000, 0 if exclusive else 1, None,
                       4 if create else 3, 0x00200000, None)  # OPEN_ALWAYS/EXISTING; OPEN_REPARSE_POINT
    if handle == ctypes.c_void_p(-1).value:
        raise Refused("Não foi possível reservar um arquivo. Feche o aplicativo, instaladores e processos que usam os dados.")
    fd = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    with os.fdopen(fd, "rb") as stream:
        regular(path)
        yield stream


def stream_sha(stream):
    digest = hashlib.sha256()
    for block in iter(lambda: stream.read(1024 * 1024), b""):
        digest.update(block)
    return digest.hexdigest()


def file_sha(path):
    with guarded_file(path) as stream:
        return stream_sha(stream)


def read_json(path, expected=None):
    with guarded_file(path) as stream:
        raw = stream.read(4 * 1024 * 1024 + 1)
    require(len(raw) <= 4 * 1024 * 1024, "JSON excede o limite.")
    if expected is not None:
        require(re.fullmatch("[0-9a-f]{64}", expected) and hashlib.sha256(raw).hexdigest() == expected,
                "O SHA-256 do manifesto ou recibo diverge da referência aprovada.")
    return json.loads(raw.decode("utf-8-sig"))


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=True, separators=(",", ":")).encode()


def save(path, value):
    absolute(path)
    temporary = path.with_name("." + uuid.uuid4().hex + ".tmp")
    with temporary.open("xb") as stream:
        stream.write(canonical(value))
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def version(value):
    require(isinstance(value, str) and re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value),
            "Versão local não reconhecida.")
    return tuple(map(int, value.split(".")))


@dataclass(frozen=True)
class Release:
    manifest: Path
    anchor: str
    version: str
    setup: Path
    setup_sha: str
    payload_sha: str

    @property
    def ref(self):
        return {"manifest": str(self.manifest), "sha256": self.anchor}

    @classmethod
    def load(cls, ref):
        path = absolute(ref["manifest"])
        require(path.name == "release.json", "Selecione o release.json preservado.")
        data = read_json(path, ref["sha256"])
        number = data["version"]
        require(version(number) in {(0, 1, 6), (0, 1, 7), (0, 1, 8), (0, 1, 9)} and data["schema"] == 8,
                "É necessário um pacote local de perfil 8 com recibo de extração.")
        setup = path.parent / f"OpenKoda_{number}_x64-setup.exe"
        require(Path(data["installer"]).name == setup.name and file_sha(setup) == data["sha256"],
                "Instalador ausente ou alterado.")
        receipt_ref = data["receipts"]["packageExtraction"]
        receipt = read_json(path.parent / "receipts" / "packageExtraction.json", receipt_ref["sha256"])
        release, payload = receipt["newRelease"], receipt["newPayload"]
        verification = payload["verification"]
        require(receipt["passed"] is True and receipt["mode"] == "ExtractOnly"
                and release.get("version", number) == number and release["installerSha256"] == data["sha256"]
                and payload.get("entry", "openkoda.exe").replace("\\", "/").split("/")[-1].lower() == "openkoda.exe"
                and verification["verified"] is True and verification["sourceSha256"] == data["sourceBinarySha256"]
                and release["sourceBinarySha256"] == data["sourceBinarySha256"]
                and re.fullmatch("[0-9a-f]{64}", verification["sha256"]),
                "O recibo não vincula instalador, versão e executável empacotado.")
        return cls(path, ref["sha256"], number, setup, data["sha256"], verification["sha256"])


def overlap(a, b):
    return a == b or a in b.parents or b in a.parents


def fingerprint(roots):
    """Only the aggregate leaves memory. An absent root differs from an empty root."""
    digest = hashlib.sha256()
    for index, base in enumerate(roots):
        base = absolute(base)
        digest.update(canonical([index, base.exists()]))
        if not base.exists():
            continue
        regular(base, directory=True)
        todo = [base]
        while todo:
            directory = todo.pop()
            for child in sorted(directory.iterdir(), key=lambda p: p.name):
                relative = child.relative_to(base).as_posix()
                if index == 0 and relative == "profiles.lock":
                    continue
                is_dir = child.is_dir()
                regular(child, is_dir)
                digest.update(canonical([relative, None if is_dir else file_sha(child)]))
                if is_dir:
                    todo.append(child)
    return digest.hexdigest()


def owned_directory(dest):
    allowed = {"openkoda.exe", "uninstall.exe", "avisos", "avisos/avisos.txt",
               "avisos/copyright-library.html", "avisos/fontes-mpl.zip", "recuperacao",
               "recuperacao/windows_release_recovery.py", "recuperacao/recuperacao-windows.md"}
    absolute(dest)
    require(len(dest.parts) >= 3, "Destino excessivamente amplo.")
    if not dest.exists():
        return
    regular(dest, directory=True)
    for path in dest.rglob("*"):
        relative = path.relative_to(dest).as_posix().lower()
        require(relative in allowed, "Destino contém entradas externas à instalação. Nenhum arquivo foi removido.")
        regular(path, directory=relative in {"avisos", "recuperacao"})


@dataclass(frozen=True)
class PartialRegistration:
    version: str | None


def registration_fields(dest, values):
    location = values.get("InstallLocation")
    command = values.get("UninstallString")
    display = values.get("DisplayVersion")
    if location is not None:
        require(absolute(location.strip('"')) == dest, "InstallLocation aponta para outro destino.")
    if command is not None:
        require(command.casefold() == f'"{dest / "uninstall.exe"}"'.casefold(), "UninstallString inesperado.")
    if display is not None:
        display = re.sub(r"^(\d+\.\d+\.\d+)\.0$", r"\1", display)
        version(display)
    return display if all(v is not None for v in (location, command, display)) else PartialRegistration(display)


def observed_version(registered):
    return registered.version if isinstance(registered, PartialRegistration) else registered


def can_repair(registered, dest, host, release):
    if registered == release.version:
        return True
    return (observed_version(registered) in (None, release.version) and
            dest == host.default_dest and installed(dest, release))


class WindowsHost:
    def __init__(self):
        require(os.name == "nt", "Este fluxo é destinado ao Windows.")
        self.roots = (Path.home() / ".openkoda", Path(os.environ["APPDATA"]) / "com.openkoda.desktop")
        self.state = Path(os.environ["LOCALAPPDATA"]) / "OpenKoda.ReleaseRecovery"
        self.default_dest = Path(os.environ["LOCALAPPDATA"]) / "OpenKoda"

    def ready(self):
        # Keep NSIS offline and avoid asking the installer to close a running application.
        import winreg as wr
        key = r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
        runtime = []
        for hive in (wr.HKEY_LOCAL_MACHINE, wr.HKEY_CURRENT_USER):
            for view in (wr.KEY_WOW64_32KEY, wr.KEY_WOW64_64KEY):
                try:
                    with wr.OpenKey(hive, key, 0, wr.KEY_READ | view) as item:
                        runtime.append(wr.QueryValueEx(item, "pv")[0])
                except FileNotFoundError:
                    continue
        require(any(re.fullmatch(r"[1-9]\d*(?:\.\d+){3}", str(v)) for v in runtime),
                "Instale o WebView2 antes da recuperação offline.")
        tasklist = Path(os.environ["SystemRoot"]) / "System32" / "tasklist.exe"
        import csv
        result = subprocess.run([str(tasklist), "/FO", "CSV", "/NH"], check=True, capture_output=True,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        names = [row[0].casefold() for row in csv.reader(result.stdout.decode("mbcs", "replace").splitlines()) if len(row) >= 5]
        require(names, "Não foi possível verificar os processos do Windows.")
        require(not any(name in {"openkoda.exe", "openkoda-debug.exe", "uninstall.exe", "au_.exe"}
                        or name.startswith("openkoda_") and name.endswith("setup.exe") for name in names),
                "Feche o OpenKoda e aguarde o instalador ou desinstalador terminar.")

    def registration(self, dest):
        import winreg as wr
        product = r"Software\openkoda\OpenKoda"
        uninstall = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda"
        records = []
        for hive in (wr.HKEY_CURRENT_USER, wr.HKEY_LOCAL_MACHINE):
            for view in (wr.KEY_WOW64_32KEY, wr.KEY_WOW64_64KEY):
                for name in (uninstall, product):
                    try:
                        key = wr.OpenKey(hive, name, 0, wr.KEY_READ | view)
                    except FileNotFoundError:
                        continue
                    with key:
                        require(hive == wr.HKEY_CURRENT_USER, "Existe uma instalação de máquina. Este fluxo atende ao usuário atual.")
                        if name == product:
                            try:
                                loc = wr.QueryValueEx(key, "")[0]
                            except FileNotFoundError:
                                continue
                            require(absolute(str(loc)) == dest, "O cadastro de localização aponta para outro destino.")
                        else:
                            values = {}
                            for field in ("InstallLocation", "UninstallString", "DisplayVersion"):
                                try:
                                    values[field] = wr.QueryValueEx(key, field)[0]
                                except FileNotFoundError:
                                    pass
                            records.append(registration_fields(dest, values))
        versions = {observed_version(record) for record in records} - {None}
        require(len(versions) <= 1, "As versões registradas nas duas visões do Windows divergem.")
        if not records:
            return None
        return next((record for record in records if isinstance(record, str)), PartialRegistration(next(iter(versions), None)))

    def run(self, release, dest, uninstall_sha=None):
        self.ready()
        uninstalling = uninstall_sha is not None
        executable = dest / "uninstall.exe" if uninstalling else release.setup
        expected = uninstall_sha if uninstalling else release.setup_sha
        # NSIS requires the destination as the last raw argument, even with spaces.
        arguments = f"/S /UPDATE _?={dest}" if uninstalling else f"/S /NS /UPDATE /D={dest}"
        command = subprocess.list2cmdline([str(executable)]) + " " + arguments
        with guarded_file(executable) as stream:
            require(stream_sha(stream) == expected, "O executável da operação mudou desde a revisão.")
            result = subprocess.run(command, executable=str(executable), cwd=dest.parent,
                                    creationflags=subprocess.CREATE_NO_WINDOW, check=False)
        require(result.returncode == 0, f"O NSIS encerrou com código {result.returncode}. O diário foi preservado.")


def installed(dest, release):
    binary = dest / "openkoda.exe"
    return binary.is_file() and file_sha(binary) == release.payload_sha


def history_for(previous, target, dest):
    return {"older": previous.ref, "newer": target.ref, "destination": str(dest)}


def rescue_ref(plan):
    # Repair the newest of the reviewed versions, including an interrupted first installation.
    return plan["from"] if plan["operation"] == "rollback" else plan["to"]


class Recovery:
    def __init__(self, host):
        self.host = host
        self.state = host.state
        absolute(self.state)
        self.state.mkdir(exist_ok=True)
        self.active = self.state / "active.json"
        self.previous = self.state / "checkpoint.json"

    def optional(self, path):
        return read_json(path) if path.exists() else None

    @contextmanager
    def session(self):
        with guarded_file(self.state / "recovery.lock", exclusive=True, create=True):
            yield

    def context(self, old, new, destination):
        dest = absolute(destination)
        protected = [self.state, *self.host.roots, old.manifest.parent, new.manifest.parent]
        require(all(not overlap(dest, absolute(other)) for other in protected), "Destino sobreposto aos dados, diário ou pacotes.")
        require(dest not in Path(__file__).resolve().parents, "Copie a ferramenta para fora do destino instalado antes de continuar.")
        owned_directory(dest)
        self.host.ready()
        return dest, self.host.registration(dest)

    def prepare(self, op, old, new, destination):
        dest, registered = self.context(old, new, destination)
        pending = self.optional(self.active)
        checkpoint = self.optional(self.previous)
        recovery_of = None
        if pending:
            plan = pending["plan"]
            require(op == "repair" and rescue_ref(plan) == old.ref == new.ref and plan["destination"] == str(dest)
                    and observed_version(registered) in (None, plan["fromVersion"], plan["toVersion"]),
                    "Existe operação pendente. Retome-a pelo identificador registrado.")
            recovery_of = pending["id"]
        elif op == "repair":
            require(old.ref == new.ref and can_repair(registered, dest, self.host, old),
                    "O reparo exige registro correspondente ou binário conferido no destino padrão.")
        elif op == "install":
            require(old.ref == new.ref and registered is None and dest == self.host.default_dest
                    and (not dest.exists() or not any(dest.iterdir())),
                    "A instalação acompanhada começa em um destino padrão vazio, sem registro.")
        elif op == "upgrade":
            require(version(new.version) > version(old.version) and registered == old.version and installed(dest, old),
                    "O binário, registro atual e direção de atualização precisam corresponder.")
        elif op == "rollback":
            require(checkpoint and checkpoint["pair"] == history_for(new, old, dest)
                    and registered == old.version and installed(dest, old),
                    "O retorno exige o ponto pré-upgrade desta instalação, dados e par de pacotes.")
            require(file_sha(dest / "uninstall.exe") == checkpoint["uninstallerSha256"],
                    "O desinstalador diverge da atualização acompanhada.")
        else:
            raise Refused("Operação desconhecida.")
        lock = self.host.roots[0] / "profiles.lock"
        if op in ("upgrade", "rollback"):
            require(lock.is_file(), "Abra e feche a versão estável antes de acompanhar sua atualização.")
        with ExitStack() as stack:
            if lock.exists():
                stack.enter_context(guarded_file(lock, exclusive=True))
            before = fingerprint(self.host.roots)
        if op == "rollback":
            require(before == checkpoint["data"], "Os dados diferem do ponto pré-upgrade. O retorno do binário está bloqueado.")
        plan = {"operation": op, "review": uuid.uuid4().hex, "from": old.ref, "to": new.ref, "fromVersion": old.version, "toVersion": new.version,
                "destination": str(dest), "data": before, "recoveryOf": recovery_of}
        token = hashlib.sha256(canonical(plan)).hexdigest()
        save(self.state / (token + ".json"), plan)
        return token, plan

    def apply(self, token):
        require(re.fullmatch("[0-9a-f]{64}", token), "Identificador inválido.")
        plan = read_json(self.state / (token + ".json"))
        require(hashlib.sha256(canonical(plan)).hexdigest() == token, "Plano alterado após revisão.")
        old, new = Release.load(plan["from"]), Release.load(plan["to"])
        dest, registered = self.context(old, new, plan["destination"])
        op = plan["operation"]
        pending = self.optional(self.active)
        if not pending and (self.state / (token + ".done.json")).exists():
            require(registered == new.version and installed(dest, new), "Recibo histórico. O destino já mudou.")
            return read_json(self.state / (token + ".done.json"))
        checkpoint = self.optional(self.previous)
        lock = self.host.roots[0] / "profiles.lock"
        require(op in ("install", "repair") or lock.is_file(), "O bloqueio nativo dos perfis está ausente.")
        with ExitStack() as stack:
            if lock.exists():
                stack.enter_context(guarded_file(lock, exclusive=True))
            before = fingerprint(self.host.roots)
            require(before == plan["data"], "Os dados mudaram. Revise novamente antes de prosseguir.")
            if not pending or pending["id"] != token:
                rescue_versions = []
                if pending:
                    parent = pending["plan"]
                    require(op == "repair" and pending["id"] == plan["recoveryOf"]
                            and rescue_ref(parent) == old.ref == new.ref and parent["destination"] == str(dest),
                            "Outro plano está em execução.")
                    rescue_versions = [parent["fromVersion"], parent["toVersion"]]
                    require(observed_version(registered) in [None, *rescue_versions], "Cadastro inesperado.")
                else:
                    require(plan["recoveryOf"] is None, "O diário anterior mudou.")
                    if op == "install":
                        require(registered is None and dest == self.host.default_dest
                                and (not dest.exists() or not any(dest.iterdir())), "O destino deixou de estar vazio.")
                    elif op == "repair":
                        require(can_repair(registered, dest, self.host, old), "O destino do reparo mudou.")
                    else:
                        require(registered == old.version, "A versão registrada mudou.")
                        require(installed(dest, old), "O binário atual mudou após a revisão.")
                if op == "rollback":
                    require(checkpoint and checkpoint["pair"] == history_for(new, old, dest)
                            and before == checkpoint["data"] and file_sha(dest / "uninstall.exe") == checkpoint["uninstallerSha256"],
                            "O ponto pré-upgrade ou desinstalador mudou.")
                pending = {"id": token, "plan": plan, "stage": "uninstall" if op == "rollback" else "install",
                           "rescueVersions": rescue_versions}
                save(self.active, pending)
            if pending["stage"] == "uninstall":
                require(checkpoint and checkpoint["pair"] == history_for(new, old, dest) and before == checkpoint["data"],
                        "O ponto de retorno divergiu.")
                # A crash may have happened on either side of UninstallString removal.
                if registered == old.version or isinstance(registered, PartialRegistration) and registered.version in (None, old.version):
                    self.host.run(old, dest, checkpoint["uninstallerSha256"])
                require(self.host.registration(dest) is None, "O registro da versão mais nova ainda existe.")
                pending["stage"] = "install"
                save(self.active, pending)
            owned_directory(dest)
            registered = self.host.registration(dest)
            known = observed_version(registered)
            require(known in (None, old.version, new.version) or op == "repair" and plan["recoveryOf"]
                    and known in pending["rescueVersions"],
                    "Registro inesperado durante a retomada.")
            require(op != "rollback" or known != old.version, "A versão mais nova reapareceu. Retorno interrompido.")
            require(fingerprint(self.host.roots) == before, "Os dados mudaram durante a operação.")
            self.host.run(new, dest)
            owned_directory(dest)
            require(self.host.registration(dest) == new.version and installed(dest, new),
                    "O destino não contém o executável e o registro esperados. Retome o diário.")
            uninstaller_sha = file_sha(dest / "uninstall.exe")
            require(fingerprint(self.host.roots) == before, "Os dados mudaram durante a instalação. Preserve o diário.")
            if op == "upgrade":
                save(self.previous, {"pair": history_for(old, new, dest), "data": before, "uninstallerSha256": uninstaller_sha})
            elif op == "repair" and checkpoint and checkpoint["pair"]["destination"] == str(dest) and checkpoint["pair"]["newer"] == new.ref:
                save(self.previous, {**checkpoint, "uninstallerSha256": uninstaller_sha})
            receipt = {"id": token, "operation": op, "version": new.version, "destination": str(dest),
                       "installedBinarySha256": new.payload_sha, "dataPreserved": True}
            save(self.state / (token + ".done.json"), receipt)
            self.active.unlink()
            return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    cli = parser.add_subparsers(dest="command", required=True)
    plan = cli.add_parser("plan", help="Revisar e guardar uma proposta sem executar NSIS.")
    plan.add_argument("operation", choices=("install", "repair", "upgrade", "rollback"))
    for option in ("from-manifest", "from-sha256", "to-manifest", "to-sha256", "destination"):
        plan.add_argument("--" + option, required=True)
    apply = cli.add_parser("apply", help="Executar ou retomar o plano já revisado.")
    apply.add_argument("--confirm", required=True)
    cli.add_parser("status", help="Consultar somente o diário de recuperação.")
    args = parser.parse_args()
    recovery = Recovery(WindowsHost())
    with recovery.session():
        if args.command == "status":
            result = recovery.optional(recovery.active) or {"pending": False}
        elif args.command == "plan":
            old = Release.load({"manifest": args.from_manifest, "sha256": args.from_sha256.lower()})
            new = Release.load({"manifest": args.to_manifest, "sha256": args.to_sha256.lower()})
            token, plan = recovery.prepare(args.operation, old, new, args.destination)
            result = {"id": token, **plan, "installerFromSha256": old.setup_sha, "installerToSha256": new.setup_sha}
        else:
            result = recovery.apply(args.confirm)
        print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except (Refused, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(str(error) if isinstance(error, Refused) else
              "Falha de leitura, formato ou acesso. Os dados foram preservados para inspeção.", file=sys.stderr)
        sys.exit(1)

