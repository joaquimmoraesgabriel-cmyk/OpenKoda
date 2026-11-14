import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile, readdir} from 'node:fs/promises';
import {join, relative, sep} from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

const dist=fileURLToPath(new URL('../dist/',import.meta.url));
const build=fileURLToPath(new URL('../build.mjs',import.meta.url));
const origin='https://openkoda.example.invalid';

async function collectPages(directory,pages){
 for(const entry of await readdir(directory,{withFileTypes:true})){
  const file=join(directory,entry.name);
  if(entry.isDirectory())await collectPages(file,pages);
  else if(entry.name==='index.html'){
   const route=relative(dist,directory).split(sep).filter(Boolean).join('/');
   pages.set(route?'/'+route+'/':'/',await readFile(file,'utf8'));
  }
 }
}

test('a navegação pública resolve páginas, recursos e seções da compilação',async()=>{
 execFileSync(process.execPath,[build]);
 const pages=new Map();
 await collectPages(dist,pages);
 for(const expected of ['/guias/','/guias/primeiro-projeto/','/guias/conversas/','/guias/relatorios/','/guias/uso/','/guias/skills/','/guias/conta/','/perguntas/','/seguranca/']){
  assert.ok(pages.has(expected),expected);
 }
 for(const [route,html] of pages){
  if(!route.startsWith('/conta/')){
   assert.equal([...html.matchAll(/<h1\b/gi)].length,1,route);
   assert.doesNotMatch(html,/\b(?:Gemini|Ministral)\b/i,route);
   assert.doesNotMatch(html,/[→←↓↗▸▶]/u,route);
  }
  for(const [,value] of html.matchAll(/\b(?:href|src)="([^"]+)"/gi)){
   const target=new URL(value,new URL(route,origin));
   if(target.origin!==origin)continue;
   if(target.pathname.endsWith('/')){
    assert.ok(pages.has(target.pathname),route+' → '+value);
    if(target.hash){
     const id=decodeURIComponent(target.hash.slice(1));
     assert.ok(pages.get(target.pathname).includes('id="'+id+'"'),route+' → '+value);
    }
   }else {
    const asset=await readFile(join(dist,decodeURIComponent(target.pathname.slice(1))));
    assert.ok(asset.byteLength>0,route+' → '+value);
   }
  }
 }
});
