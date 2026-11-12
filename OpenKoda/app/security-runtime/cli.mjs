import { fork } from 'node:child_process';
import { join } from 'node:path';
import { baseEnvironment, root } from './platform.mjs';
import { validateRequest } from './scan.mjs';

const limit=65536;
let credential;
let worker=null,terminal=false,buffer=Buffer.alloc(0),cancelRequested=false;
function write(event){
  const line=JSON.stringify(event,(_k,value)=>typeof value==='string' && credential?value.split(credential).join('[redacted]'):value);
  if(Buffer.byteLength(line)>2*1024*1024){
    process.stdout.write('{"type":"error","code":"output_limit","message":"O resultado excedeu o limite de transporte."}\n');
    return false;
  }
  process.stdout.write(line+'\n');
  return true;
}
function finish(event){
  if(terminal)return;
  terminal=true;
  const delivered=write(event);
  credential=undefined;
  process.exitCode=delivered && (event.type==='check' && event.available || event.type==='result' && event.status==='completed')?0:1;
  process.stdin.pause();
  process.stdin.unref?.();
}
function cancel(){
  if(cancelRequested || !worker?.connected || terminal)return;
  cancelRequested=true;
  worker.send({operation:'cancel'},()=>{});
}
function protocolError(){
  cancel();
  finish({type:'error',code:'invalid_request',message:'Mensagem inválida no protocolo do motor.'});
}
function start(input){
  // Only the parent owns stdout. SDK and native children's stdout/stderr
  // cannot mix with IPC or expose provider errors to application logs.
  credential=input.apiKey;
  worker=fork(join(root,'worker.mjs'),[],{cwd:root,execPath:process.execPath,execArgv:[],env:baseEnvironment(),stdio:['ignore','ignore','ignore','ipc'],windowsHide:true});
  worker.on('message',event=>{
    if(terminal)return;
    if(event?.type==='progress')write(event);
    else if(['error','result','check'].includes(event?.type))finish(event);
    else protocolError();
  });
  const lost=()=>finish({type:'error',code:'runtime_exit',message:'O processo do motor encerrou sem confirmação do resultado.'});
  worker.once('exit',lost);
  worker.once('error',lost);
  worker.send(input,error=>{if(error)lost();});
}
function line(bytes){
  const msg=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
  if(!worker){start(validateRequest(msg));return;}
  if(msg?.operation==='cancel' && Object.keys(msg).length===1){cancel();return;}
  protocolError();
}
if(process.argv.length===3 && process.argv[2]==='--check')start({operation:'check'});
else if(process.argv.length!==2)protocolError();
else {
  process.stdin.on('data',chunk=>{
    try{
      while(chunk.length && !terminal){
        const end=chunk.indexOf(10);
        const part=end<0?chunk:chunk.subarray(0,end);
        if (part.length+buffer.length>limit)throw new Error();
        buffer=Buffer.concat([buffer,part]);
        if(end<0)return;
        line(buffer);buffer=Buffer.alloc(0);chunk=chunk.subarray(end+1);
      }
    }catch{protocolError();}
  });
  process.stdin.once('end',()=>{if(!worker || buffer.length)protocolError();else cancel();});
  process.stdin.once('error',protocolError);
}
process.once('SIGTERM',cancel);
process.once('SIGINT',cancel);
