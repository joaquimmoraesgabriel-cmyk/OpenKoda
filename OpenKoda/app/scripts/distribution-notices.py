"""Generate offline notices for the Windows desktop distribution from its locked inputs."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
NOTICE_NAME = re.compile(r'^(?:licen[sc]es?|copying|copyright|notices?|attributions?|third[-_ .]?party[-_ .]?notices?)(?:$|[-_ .].*)', re.I)
LOADER = 'WebView2LoaderStatic.lib'
PLUGIN = 'nsis_tauri_utils.dll'
UPSTREAM = {
    LOADER: {
        'id': 'sdk/Microsoft.Web.WebView2@1.0.3800.47/x64',
        'binary': '89c6b872783b8f6c3cedbff618adb42082d455c615453ae10cfc753f1e8f25d8',
        'license': 'LICENSE.txt e NOTICE.txt do pacote Microsoft.Web.WebView2',
        'source': 'https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/1.0.3800.47/microsoft.web.webview2.1.0.3800.47.nupkg',
        'origin': 'NuGet sha256:56c9f26bdd07916a2d1949fb58a5c7e434dfa1173577dca879206050c4e718db; build/native/x64/WebView2LoaderStatic.lib',
        'documents': {
            'WebView2-SDK-1.0.3800.47-LICENSE.txt': '0af8f1b807512aae39c2ac1aa4d0cae65cabecb6fd554b8439a5162a0d6eca55',
            'WebView2-SDK-1.0.3800.47-NOTICE.txt': '106423785c5b7eba0a8e61d1837f2132e9c828e20ad530f565d981c1df60dd90',
        },
    },
    PLUGIN: {
        'id': 'instalador/nsis_tauri_utils@0.5.3',
        'binary': '5ba143b5db4a87d32d6e7802e033330aae56cbceabe0d1e3ba41948385ad4709',
        'license': 'MIT OR Apache-2.0',
        'source': 'https://github.com/tauri-apps/nsis-tauri-utils/releases/tag/nsis_tauri_utils-v0.5.3',
        'origin': 'https://github.com/tauri-apps/nsis-tauri-utils/tree/13d9edd27b69310e108d6fbd49f90992f8a05390',
        'documents': {
            'nsis-tauri-utils-v0.5.3-LICENSE_MIT.txt': '1c1020fa10a6bf318717e82c911bcc54ebdfb9bb280460ae332bcb2f82f57fbe',
            'nsis-tauri-utils-v0.5.3-LICENSE_APACHE-2.0.txt': '0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594',
        },
    },
}

# These packages publish their Cargo.toml.orig and VCS provenance, while
# keeping the license text at the corresponding repository root.
ROOT_LICENSES = (
    {
        'repo': 'https://github.com/dropbox/rust-alloc-no-stdlib',
        'revision': '0a81fd6928ea3b33c8cd484aa4575d50ffb98012',
        'license': 'BSD-3-Clause',
        'text': 'alloc-stdlib-0.3.0-BSD-3-Clause.txt',
        'sha256': 'c0c56f26d9c051cac4d200c34c84e7ae9aaa853e01a982a1df08b09931e518ae',
        'packages': {
            ('alloc-stdlib', '0.3.0'): ('alloc-stdlib', '13c6e8d5ec2e75ae6b66542498e049e1100e71ef25e29ccb9cb083b40d6279ea'),
        },
    },
    {
        'repo': 'https://github.com/wravery/webview2-rs',
        'revision': 'edc2caf886175ccaebe86078c9cfe1ae2a187328',
        'license': 'MIT',
        'text': 'webview2-rs-0.39.1-MIT.txt',
        'sha256': '0dcf41516e608bbcb6cdc5229feb7b86fe4a643b85e7df251133c93408fdac73',
        'packages': {
            ('webview2-com', '0.39.1'): ('crates/webview2-com', 'bee8a11a0f7c152ff835ea5ca64bcb5558e8a6079be56eccc4086dddb383288c'),
            ('webview2-com-sys', '0.39.1'): ('crates/bindings', '00229c125c597e4f0ddb9e598738ee5b38a9086fd59eea6d2577cdda5f8e5875'),
        },
    },
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def normal_graph(metadata):
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    macros = {p['id'] for p in metadata['packages'] if any('proc-macro' in t['kind'] for t in p['targets'])}
    pending = [metadata['resolve']['root']]
    seen = set()
    while pending:
        package = pending.pop()
        if package in seen or package in macros:
            continue
        seen.add(package)
        for edge in nodes[package]['deps']:
            if any(kind['kind'] is None for kind in edge['dep_kinds']):
                pending.append(edge['pkg'])
    return sorted((p for p in metadata['packages'] if p['id'] in seen and p['source']),
                  key=lambda p: (p['name'], p['version']))


def npm_graph(lock):
    packages = lock['packages']
    # Vite provides the modulepreload polyfill included in the production entry.
    selected = {'node_modules/vite'}
    pending = [('node_modules/' + name) for name in packages['']['dependencies']]
    while pending:
        key = pending.pop()
        if key in selected:
            continue
        selected.add(key)
        package = packages[key]
        names = set(package.get('dependencies', {})) | set(package.get('peerDependencies', {}))
        for name in names:
            parent = key
            while True:
                child = '/'.join(filter(None, (parent, 'node_modules', name)))
                if child in packages:
                    pending.append(child)
                    break
                if not parent:
                    if not package.get('peerDependenciesMeta', {}).get(name, {}).get('optional'):
                        raise ValueError(f'Pacote transitivo indisponível: {key} → {name}')
                    break
                parent = parent.rpartition('node_modules/')[0].rstrip('/')
    return sorted(selected)


def add_document(texts, files, label, data):
    if len(data) > 2_000_000 or b'\x00' in data:
        raise ValueError(f'Arquivo de aviso inesperado: {label}')
    data.decode('utf-8-sig')  # All printed notices must be readable.
    key = sha(data)
    texts[key] = data
    files.append((label, key))


def pinned_notice(texts, files, filename, expected, label):
    content = (ROOT / 'docs/avisos/upstream' / filename).read_bytes()
    if sha(content) != expected:
        raise ValueError(f'Texto de origem divergente: {filename}')
    add_document(texts, files, label, content)


def crate_root_license(tar, name, version, texts, files):
    for spec in ROOT_LICENSES:
        pin = spec['packages'].get((name, version))
        if not pin:
            continue
        prefix = f'{name}-{version}/'
        try:
            info = json.load(tar.extractfile(prefix + '.cargo_vcs_info.json'))
            published = tomllib.load(tar.extractfile(prefix + 'Cargo.toml'))['package']
            original = tar.extractfile(prefix + 'Cargo.toml.orig').read()
        except KeyError:
            return
        path, expected = pin
        if (info.get('git', {}).get('sha1') != spec['revision'] or info['git'].get('dirty')
                or info.get('path_in_vcs') != path or sha(original) != expected
                or published.get('repository') != spec['repo'] or published.get('license') != spec['license']):
            return
        url = f'{spec["repo"]}/blob/{spec["revision"]}/LICENSE'
        pinned_notice(texts, files, spec['text'], spec['sha256'], f'Código Rust de {path}: {url}')
        return


def cargo_records(metadata, texts, mpl_sources, binaries):
    lock = tomllib.loads((ROOT / 'src-tauri/Cargo.lock').read_text(encoding='utf-8'))
    checks = {(p['name'], p['version']): p['checksum'] for p in lock['package'] if 'checksum' in p}
    records = []
    for package in normal_graph(metadata):
        name, version = package['name'], package['version']
        source = Path(package['manifest_path']).parent
        archive_name = f'{name}-{version}.crate'
        archive = source.parents[2] / 'cache' / source.parent.name / archive_name
        packed = archive.read_bytes()
        checksum = sha(packed)
        if checks[name, version] != checksum:
            raise ValueError(f'Assinatura de conteúdo divergente do Cargo.lock: {archive_name}')
        if 'MPL-2.0' in (package['license'] or ''):
            mpl_sources[archive_name] = packed
        files = []
        prefix = name + '-' + version
        with tarfile.open(fileobj=io.BytesIO(packed), mode='r:gz') as tar:
            for entry in tar:
                path = PurePosixPath(entry.name)
                if not entry.isfile() or path.parts[0] != prefix:
                    continue
                relative = str(path.relative_to(prefix))
                if name == 'webview2-com-sys' and relative == 'x64/' + LOADER:
                    binaries[LOADER] = sha(tar.extractfile(entry).read())
                if name == 'libsqlite3-sys' and relative.startswith('sqlcipher/'):
                    continue  # The desktop selects the bundled SQLite feature.
                if NOTICE_NAME.match(path.name) or relative == package['license_file']:
                    data = tar.extractfile(entry).read(2_000_001)
                    add_document(texts, files, relative, data)
                elif name == 'selectors' and relative == 'lib.rs':
                    data = tar.extractfile(entry).read(2048)
                    header = re.search(rb'/\*[^*]*\*+(?:[^/*][^*]*\*+)*/', data)
                    if header and b'Mozilla Public' in header[0]:
                        add_document(texts, files, 'lib.rs (cabeçalho MPL)', header[0])
                elif name == 'libsqlite3-sys' and relative == 'sqlite3/sqlite3.c':
                    data = tar.extractfile(entry).read(256_000)
                    for header in re.finditer(rb'/\*.*?\*/', data, re.S):
                        if b'disclaims copyright' in header[0].lower():
                            add_document(texts, files, 'sqlite3/sqlite3.c (dedicação do autor)', header[0])
                            break
            if not files:
                crate_root_license(tar, name, version, texts, files)
        records.append({'id': 'cargo/' + name + '@' + version, 'license': package['license'],
                        'source': f'https://static.crates.io/crates/{name}/{archive_name}',
                        'integrity': 'sha256:' + checksum, 'files': sorted(files)})
    return records


def npm_records(texts):
    lock = read_json(ROOT / 'package-lock.json')
    records = []
    for key in npm_graph(lock):
        locked = lock['packages'][key]
        root = ROOT / key
        actual = read_json(root / 'package.json')
        if actual['version'] != locked['version'] or actual['name'] != key.rsplit('node_modules/', 1)[1]:
            raise ValueError(f'Pacote instalado divergente: {key}')
        files = []
        for path in sorted(root.iterdir()):
            if path.is_file() and NOTICE_NAME.match(path.name):
                add_document(texts, files, path.name, path.read_bytes())
        records.append({'id': 'npm/' + actual['name'] + '@' + actual['version'],
                        'license': locked.get('license', actual.get('license')),
                        'source': locked['resolved'], 'integrity': locked['integrity'], 'files': files})
    return records


def source_zip(mpl_sources):
    out = io.BytesIO()
    with zipfile.ZipFile(out, 'w') as archive:
        for name, data in sorted(mpl_sources.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = 0o100644 << 16
            archive.writestr(entry, data)  # .crate already contains a gzip archive.
    return out.getvalue()


def upstream_records(binaries, texts):
    """Match the actual binary before assigning the corresponding publisher's notices."""
    records, gaps = [], []
    for name, spec in UPSTREAM.items():
        actual = binaries.get(name)
        if actual != spec['binary']:
            gaps.append(f'{name}: conferir os avisos para a identidade local {actual or "indisponível"}.')
            continue
        files = []
        for filename, expected in spec['documents'].items():
            pinned_notice(texts, files, filename, expected, filename)
        records.append({'id': spec['id'], 'license': spec['license'], 'source': spec['source'],
                        'integrity': f'{spec["origin"]}\n  {name} sha256:{actual}', 'files': files})
    return records, gaps


