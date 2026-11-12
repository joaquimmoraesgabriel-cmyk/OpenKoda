import { execFile } from 'node:child_process';
import { access, lstat, readFile, readdir, realpath } from 'node:fs/promises';
import { dirname, join, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const run = promisify(execFile);
export const root = dirname(fileURLToPath(import.meta.url));
export const sdkVersion = '0.10.0';
const windows = process.platform === 'win32';

// Both the worker and its tools start with an explicit environment. In
// particular, developer credentials, provider overrides and interpreter
// preload options from the desktop session do not reach the scanner.
export function baseEnvironment() {
  const allowed = new Set(['SystemRoot','WINDIR','SystemDrive','ComSpec','TEMP','TMP','TMPDIR','USERPROFILE','USERNAME','USERDOMAIN','APPDATA','LOCALAPPDATA','ProgramData','ProgramFiles','ProgramFiles(x86)','ProgramW6432','PROCESSOR_ARCHITECTURE','NUMBER_OF_PROCESSORS','PATHEXT','HOME','LANG','LC_ALL'].map(s => s.toUpperCase()));
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => allowed.has(key.toUpperCase())));
  const paths = windows
    ? [dirname(process.execPath), join(env.ProgramFiles || 'C:\\Program Files','Git','cmd'), join(env.SystemRoot || 'C:\\Windows','System32'), join(env.SystemRoot || 'C:\\Windows','System32','WindowsPowerShell','v1.0')]
    : [dirname(process.execPath),'/usr/local/bin','/usr/bin','/bin'];
  env.PATH = paths.join(delimiter);
  env.PYTHONNOUSERSITE = '1';
  env.PYTHONDONTWRITEBYTECODE = '1';
  env.GIT_TERMINAL_PROMPT = '0';
  return env;
}

const probeCodes=new Set(['ENOENT','ENOTDIR','EACCES','EPERM','ELOOP','EINVAL','ENOEXEC','ENAMETOOLONG','EMFILE','ENFILE','ENOMEM','EBUSY','ETXTBSY','EAGAIN','UNKNOWN','ERR_CHILD_PROCESS_STDIO_MAXBUFFER']);
const probeSignals=new Set(['SIGHUP','SIGINT','SIGQUIT','SIGILL','SIGTRAP','SIGABRT','SIGBUS','SIGFPE','SIGKILL','SIGUSR1','SIGSEGV','SIGUSR2','SIGPIPE','SIGALRM','SIGTERM','SIGBREAK']);

// Diagnostics cross the native bridge. Never retain a path, exception message,
// stdout or stderr: child-process errors can contain command arguments or keys.
export async function probeBinaryVersion(paths, args, pattern, timeoutMs=10000) {
  if(!Number.isInteger(timeoutMs)||timeoutMs<1||timeoutMs>10000)throw new TypeError('invalid_probe_timeout');
  const attempts=[];
  const result=binary=>({binary,diagnostic:{attempts:attempts.slice(0,8),additionalAttempts:Math.max(0,attempts.length-8)}});
  for (const [index,path] of paths.entries()) {
    const started=performance.now();let executing=false,child=null,stdoutBytes=null,stderrBytes=null,spawnElapsedMs=null;
    const record=(category,error)=>{
      const raw=error?.code;
      const code=typeof raw==='number' && Number.isSafeInteger(raw)?raw:probeCodes.has(raw)?raw:null;
      attempts.push({candidate:index+1,category,code,signal:probeSignals.has(error?.signal)?error.signal:null,elapsedMs:Math.max(0,Math.round(performance.now()-started)),stdoutBytes,stderrBytes,spawnElapsedMs,childKilled:typeof child?.killed==='boolean'?child.killed:null,exitCode:Number.isSafeInteger(child?.exitCode)?child.exitCode:null});
    };
    try {
      const exe = await realpath(path);
      if (!(await lstat(exe)).isFile()){record('not_file');continue;}
      executing=true;
      const beforeSpawn=performance.now();
      const pending=run(exe,args,{cwd:dirname(exe),env:baseEnvironment(),windowsHide:true,timeout:timeoutMs,maxBuffer:8192});
      spawnElapsedMs=Math.max(0,Math.round(performance.now()-beforeSpawn));child=pending.child;
      const {stdout,stderr} = await pending;
      stdoutBytes=Buffer.byteLength(stdout);stderrBytes=Buffer.byteLength(stderr);
      // execFile can resolve an exit-zero callback even after its kill path.
      // A terminated child is never evidence of a usable installation.
      if(child?.killed===true){record('terminated',{code:child.exitCode,signal:child.signalCode});continue;}
      const match=stdout.trim().match(pattern);
      if (match){record('available');return result({path:exe,version:match[1]});}
      record('output_format');
    } catch(error) {
      if(typeof error.stdout==='string'||Buffer.isBuffer(error.stdout))stdoutBytes=Buffer.byteLength(error.stdout);
      if(typeof error.stderr==='string'||Buffer.isBuffer(error.stderr))stderrBytes=Buffer.byteLength(error.stderr);
      const category=error.code==='ERR_CHILD_PROCESS_STDIO_MAXBUFFER'?'output_limit':
        error.killed===true?'timeout':typeof error.code==='number'?'exit':
        probeSignals.has(error.signal)?'signal':['ENOENT','ENOTDIR'].includes(error.code)?'missing':
        ['EACCES','EPERM'].includes(error.code)?'access_denied':executing?'spawn_error':'filesystem_error';
      record(category,error);
    }
  }
  return result(null);
}

