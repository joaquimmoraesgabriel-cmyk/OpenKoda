//! Public GitHub acquisition. Only a validated HTTPS GitHub repository URL can
//! reach the fixed Git executable. A fixed Node handshake lets Windows attach
//! the process tree to a kill-on-close job before Git is allowed to start.
use crate::{core,model};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{fs,io::{Read,Write},path::{Path,PathBuf},process::{Child,Command,Stdio},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering},mpsc},thread,time::{Duration,Instant}};

const MAX_DISK:u64=768*1024*1024;
const MAX_TOTAL:u64=2*1024*1024*1024;
const MAX_FILES:usize=25000;
const TIMEOUT:Duration=Duration::from_secs(180);
const HANDSHAKE:&str="let b='';process.stdin.setEncoding('utf8');process.stdin.on('data',s=>{b+=s;if(b.length>65536)process.exit(2);const i=b.indexOf('\\n');if(i<0)return;process.stdin.pause();const a=JSON.parse(b.slice(0,i));if(!Array.isArray(a)||a.some(x=>typeof x!=='string'))process.exit(2);const c=require('node:child_process').spawn(a[0],a.slice(1),{windowsHide:true,stdio:['ignore','inherit','inherit']});c.once('error',()=>process.exit(3));c.once('exit',code=>process.exit(code??4));});";

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Source{pub url:String,pub revision:String,pub import_id:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Receipt{id:String,url:String,name:String,status:String,message:String,revision:Option<String>,created_at:String,updated_at:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Import{url:String,idempotency_key:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Lookup{id:String}
struct Active{id:String,cancel:Arc<AtomicBool>}
pub(crate) struct Service{root:PathBuf,git:PathBuf,node:PathBuf,active:Mutex<Option<Active>>}

pub(crate) fn normalize_url(raw:&str)->Result<(String,String),String>{
    if raw.len()>512||raw.trim()!=raw||raw.bytes().any(|b|b<0x21||b>0x7e)||raw.contains(['%','\\','?','#']){return Err("Use um link público HTTPS do GitHub, no formato https://github.com/dono/repositorio.".into());}
    let url=url::Url::parse(raw).map_err(|_|"Link do GitHub inválido.")?;
    if url.scheme()!="https"||url.host_str()!=Some("github.com")||url.port().is_some()||!url.username().is_empty()||url.password().is_some(){return Err("Somente links públicos https://github.com/dono/repositorio são aceitos.".into());}
    let path=url.path().trim_end_matches('/');let parts=path.strip_prefix('/').unwrap_or("").split('/').collect::<Vec<_>>();
    if parts.len()!=2{return Err("Informe o link do repositório, sem /tree, /blob ou caminho de arquivo.".into());}
    let owner=parts[0];let repo=parts[1].strip_suffix(".git").unwrap_or(parts[1]);
    let safe=|s:&str,limit:usize|!s.is_empty()&&s.len()<=limit&&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))&&!s.starts_with(['-','.'])&&!s.ends_with('.')&&!s.contains("..");
    if !safe(owner,39)||!safe(repo,100){return Err("Nome de usuário ou repositório inválido no GitHub.".into());}
    Ok((format!("https://github.com/{owner}/{repo}"),repo.into()))
}
fn plain(path:&Path,directory:bool)->Result<(),String>{
    let meta=fs::symlink_metadata(path).map_err(|_|"Arquivo da importação indisponível.")?;
    #[cfg(windows)]let redirected={use std::os::windows::fs::MetadataExt;meta.file_attributes()&0x400!=0};
    #[cfg(not(windows))]let redirected=false;
    if redirected||meta.file_type().is_symlink()||if directory{!meta.is_dir()}else{!meta.is_file()}{return Err("A importação contém caminho redirecionado ou irregular.".into());}
    Ok(())
}
fn owned_root(base:&Path)->Result<PathBuf,String>{
    let base=fs::canonicalize(base).map_err(|_|"Perfil indisponível.")?;let root=base.join("github-repositories");
    if !root.exists(){fs::create_dir(&root).map_err(|_|"Não foi possível criar a pasta gerida de repositórios.")?;}
    plain(&root,true)?;let actual=fs::canonicalize(root).map_err(|_|"Pasta de importação indisponível.")?;
    if actual.parent()!=Some(base.as_path()){return Err("Pasta de importação fora do perfil gerido.".into());}Ok(actual)
}
fn write_receipt(dir:&Path,receipt:&Receipt)->Result<(),String>{
    plain(dir,true)?;let temporary=dir.join(format!("{}.tmp",model::id()));
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|_|"Não foi possível registrar a importação.")?;
    let bytes=serde_json::to_vec(receipt).map_err(|_|"Registro de importação inválido.")?;
    file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|"Não foi possível confirmar a importação em disco.")?;drop(file);
    let target=dir.join("receipt.json");
    // Windows rename does not replace an existing destination. The receipt is
    // metadata only; an interrupted replacement is conservatively unavailable.
    if target.exists(){plain(&target,false)?;fs::remove_file(&target).map_err(|_|"Registro da importação em uso.")?;}
    fs::rename(temporary,target).map_err(|_|"Não foi possível publicar o registro da importação.".into())
}
fn read_receipt(dir:&Path)->Result<Receipt,String>{
    plain(dir,true)?;let path=dir.join("receipt.json");plain(&path,false)?;
    let mut bytes=Vec::new();fs::File::open(path).map_err(|_|"Registro ausente.")?.take(8193).read_to_end(&mut bytes).map_err(|_|"Registro ilegível.")?;
    if bytes.len()>8192{return Err("Registro de importação excede o limite.".into());}
    let r:Receipt=serde_json::from_slice(&bytes).map_err(|_|"Registro de importação inválido.")?;
    core::valid_id(&r.id)?;let (url,name)=normalize_url(&r.url)?;
    if dir.file_name().and_then(|s|s.to_str())!=Some(&r.id)||r.url!=url||r.name!=name||!matches!(r.status.as_str(),"downloading"|"ready"|"failed"|"cancelled"|"interrupted")||r.revision.as_ref().is_some_and(|s|s.len()!=40||!s.bytes().all(|b|b.is_ascii_hexdigit())){return Err("O registro da importação não corresponde à pasta.".into());}
    Ok(r)
}
fn public(root:&Path,r:&Receipt,replayed:bool)->Value{
    let mut v=json!(r);v["replayed"]=json!(replayed);
    if r.status=="ready"{v["path"]=json!(display_path(&root.join(&r.id).join("repository")));}v
}
fn display_path(path:&Path)->String{
    let value=path.to_string_lossy();value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}