def build(args):
    command = ['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform',
               'x86_64-pc-windows-msvc', '--features', 'custom-protocol', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml')]
    if args.cargo_config:
        command += ['--config', str(args.cargo_config.resolve())]
    metadata = json.loads(subprocess.check_output(command, cwd=ROOT))
    texts, sources, binaries = {}, {}, {}
    records = cargo_records(metadata, texts, sources, binaries) + npm_records(texts)
    rust = subprocess.check_output(['rustc', '--version'], text=True).strip()
    sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    rust_notices = (sysroot / 'share/doc/rust/COPYRIGHT-library.html').read_bytes()
    nsis = args.nsis or Path(os.environ['LOCALAPPDATA']) / 'tauri/NSIS'
    files = []
    add_document(texts, files, 'COPYING', (nsis / 'COPYING').read_bytes())
    records.append({'id': 'instalador/NSIS', 'license': 'Termos discriminados em COPYING',
                    'source': 'Distribuição NSIS usada pelo Tauri no Windows',
                    'integrity': 'makensis.exe sha256:' + sha((nsis / 'makensis.exe').read_bytes()), 'files': files})
    binaries[PLUGIN] = sha((nsis / 'Plugins/x86-unicode/additional' / PLUGIN).read_bytes())
    upstream, gaps = upstream_records(binaries, texts)
    records.extend(upstream)
    lines = ['Avisos de terceiros do OpenKoda ' + read_json(ROOT / 'package.json')['version'], '',
             'O índice identifica o pacote, a versão e os arquivos de origem de cada texto. Procure o SHA-256 indicado para localizar o texto integral na segunda parte deste documento.',
             'O grafo Cargo segue dependências normais resolvidas para Windows x64 com custom-protocol. Exclui ramos exclusivos de build, testes e macros de compilação. O linker pode descartar código. A lista web cobre as dependências de produção e o auxiliar de carregamento de módulos do Vite.',
             'Os arquivos .crate foram verificados pelo SHA-256 do Cargo.lock. Os avisos npm foram lidos dos pacotes locais nas versões do package-lock.json. A integridade npm exibida identifica o arquivo de distribuição registrado no lockfile.', '',
             f'Biblioteca padrão de {rust}. Abra COPYRIGHT-library.html (sha256:{sha(rust_notices)}).',
             'Fontes das crates que declaram MPL-2.0: fontes-MPL.zip. Cada .crate contém a fonte publicada na versão indicada no índice.',
             'O aplicativo usa o runtime WebView2 do Windows. A distribuição Kali é preparada separadamente pelo usuário.', '',
             'Itens para revisão de distribuição', *gaps,
             *[r['id'] + ' — sem texto de licença no pacote consultado.' for r in records if not r['files']], '',
             'Índice de componentes', '']
    for record in records:
        lines.extend([record['id'], '  ' + str(record['license']), '  ' + record['source'], '  ' + record['integrity']])
        lines.extend('  ' + path + ' → ' + key for path, key in record['files'])
        lines.append('')
    output = bytearray(('\n'.join(lines) + '\nTextos dos avisos\n').encode())
    for key, data in sorted(texts.items()):
        output.extend(f'\n===== SHA-256 {key} =====\n'.encode() + data + b'\n')
    resources = {'AVISOS.txt': bytes(output), 'COPYRIGHT-library.html': rust_notices, 'fontes-MPL.zip': source_zip(sources)}
    return resources, {'componentes': len(records), 'textosDistintos': len(texts), 'fontesMPL': len(sources),
                       'semTexto': [r['id'] for r in records if not r['files']], 'avisosDeBinariosPendentes': gaps}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cargo-config', type=Path)
    parser.add_argument('--nsis', type=Path, help='Pasta local do NSIS usada pelo empacotador')
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/avisos')
    parser.add_argument('--check', action='store_true', help='Conferir os avisos já gerados')
    args = parser.parse_args()
    resources, summary = build(args)
    if not args.check:
        args.output.mkdir(parents=True, exist_ok=True)
    for name, data in resources.items():
        target = args.output / name
        if args.check:
            if not target.is_file() or target.read_bytes() != data:
                parser.exit(1, f'Aviso ausente ou divergente: {target}\n')
        else:
            target.write_bytes(data)
    print(json.dumps({**summary, 'bytes': {n: len(d) for n, d in resources.items()}}, ensure_ascii=False))


if __name__ == '__main__':
    main()
