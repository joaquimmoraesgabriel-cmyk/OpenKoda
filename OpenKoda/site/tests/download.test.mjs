import test from 'node:test';
import assert from 'node:assert/strict';
import {downloadInstaller} from '../worker.mjs';
const installer={url:'https://github.com/example/fixed.exe',filename:'OpenKoda_1.0.0_x64-setup.exe',bytes:100};
test('download relays streams and ranges without forwarding secrets',async()=>{
 let seen;
 const send=async(url,options)=>{seen={url,options};return new Response(new Uint8Array([77,90]),{status:206,headers:{'Content-Length':'2','Content-Range':'bytes 0-1/100'}});};
 const r=await downloadInstaller(new Request('https://site.example/download/windows?url=https://evil.test',{headers:{Range:'bytes=0-1',Cookie:'private',Authorization:'Bearer private'}}),installer,send);
 assert.equal(r.status,206);assert.equal(await r.text(),'MZ');assert.equal(seen.url,installer.url);
 assert.equal(seen.options.headers.get('Cookie'),null);assert.equal(seen.options.headers.get('Authorization'),null);
 assert.equal(seen.options.headers.get('Range'),'bytes=0-1');assert.match(r.headers.get('Content-Disposition'),/attachment/);
});
test('invalid ranges and upstream errors never masquerade as installer',async()=>{
 for(const range of ['bytes=0-1,3-4','bytes=101-','bytes=2-1','bytes=-0']){
  const r=await downloadInstaller(new Request('https://site.example/download/windows',{headers:{Range:range}}),installer,()=>{throw Error('must not call');});assert.equal(r.status,416);
 }
 const r=await downloadInstaller(new Request('https://site.example/download/windows'),installer,async()=>new Response('oops',{status:404}));
 assert.equal(r.status,503);assert.equal(r.headers.get('Content-Disposition'),null);
});
test('resume carries If-Range and accepts suffix or clamped ranges',async()=>{
 for(const [input,normalized] of [['bytes=-5','bytes=95-99'],['bytes=90-999','bytes=90-99']]){
  const r=await downloadInstaller(new Request('https://site.example/download/windows',{headers:{Range:input,'If-Range':'"older-release"'}}),installer,async(_url,options)=>{
   assert.equal(options.headers.get('Range'),normalized);assert.equal(options.headers.get('If-Range'),'"older-release"');
   return new Response(new Uint8Array([77,90]),{status:200});
  });assert.equal(r.status,200);
 }
});