fn tree_size(path:&Path,limit:u64)->Result<u64,String>{
    plain(path,true)?;let mut total=0u64;let mut stack=vec![path.to_path_buf()];let mut count=0;
    while let Some(dir)=stack.pop(){for entry in fs::read_dir(dir).map_err(|_|"Não foi possível conferir o tamanho da importação.")?{
        let p=entry.map_err(|_|"Entrada de importação inválida.")?.path();let metadata=fs::symlink_metadata(&p).map_err(|_|"Arquivo de importação indisponível.")?;count+=1;
        if count>100000{return Err("A importação contém arquivos demais.".into());}
        plain(&p,metadata.is_dir())?;if metadata.is_dir(){stack.push(p);}else{total=total.saturating_add(metadata.len());if total>limit{return Err("A importação excedeu o limite de espaço desta instalação.".into());}}
    }}Ok(total)
}
fn valid_tree(bytes:&[u8])->Result<(),String>{
    let mut count=0;let mut total=0u64;
    for entry in bytes.split(|b|*b==0).filter(|b|!b.is_empty()){
        count+=1;if count>MAX_FILES{return Err("O repositório contém mais de 25.000 arquivos.".into());}
        let tab=entry.iter().position(|b|*b==b'\t').ok_or("Inventário Git inválido.")?;
        let meta=std::str::from_utf8(&entry[..tab]).map_err(|_|"Inventário Git inválido.")?.split_whitespace().collect::<Vec<_>>();
        let name=std::str::from_utf8(&entry[tab+1..]).map_err(|_|"Nome de arquivo fora de UTF-8 não suportado.")?;
        if meta.len()!=4||!matches!(meta[0],"100644"|"100755")||meta[1]!="blob"{return Err("Esta importação não aceita links simbólicos ou submódulos. Use uma cópia local revisada para esse repositório.".into());}
        let size=meta[3].parse::<u64>().map_err(|_|"Tamanho de arquivo inválido.")?;total=total.saturating_add(size);
        if size>64*1024*1024||total>384*1024*1024{return Err("Os arquivos do repositório excedem os limites da importação.".into());}
        if name.len()>1024||name.split('/').any(|p|{
            let base=p.split('.').next().unwrap_or("").to_ascii_uppercase();
            p.is_empty()||p=="."||p==".."||p.eq_ignore_ascii_case(".git")||p.ends_with([' ','.'])||p.chars().any(|c|c.is_control()||"\\:<>\"|?*".contains(c))||["CON","PRN","AUX","NUL","COM1","COM2","COM3","COM4","COM5","COM6","COM7","COM8","COM9","LPT1","LPT2","LPT3","LPT4","LPT5","LPT6","LPT7","LPT8","LPT9"].contains(&base.as_str())
        }){return Err("O repositório contém um caminho incompatível ou inseguro para Windows.".into());}
    }Ok(())
}
impl Service{
    pub(crate) fn new(profile:PathBuf,resources:&Path)->Self{
        let packaged=resources.join("security-runtime/bin/node.exe");
        #[cfg(debug_assertions)]let node=if packaged.is_file(){packaged}else{PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../security-runtime/bin/node.exe")};
        #[cfg(not(debug_assertions))]let node=packaged;
        let git=std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(||PathBuf::from(r"C:\Program Files")).join("Git/cmd/git.exe");
        Self{root:profile,git,node,active:Mutex::new(None)}
    }
    pub(crate) fn get(&self,payload:Value)->Result<Value,String>{
        let input:Lookup=serde_json::from_value(payload).map_err(|_|"Importação inválida.")?;core::valid_id(&input.id)?;
        let active=self.active.lock().map_err(|_|"Importação indisponível.")?;
        let root=owned_root(&self.root)?;let dir=root.join(&input.id);let mut r=read_receipt(&dir)?;
        if r.status=="downloading"&&!active.as_ref().is_some_and(|v|v.id==r.id){r.status="interrupted".into();r.message="Download interrompido. Nenhum projeto foi criado; inicie uma nova importação para tentar novamente.".into();r.updated_at=model::now();write_receipt(&dir,&r)?;}
        if r.status=="ready"{plain(&dir.join("repository"),true)?;}
        Ok(public(&root,&r,false))
    }
    pub(crate) fn cancel(&self,payload:Value)->Result<Value,String>{
        let input:Lookup=serde_json::from_value(payload.clone()).map_err(|_|"Importação inválida.")?;core::valid_id(&input.id)?;
        if let Some(job)=self.active.lock().map_err(|_|"Importação indisponível.")?.as_ref().filter(|job|job.id==input.id){job.cancel.store(true,Ordering::Release);}
        self.get(payload)
    }
    pub(crate) fn start_worker(&self,payload:Value,tx:&mpsc::SyncSender<Result<Value,String>>)->Result<(),String>{
        let input:Import=serde_json::from_value(payload).map_err(|_|"Solicitação de importação inválida.")?;core::valid_id(&input.idempotency_key)?;
        let (url,name)=normalize_url(&input.url)?;let root=owned_root(&self.root)?;
        let _lease=crate::source_scan::lock_metadata_target(&root)?;
        let id=crate::scope_targets::derived_id("openkoda.github-import.v1",&[&input.idempotency_key]);let dir=root.join(&id);
        let mut active=self.active.lock().map_err(|_|"Importação indisponível.")?;
        if dir.exists(){let r=read_receipt(&dir)?;if r.url!=url{return Err("Esta solicitação já identifica outro repositório.".into());}drop(active);let mut v=self.get(json!({"id":id}))?;v["replayed"]=json!(true);let _=tx.send(Ok(v));return Ok(());}
        if active.is_some(){return Err("Aguarde ou cancele a importação atual antes de iniciar outra.".into());}
        if fs::read_dir(&root).map_err(|_|"Pasta de importação indisponível.")?.count()>=100||tree_size(&root,MAX_TOTAL)?>MAX_TOTAL{return Err("Revise as cópias geridas antes de importar mais repositórios.".into());}
        plain(&self.git,false).map_err(|_|"Instale o Git para Windows em Arquivos de Programas para importar repositórios.")?;plain(&self.node,false).map_err(|_|"O componente local de importação não foi instalado.")?;
        fs::create_dir(&dir).map_err(|_|"Não foi possível criar o destino exclusivo da importação.")?;
        let _directory=crate::source_scan::lock_metadata_target(&dir)?;
        for name in ["home","tmp","empty-hooks"]{fs::create_dir(dir.join(name)).map_err(|_|"Não foi possível preparar a importação isolada.")?;}
        fs::File::create(dir.join("empty-config")).map_err(|_|"Não foi possível preparar as opções Git isoladas.")?;
        let mut r=Receipt{id:id.clone(),url,name,status:"downloading".into(),message:"Baixando o repositório público do GitHub…".into(),revision:None,created_at:model::now(),updated_at:model::now()};write_receipt(&dir,&r)?;
        let cancel=Arc::new(AtomicBool::new(false));*active=Some(Active{id:id.clone(),cancel:cancel.clone()});drop(active);
        let _=tx.send(Ok(public(&root,&r,false)));
        let result=self.acquire(&root,&dir,&r,&cancel);
        match result{Ok(revision)=>{r.revision=Some(revision);r.status="ready".into();r.message="Repositório baixado e arquivos conferidos. Pronto para criar o projeto.".into();},Err(error)=>{r.status=if cancel.load(Ordering::Acquire){"cancelled"}else{"failed"}.into();r.message=error;}}
        r.updated_at=model::now();
        let mut active=self.active.lock().map_err(|_|"Importação indisponível.")?;
        let saved=write_receipt(&dir,&r);*active=None;saved
    }
    fn acquire(&self,root:&Path,dir:&Path,r:&Receipt,cancel:&AtomicBool)->Result<String,String>{
        let deadline=Instant::now()+TIMEOUT;let repo=dir.join("repository");
        self.run(root,dir,cancel,deadline,&["clone".into(),"--no-checkout".into(),"--no-local".into(),"--no-hardlinks".into(),"--no-recurse-submodules".into(),"--depth=1".into(),"--no-tags".into(),r.url.clone(),display_path(&repo)])?;
        plain(&repo,true)?;plain(&repo.join(".git"),true)?;
        let revision=String::from_utf8(self.run(root,dir,cancel,deadline,&["-C".into(),display_path(&repo),"rev-parse".into(),"--verify".into(),"HEAD".into()])?).map_err(|_|"Revisão Git inválida.")?.trim().to_string();
        if revision.len()!=40||!revision.bytes().all(|b|b.is_ascii_hexdigit()){return Err("O GitHub retornou uma revisão não suportada.".into());}
        let inventory=self.run(root,dir,cancel,deadline,&["-C".into(),display_path(&repo),"ls-tree".into(),"-rlz".into(),"HEAD".into()])?;valid_tree(&inventory)?;
        self.run(root,dir,cancel,deadline,&["-C".into(),display_path(&repo),"checkout".into(),"--detach".into(),"--force".into(),revision.clone()])?;
        tree_size(&repo,MAX_DISK)?;Ok(revision)
    }
    fn command(&self,dir:&Path)->Command{
        let mut command=Command::new(&self.node);command.arg("-e").arg(HANDSHAKE).current_dir(dir).env_clear();
        for key in ["SystemRoot","WINDIR","SystemDrive","COMSPEC","ProgramFiles","ProgramFiles(x86)","ProgramW6432","PATHEXT"]{if let Some(v)=std::env::var_os(key){command.env(key,v);}}
        let home=dir.join("home");let config=if cfg!(windows){"NUL"}else{"/dev/null"};
        command.env("HOME",&home).env("USERPROFILE",&home).env("XDG_CONFIG_HOME",&home).env("TEMP",dir.join("tmp")).env("TMP",dir.join("tmp"))
            .env("GIT_CONFIG_NOSYSTEM","1").env("GIT_CONFIG_SYSTEM",&config).env("GIT_CONFIG_GLOBAL",&config).env("GIT_TERMINAL_PROMPT","0").env("GCM_INTERACTIVE","never")
            .env("GIT_ASKPASS",dir.join("disabled-askpass")).env("GIT_ALLOW_PROTOCOL","https").env("GIT_LFS_SKIP_SMUDGE","1").env("GIT_NO_LAZY_FETCH","1");
        #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());command
    }
    fn run(&self,root:&Path,dir:&Path,cancel:&AtomicBool,deadline:Instant,args:&[String])->Result<Vec<u8>,String>{
        if cancel.load(Ordering::Acquire){return Err("Importação cancelada. Nenhum projeto foi criado.".into());}
        let mut child=Managed::new(self.command(dir).spawn().map_err(|_|"Não foi possível iniciar o componente de importação.")?)?;
        let out=bounded(child.child.stdout.take().ok_or("Saída indisponível.")?,16*1024*1024);let err=bounded(child.child.stderr.take().ok_or("Saída indisponível.")?,2*1024*1024);
        let mut argv=vec![display_path(&self.git)];
        for value in ["protocol.allow=never".to_string(),"protocol.https.allow=always".into(),"credential.helper=".into(),"http.extraHeader=".into(),"http.followRedirects=false".into(),"http.sslVerify=true".into(),"core.fsmonitor=false".into(),"core.symlinks=false".into(),"core.protectNTFS=true".into(),"submodule.recurse=false".into(),"filter.lfs.required=false".into(),"filter.lfs.smudge=".into(),"filter.lfs.process=".into(),format!("core.hooksPath={}",if cfg!(windows){"NUL"}else{"/dev/null"}),format!("init.templateDir={}",display_path(&dir.join("empty-hooks"))) ]{argv.push("-c".into());argv.push(value);}
        argv.extend_from_slice(args);let mut input=child.child.stdin.take().ok_or("Canal de importação indisponível.")?;
        serde_json::to_writer(&mut input,&argv).map_err(|_|"Não foi possível preparar a importação.")?;input.write_all(b"\n").map_err(|_|"Não foi possível iniciar o Git.")?;drop(input);
        let mut checked=Instant::now();let status=loop{
            if cancel.load(Ordering::Acquire){return Err("Importação cancelada. Nenhum projeto foi criado.".into());}
            if Instant::now()>=deadline{return Err("A importação ultrapassou três minutos. Nenhum projeto foi criado.".into());}
            if checked.elapsed()>=Duration::from_secs(1){tree_size(dir,MAX_DISK)?;tree_size(root,MAX_TOTAL)?;checked=Instant::now();}
            if let Some(status)=child.child.try_wait().map_err(|_|"Não foi possível conferir o processo Git.")?{break status;}
            thread::sleep(Duration::from_millis(100));
        };
        let stdout=out.recv_timeout(Duration::from_secs(3)).map_err(|_|"A importação não confirmou o encerramento.")??;
        let _stderr=err.recv_timeout(Duration::from_secs(3)).map_err(|_|"A importação não confirmou o encerramento.")??;
        if !status.success(){return Err("O GitHub não permitiu baixar este repositório. Confirme que o link está correto, o repositório é público e a conexão está disponível. Repositórios privados exigem uma cópia local.".into());}
        Ok(stdout)
    }
}
fn bounded(mut input:impl Read+Send+'static,limit:usize)->mpsc::Receiver<Result<Vec<u8>,String>>{
    let (tx,rx)=mpsc::sync_channel(1);thread::spawn(move||{let mut bytes=Vec::new();let result=input.by_ref().take(limit as u64+1).read_to_end(&mut bytes).map_err(|_|"Saída Git interrompida.".to_string()).and_then(|_|if bytes.len()>limit{Err("O Git excedeu o limite de saída.".into())}else{Ok(bytes)});let _=tx.send(result);});rx
}
struct Managed{child:Child,#[cfg(windows)]job:windows_sys::Win32::Foundation::HANDLE}
impl Managed{
    fn new(mut child:Child)->Result<Self,String>{
        #[cfg(windows)]{use std::os::windows::io::AsRawHandle;use windows_sys::Win32::{Foundation::CloseHandle,System::JobObjects::*};unsafe{
            let job=CreateJobObjectW(std::ptr::null(),std::ptr::null());let mut info:JOBOBJECT_EXTENDED_LIMIT_INFORMATION=std::mem::zeroed();info.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if job.is_null()||SetInformationJobObject(job,JobObjectExtendedLimitInformation,(&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),std::mem::size_of_val(&info) as u32)==0||AssignProcessToJobObject(job,child.as_raw_handle())==0{if !job.is_null(){CloseHandle(job);}let _=child.kill();let _=child.wait();return Err("O Windows recusou o controle dos processos de importação.".into());}
            Ok(Self{child,job})
        }}
        #[cfg(not(windows))]{Ok(Self{child})}
    }
}
impl Drop for Managed{fn drop(&mut self){#[cfg(windows)]unsafe{windows_sys::Win32::Foundation::CloseHandle(self.job);}let _=self.child.kill();let _=self.child.wait();}}

pub(crate) fn source(profile:&Path,id:&str,root:&str)->Result<Source,String>{
    core::valid_id(id)?;let managed=owned_root(profile)?;let dir=managed.join(id);let r=read_receipt(&dir)?;
    if r.status!="ready"{return Err("Aguarde a conclusão da importação antes de criar o projeto.".into());}
    let expected=dir.join("repository");plain(&expected,true)?;
    let actual=fs::canonicalize(root).map_err(|_|"A pasta importada não está disponível.")?;
    if actual!=fs::canonicalize(&expected).map_err(|_|"A pasta importada não está disponível.")?{return Err("A pasta do projeto não corresponde à importação selecionada.".into());}
    Ok(Source{url:r.url,revision:r.revision.ok_or("Revisão da importação ausente.")?,import_id:id.into()})
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn only_github_repository_urls_without_auth_or_paths_are_accepted(){
        assert_eq!(normalize_url("https://github.com/openai/codex-security.git/").unwrap(),("https://github.com/openai/codex-security".into(),"codex-security".into()));
        for input in ["http://github.com/a/b","https://github.com.evil.test/a/b","https://github.com@evil.test/a/b","https://x@github.com/a/b","https://github.com/a/b?token=secret","https://github.com/a/b#main","https://github.com/a/b/tree/main","git@github.com:a/b","https://github.com/a/%2e%2e","https://github.com/a/../b","https://github.com/a/-repo","https://github.com:444/a/b"," https://github.com/a/b"]{assert!(normalize_url(input).is_err(),"{input}");}
    }
    fn entry(mode:&str,path:&str,size:&str)->Vec<u8>{format!("{mode} blob {} {size}\t{path}\0","a".repeat(40)).into_bytes()}
    #[test]fn git_inventory_refuses_links_devices_traversal_and_large_files(){
        valid_tree(&entry("100644","src/árvore.rs","42")).unwrap();
        for (mode,path,size) in [("120000","link","1"),("160000","submodule","1"),("100644","../outside","1"),("100644","NUL.txt","1"),("100644",".git/config","1"),("100644","src/foo:bar","1"),("100644","src/a.","1"),("100644","src/a","999999999")]{assert!(valid_tree(&entry(mode,path,size)).is_err());}
    }
    #[test]fn receipt_identity_and_interrupted_download_are_not_replayed_as_success(){
        let tmp=tempfile::tempdir().unwrap();let service=Service::new(tmp.path().into(),tmp.path());let root=owned_root(tmp.path()).unwrap();let id=model::id();let dir=root.join(&id);fs::create_dir(&dir).unwrap();
        let r=Receipt{id:id.clone(),url:"https://github.com/openai/codex-security".into(),name:"codex-security".into(),status:"downloading".into(),message:"Downloading".into(),revision:None,created_at:model::now(),updated_at:model::now()};write_receipt(&dir,&r).unwrap();
        assert_eq!(service.get(json!({"id":id})).unwrap()["status"],"interrupted");
        assert!(source(tmp.path(),&id,tmp.path().to_str().unwrap()).is_err());
    }
    #[test]fn launch_metadata_is_bound_to_ready_managed_directory(){
        let tmp=tempfile::tempdir().unwrap();let root=owned_root(tmp.path()).unwrap();let id=model::id();let dir=root.join(&id);fs::create_dir(&dir).unwrap();let repo=dir.join("repository");fs::create_dir(&repo).unwrap();
        let r=Receipt{id:id.clone(),url:"https://github.com/openai/codex-security".into(),name:"codex-security".into(),status:"ready".into(),message:"Ready".into(),revision:Some("a".repeat(40)),created_at:model::now(),updated_at:model::now()};write_receipt(&dir,&r).unwrap();
        assert!(source(tmp.path(),&id,tmp.path().to_str().unwrap()).is_err());assert_eq!(source(tmp.path(),&id,repo.to_str().unwrap()).unwrap().revision,"a".repeat(40));
    }
    #[test]#[cfg(windows)]fn fixed_handshake_runs_git_without_inheriting_credentials_or_configuration(){
        let tmp=tempfile::tempdir().unwrap();let service=Service::new(tmp.path().into(),tmp.path());
        for name in ["home","tmp","empty-hooks"]{fs::create_dir(tmp.path().join(name)).unwrap();}
        fs::write(tmp.path().join("home/.gitconfig"),"[url \"http://127.0.0.1:1/\"]\n insteadOf = https://github.com/\n").unwrap();
        let cancellation=AtomicBool::new(false);
        let bytes=service.run(tmp.path(),tmp.path(),&cancellation,Instant::now()+Duration::from_secs(15),&["--version".into()]).unwrap();
        assert!(std::str::from_utf8(&bytes).unwrap().starts_with("git version "));
        let cmd=service.command(tmp.path());let env=cmd.get_envs().map(|(k,v)|(k.to_string_lossy().to_string(),v.map(|v|v.to_string_lossy().to_string()))).collect::<std::collections::BTreeMap<_,_>>();
        assert_eq!(env["GIT_CONFIG_GLOBAL"].as_deref(),Some("NUL"));assert_eq!(env["GIT_TERMINAL_PROMPT"].as_deref(),Some("0"));assert!(!env.contains_key("OPENAI_API_KEY")&&!env.contains_key("NODE_OPTIONS"));
        cancellation.store(true,Ordering::Release);assert!(service.run(tmp.path(),tmp.path(),&cancellation,Instant::now()+Duration::from_secs(15),&["--version".into()]).is_err());
    }
    #[test]#[ignore="Explicit network download of public octocat fixture; no inference or credentials"]#[cfg(windows)]
    fn public_github_import_is_idempotent_and_ready_source_survives_reopen(){
        let tmp=tempfile::tempdir().unwrap();let service=Service::new(tmp.path().into(),tmp.path());let body=json!({"url":"https://github.com/octocat/Hello-World","idempotencyKey":model::id()});
        let (tx,rx)=mpsc::sync_channel(1);
        thread::scope(|scope|{
            let worker=scope.spawn(||service.start_worker(body.clone(),&tx));
            let first=rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();assert_eq!(first["status"],"downloading");
            let (tx2,rx2)=mpsc::sync_channel(1);service.start_worker(body.clone(),&tx2).unwrap();let replay=rx2.recv().unwrap().unwrap();assert_eq!(first["id"],replay["id"]);assert_eq!(replay["replayed"],true);
            worker.join().unwrap().unwrap();
            let record=service.get(json!({"id":first["id"]})).unwrap();assert_eq!(record["status"],"ready","{record}");
            let root=PathBuf::from(record["path"].as_str().unwrap());assert!(root.join("README").is_file());
            assert_eq!(fs::read_dir(tmp.path().join("github-repositories")).unwrap().count(),1);
            let reopened=Service::new(tmp.path().into(),tmp.path());let restored=reopened.get(json!({"id":first["id"]})).unwrap();assert_eq!(record["revision"],restored["revision"]);assert_eq!(record["path"],restored["path"]);
        });
    }
}
