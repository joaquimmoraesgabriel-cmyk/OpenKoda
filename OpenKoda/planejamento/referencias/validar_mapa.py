"""Valida a documentação e os diagramas; não executa código do produto proposto."""
import json
import hashlib
from pathlib import Path
import re
import struct
import xml.etree.ElementTree as ET

base = Path(__file__).resolve().parents[1]
product_root = base.parent
errors = []
documents = sorted(base.rglob('*.md')) + [product_root / 'README.md', product_root / 'GLOSSARY.md'] + sorted((product_root / 'docs' / 'adr').glob('*.md'))
links = 0
for file in documents:
    text = file.read_text(encoding='utf-8-sig')
    for match in re.findall(r'\[[^\]]*\]\(([^)]+)\)', text):
        target = match.strip('<>').split('#')[0]
        if not target or re.match(r'^(https?://|mailto:)', target):
            continue
        resolved = Path(target) if re.match(r'^[A-Za-z]:[/\\]', target) else file.parent / target
        links += 1
        if not resolved.exists():
            errors.append({'file':str(file),'target':target,'error':'missing link target'})

def strings(obj):
    if isinstance(obj,str):
        yield obj
    elif isinstance(obj,dict):
        for value in obj.values():
            yield from strings(value)
    elif isinstance(obj,list):
        for value in obj:
            yield from strings(value)

diagrams = []
for source in sorted((base / 'diagramas').glob('*.ofk')):
    scene_path = source.with_suffix('.openflow.json')
    svg_path = source.with_suffix('.svg')
    try:
        scene = json.loads(scene_path.read_text(encoding='utf-8-sig'))
        svg = ET.parse(svg_path).getroot()
        dsl = source.read_text(encoding='utf-8-sig').strip()
        if not any(value.strip() == dsl for value in strings(scene)):
            errors.append({'file':str(scene_path),'error':'DSL source differs from edit document'})
        if svg.attrib.get('data-openflowkit-document') != scene['id']:
            errors.append({'file':str(svg_path),'error':'SVG/document identity mismatch'})
        diagrams.append({'name':source.stem,'documentId':scene['id'],'svgBytes':svg_path.stat().st_size,'sourceBytes':source.stat().st_size})
    except Exception as exc:
        errors.append({'file':str(source),'error':str(exc)})
if len(diagrams) != 10:
    errors.append({'error':'expected ten maps','actual':len(diagrams)})
adr_status = []
for file in sorted((product_root / 'docs' / 'adr').glob('*.md')):
    expected = 'accepted' if file.name == '0004-tauri-como-framework-desktop.md' else 'proposed'
    if f'status: {expected}' not in file.read_text(encoding='utf-8-sig'):
        errors.append({'file':str(file),'error':f'ADR status differs from recorded decision: {expected}'})
    adr_status.append({'name':file.name,'expectedStatus':expected})
acceptance = (base / 'FASES-E-VALIDACAO.md').read_text(encoding='utf-8-sig')
criteria = re.findall(r'^\| (AC-\d{2}) \|', acceptance, re.M)
if criteria != [f'AC-{i:02d}' for i in range(1,29)]:
    errors.append({'error':'acceptance criterion ids incomplete or duplicate numbering issue','ids':criteria})
security = (base / 'PERMISSOES-E-SEGURANCA.md').read_text(encoding='utf-8-sig')
requirements = re.findall(r'^\| (SEC-\d{2}) \|', security, re.M)
if requirements != [f'SEC-{i:02d}' for i in range(1,24)]:
    errors.append({'error':'security requirement ids incomplete or duplicated','ids':requirements})
visual = (base / 'VISUAL-E-TELAS.md').read_text(encoding='utf-8-sig')
visual_ids = re.findall(r'^\| (V-\d{2}) \|', visual, re.M)
image_names = ['painel-openkoda-v2.png', '01-nova-avaliacao-v1.png', '02-kali-e-ferramentas-v1.png', '03-achados-v1.png', '04-projetos-v1.png', '05-configuracoes-v1.png']
if visual_ids != [f'V-{i:02d}' for i in range(1,7)]:
    errors.append({'error':'approved visual ids incomplete or duplicated','ids':visual_ids})
images = []
for index,name in enumerate(image_names,1):
    image_path = base / 'conceitos' / name
    try:
        data = image_path.read_bytes()
        if data[:8] != b'\x89PNG\r\n\x1a\n' or data[12:16] != b'IHDR':
            raise ValueError('invalid PNG header')
        width,height = struct.unpack('>II',data[16:24])
        if not width or not height or name not in visual:
            raise ValueError('empty image or missing gallery reference')
        images.append({'id':f'V-{index:02d}','name':name,'width':width,'height':height,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
    except Exception as exc:
        errors.append({'file':str(image_path),'error':str(exc)})
result = {'valid':not errors,'markdownDocuments':len(documents),'localLinks':links,'diagrams':diagrams,'adrStatus':adr_status,'approvedImages':images,'acceptanceCriteria':len(criteria),'securityRequirements':len(requirements),'errors':errors,'scope':'Document links, serialization, ADR status, DSL/SVG identities, PNG metadata/hashes and requirement IDs only. No product behavior, accessibility or security control was tested.'}
(base / 'referencias' / 'validacao.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
raise SystemExit(bool(errors))
