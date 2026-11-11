"""Record exact resolved package versions and declared licenses, offline."""
import argparse
import collections
import json
import pathlib
import subprocess

parser=argparse.ArgumentParser()
parser.add_argument('--cargo-config')
args=parser.parse_args()
root=pathlib.Path(__file__).resolve().parents[1]
command=['cargo','metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc','--manifest-path',str(root/'src-tauri/Cargo.toml')]
if args.cargo_config:
    command+=['--config',str(pathlib.Path(args.cargo_config).resolve())]
metadata=json.loads(subprocess.check_output(command,cwd=root))
records=[]
for package in metadata['packages']:
    if package.get('source'):
        records.append({'ecosystem':'cargo','name':package['name'],'version':package['version'],'declaredLicense':package.get('license'),'repository':package.get('repository')})
lock=json.loads((root/'package-lock.json').read_text(encoding='utf-8'))
for path,package in lock['packages'].items():
    if not path:
        continue
    records.append({'ecosystem':'npm','name':package.get('name') or path.rsplit('node_modules/',1)[-1],'version':package.get('version'),'declaredLicense':package.get('license'),'development':bool(package.get('dev'))})
records.sort(key=lambda p:(p['ecosystem'],p['name'],p['version'] or ''))
report={'format':1,'scope':'Resolved lockfiles. Cargo filtered to x86_64-pc-windows-msvc; npm includes development/build and optional packages. Declaration inventory only; not a legal compatibility determination or vulnerability scan.','packages':records}
destination=root/'docs/DEPENDENCIES.json'
destination.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
counts=collections.Counter(p['ecosystem'] for p in records)
unknown=[p for p in records if not p['declaredLicense']]
summary='# Dependências resolvidas\n\nInventário gerado offline a partir dos lockfiles e metadados locais. Cargo filtrado para Windows x64; npm inclui dependências de desenvolvimento, build e plataformas opcionais. As licenças são declarações dos pacotes; este inventário não substitui textos de licença, análise de compatibilidade ou auditoria de vulnerabilidades.\n\n'
summary+='Versões e licenças: [DEPENDENCIES.json](DEPENDENCIES.json).\n\n'
summary+='\n'.join(f'- {name}: {count} registros.' for name,count in sorted(counts.items()))+'\n'
summary+=f'- Sem declaração de licença no metadado consultado: {len(unknown)}.\n\n'
summary+='Regenerar: `python scripts/dependency-inventory.py` (opcional `--cargo-config .local/cargo-transport.toml`).\n'
(root/'docs/DEPENDENCIES.md').write_text(summary,encoding='utf-8')
print(json.dumps({'path':str(destination),'counts':dict(counts),'missingLicense':len(unknown)}))