async function pythonCandidates() {
  if (!windows) return [join(root,'python','bin','python3'),'/usr/local/bin/python3','/usr/bin/python3'];
  const paths=[join(root,'python','python.exe')];
  const bases=[process.env.LOCALAPPDATA && join(process.env.LOCALAPPDATA,'Programs','Python'),process.env.ProgramFiles];
  for (const dir of bases.filter(Boolean)) {
    const names=await readdir(dir,{withFileTypes:true}).catch(()=>[]);
    for (const name of names.filter(entry=>entry.isDirectory() && /^Python3\d+$/.test(entry.name)).sort((a,b)=>b.name.localeCompare(a.name,undefined,{numeric:true}))) {
      paths.push(join(dir,name.name,'python.exe'));
    }
  }
  return paths;
}

export async function inspectPlatform() {
  const major=Number(process.versions.node.split('.')[0]);
  const goodNode=major===24 || major===26 || major===22 && Number(process.versions.node.split('.')[1])>=13;
  const triple=(process.arch==='arm64'?'aarch64':'x86_64')+(windows?'-pc-windows-msvc':process.platform==='darwin'?'-apple-darwin':'-unknown-linux-musl');
  const vendor=join(root,'node_modules','@openai',`codex-${process.platform}-${process.arch}`,'vendor',triple,'bin',windows?'codex.exe':'codex');
  const gitPaths=windows?[join(process.env.ProgramFiles || 'C:\\Program Files','Git','cmd','git.exe')]:['/usr/bin/git','/usr/local/bin/git'];
  const [sdk,pythonProbe,gitProbe,codexProbe]=await Promise.all([
    (async()=>{
      const dir=join(root,'node_modules','@openai','codex-security');
      const pkg=JSON.parse(await readFile(join(dir,'package.json'),'utf8'));
      if(pkg.name!=='@openai/codex-security'||pkg.version!==sdkVersion)return false;
      await access(join(dir,'_bundled_plugin','skills','security-scan','SKILL.md'));
      return typeof (await import('@openai/codex-security')).CodexSecurity==='function';
    })().catch(()=>false),
    probeBinaryVersion(await pythonCandidates(),['-I','-B','-c','import sys, importlib, sqlite3; assert sys.version_info >= (3,10); importlib.import_module("tomllib" if sys.version_info >= (3,11) else "tomli"); print(sys.version.split()[0])'],/^(3\.\d+\.\d+)$/),
    probeBinaryVersion(gitPaths,['--version'],/^git version ([\w.\-]+)$/),
    probeBinaryVersion([vendor],['--version'],/^codex(?:-cli)?\s+([\w.\-]+)$/),
  ]);
  const python=pythonProbe.binary,git=gitProbe.binary,codex=codexProbe.binary;
  const missing=[];
  if(!goodNode)missing.push('node');
  if(!sdk)missing.push('sdk');
  if(!python)missing.push('python');
  if(!git)missing.push('git');
  if(!codex)missing.push('codex');
  return {python,git,codex,check:{type:'check',available:missing.length===0,sdkVersion:sdk?sdkVersion:null,nodeVersion:process.versions.node,pythonVersion:python?.version??null,gitVersion:git?.version??null,codexVersion:codex?.version??null,missing,diagnostics:{python:pythonProbe.diagnostic,git:gitProbe.diagnostic,codex:codexProbe.diagnostic}}};
}
