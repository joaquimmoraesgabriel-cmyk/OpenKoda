//! Desktop sessions are private to the selected local profile. Only the native
//! host exchanges credentials with Supabase or opens an allowlisted site page.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{blocking::{Client, Response}, redirect::Policy};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::{Read, Write}, net::{TcpListener, TcpStream}, path::{Path, PathBuf},
    sync::Mutex, time::{Duration, Instant}};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

pub(crate) const BASE: &str = "https://ynjcyvnrfxsrijrwpaye.supabase.co";
pub(crate) const EDGE: &str = "https://ynjcyvnrfxsrijrwpaye.supabase.co/functions/v1/openkoda";
pub(crate) const SITE: &str = "https://openkoda-desktop.steelyrowan1.chatgpt.site";
pub(crate) const PUBLISHABLE: &str = "sb_publishable_7HcLt5Ibu4dnsyj6DvvW9A_8OOCuI3N";
const SESSION_ERROR: &str = "Entre na sua conta OpenKoda para continuar.";

#[derive(Serialize, Deserialize)]
struct Session { access_token: String, refresh_token: String, expires_at: i64, user: Identity }
#[derive(Serialize, Deserialize)]
struct Identity { id: Uuid, email: String }

pub(crate) struct Account { file: PathBuf, guard: Mutex<()> }

pub(crate) fn supports(action: &str) -> bool {
    matches!(action,"account.session"|"account.signIn"|"account.google"|"account.recover"|
        "account.signOut"|"account.site"|"account.overview"|"account.consent"|"account.marketing"|
        "account.checkout"|"account.cancel"|"account.support"|"account.supportList"|"account.export"|
        "account.delete"|"account.models"|"account.skills"|"account.skill")
}
fn client() -> Result<Client,String> {
    Client::builder().https_only(true).redirect(Policy::none()).timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(8)).build().map_err(|_|"Cliente HTTPS indisponível.".into())
}
fn response(response: Response) -> Result<Value,String> {
    let status = response.status();
    let mut bytes = Zeroizing::new(Vec::new());
    response.take(2_097_153).read_to_end(&mut bytes).map_err(|_|"Resposta da conta interrompida.")?;
    if bytes.len()>2_097_152 { return Err("Resposta da conta excedeu o limite.".into()); }
    if !status.is_success() {
        if serde_json::from_slice::<Value>(&bytes).ok().is_some_and(|v| v["error_code"]=="captcha_failed") {
            return Err("captcha_required".into());
        }
        return Err(match status.as_u16() {
            400|401=>"O serviço recusou as credenciais ou os dados da solicitação.",
            403=>"Esta operação requer autorização da conta.",
            409=>"A solicitação já foi registrada. Consulte o estado antes de continuar.",
            422=>"A opção solicitada não está disponível para esta conta ou modelo.",
            429=>"Limite do serviço atingido. Tente novamente mais tarde.",
            _=>"O serviço de conta não conseguiu concluir a solicitação."
        }.into());
    }
    if bytes.is_empty() {return Ok(Value::Null);}
    serde_json::from_slice(&bytes).map_err(|_|"Resposta da conta inválida.".into())
}
fn auth(path:&str, body:Value, bearer:Option<&str>) -> Result<Value,String> {
    let mut request=client()?.post(format!("{BASE}/auth/v1/{path}")).header("apikey",PUBLISHABLE).json(&body);
    if let Some(token)=bearer {request=request.bearer_auth(token);}
    response(request.send().map_err(|_|"Não foi possível conectar ao serviço de autenticação.")?)
}
fn parse_session(value:Value) -> Result<Session,String> {
    let session:Session=serde_json::from_value(value).map_err(|_|"A autenticação não devolveu uma sessão válida.")?;
    if !(16..=16384).contains(&session.access_token.len()) || !(8..=2048).contains(&session.refresh_token.len())
        || session.user.email.len()>320 || session.expires_at<=chrono::Utc::now().timestamp() {
        return Err("Sessão inválida ou expirada.".into());
    }
    Ok(session)
}
fn email(value:&Value)->Result<&str,String>{
    let email=value["email"].as_str().unwrap_or("").trim();
    if email.len()>320 || !email.contains('@') || email.chars().any(char::is_whitespace) {return Err("Informe um e-mail válido.".into());}
    Ok(email)
}
fn password(value:&Value)->Result<&str,String>{
    value["password"].as_str().filter(|v|!v.is_empty()&&v.len()<=4096).ok_or_else(||"Informe a senha.".into())
}
impl Account {
    pub(crate) fn new(root:&Path)->Self { Self{file:root.join("commercial-account.sqlite3"),guard:Mutex::new(())} }
    fn db(&self)->Result<Connection,String> {
        let db=Connection::open(&self.file).map_err(|_|"Cofre da conta indisponível.")?;
        db.execute_batch("PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1),cipher BLOB NOT NULL);").map_err(|_|"Não foi possível abrir o cofre da conta.")?;
        Ok(db)
    }
    fn session(&self)->Result<Option<Session>,String>{
        if !self.file.try_exists().map_err(|_|"Cofre da conta indisponível.")?{return Ok(None);}
        let data:Option<Vec<u8>>=self.db()?.query_row("SELECT cipher FROM session WHERE id=1",[],|r|r.get(0))
            .optional().map_err(|_|"Não foi possível ler a sessão.")?;
        data.map(|v| {let plain=Zeroizing::new(crate::ai::protect(&v,true)?);serde_json::from_slice(&plain).map_err(|_|"Sessão armazenada inválida.".into())}).transpose()
    }
    fn save(&self,s:&Session)->Result<(),String>{
        let plain=Zeroizing::new(serde_json::to_vec(s).map_err(|_|"Sessão inválida.")?);
        let cipher=crate::ai::protect(&plain,false)?;
        self.db()?.execute("INSERT INTO session(id,cipher) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET cipher=excluded.cipher",[cipher]).map_err(|_|"Não foi possível guardar a sessão.")?;
        Ok(())
    }
    fn remove(&self)->Result<(),String>{self.db()?.execute("DELETE FROM session WHERE id=1",[]).map_err(|_|"Não foi possível remover a sessão.")?;Ok(())}
    fn current(&self)->Result<(Zeroizing<String>,String),String>{
        let mut s=self.session()?.ok_or(SESSION_ERROR)?;
        if s.expires_at<chrono::Utc::now().timestamp()+60 {
            let updated=parse_session(auth("token?grant_type=refresh_token",json!({"refresh_token":s.refresh_token}),None)?)?;
            if updated.user.id!=s.user.id{return Err("Identidade da sessão alterada.".into());}
            self.save(&updated)?; s=updated;
        }
        let revision=Self::session_revision(&s)?;
        Ok((Zeroizing::new(s.access_token),revision))
    }
    pub(crate) fn credential(&self)->Result<(Zeroizing<String>,String),String>{
        let _guard=self.guard.lock().map_err(|_|"Sessão indisponível.")?;
        self.current()
    }
    pub(crate) fn bearer(&self)->Result<Zeroizing<String>,String>{self.credential().map(|(token,_)|token)}
    pub(crate) fn identity_revision(&self)->Result<Option<String>,String>{
        let _guard=self.guard.lock().map_err(|_|"Sessão indisponível.")?;
        self.session()?.as_ref().map(Self::session_revision).transpose()
    }
    fn session_revision(s:&Session)->Result<String,String>{
        // This JWT was delivered by the pinned Auth origin, inside a DPAPI-encrypted
        // session. Its session_id is only a local lineage key; Edge authenticates
        // the bearer and enforces current entitlements on each hosted request.
        let payload=s.access_token.split('.').nth(1).and_then(|p|URL_SAFE_NO_PAD.decode(p).ok())
            .and_then(|v|serde_json::from_slice::<Value>(&v).ok()).ok_or("Sessão da conta inválida.")?;
        let id=payload["session_id"].as_str().and_then(|v|Uuid::parse_str(v).ok()).ok_or("Identidade da sessão indisponível.")?;
        if payload["sub"].as_str()!=Some(&s.user.id.to_string()){return Err("Identidade da conta inválida.".into());}
        Ok(id.to_string())
    }
    pub(crate) fn skill_context(&self,id:&str)->Result<Value,String>{
        if id.is_empty()||id.len()>96||!id.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-'){return Err("Skill inválida.".into());}
        let v=self.edge("skill",json!({"id":id}))?;
        if v["id"]!=id||["name","description","license","source","invocation","body","revision"].iter()
            .any(|key|v.get(key).and_then(Value::as_str).is_none())||!v["resources"].is_object(){
            return Err("O catálogo retornou uma skill inválida.".into());
        }
        let data=json!({"id":id,"name":v["name"],"description":v["description"],"license":v["license"],
            "source":v["source"],"invocation":v["invocation"],"revision":v["revision"],"body":v["body"],"resources":v["resources"]});
        if data.to_string().len()>64000{return Err("A skill excede o limite de contexto.".into());}
        Ok(data)
    }
    pub(crate) fn edge(&self, action:&str, mut payload:Value)->Result<Value,String>{
        let bearer=self.bearer()?;
        Self::edge_with(bearer.as_str(),action,&mut payload)
    }
    fn edge_with(bearer:&str,action:&str,payload:&mut Value)->Result<Value,String>{
        payload["action"]=json!(action);
        response(client()?.post(EDGE).header("apikey",PUBLISHABLE).bearer_auth(bearer)
            .json(&payload).send().map_err(|_|"Serviço comercial indisponível.")?)
    }
    pub(crate) fn handle(&self,action:&str,value:Value)->Result<Value,String>{
        if !value.is_object() || value.to_string().len()>65_536{return Err("Solicitação da conta inválida.".into());}
        if action=="account.site" {
            let path=match value["page"].as_str(){Some("account")=>"/conta/",Some("support")=>"/suporte/",
                Some("plans")=>"/precos/",Some("privacy")=>"/privacidade/",Some("terms")=>"/termos/",
                Some("refund")=>"/reembolso/",Some("signup")=>"/conta/?intent=cadastro",Some("signin")=>"/conta/?intent=login",
                _=>return Err("Página desconhecida.".into())};
            open_site(&format!("{SITE}{path}"))?;return Ok(Value::Null);
        }
        if action=="account.session"{return Ok(json!({"user":self.session()?.map(|s|s.user)}));}
        let operation=match action {
            "account.overview"=>"account", "account.consent"=>"consent","account.marketing"=>"marketing",
            "account.checkout"=>"checkout","account.cancel"=>"cancel","account.support"=>"support",
            "account.supportList"=>"support_list","account.export"=>"export",
            "account.models"=>"models","account.skills"=>"skills","account.skill"=>"skill",_=>""
        };
        if !operation.is_empty(){
            let result=self.edge(operation,value)?;
            return Ok(result);
        }
        let _g=self.guard.try_lock().map_err(|_|"Já há uma operação de conta em andamento.")?;
        if action=="account.delete"{
            let (bearer,_)=self.current()?;
            let result=Self::edge_with(&bearer,"delete",&mut value.clone())?;
            self.remove()?;return Ok(result);
        }
        if action=="account.signOut" {
            if let Some(s)=self.session()?{
                let revoked=auth("logout?scope=local",json!({}),Some(&s.access_token)).is_ok();
                self.remove()?;return Ok(json!({"revoked":revoked}));
            }
            return Ok(json!({"revoked":true}));
        }
        if action=="account.recover" {
            open_site(&format!("{SITE}/conta/?intent=recuperar"))?;
            return Ok(Value::Null);
        }
        if self.session()?.is_some(){return Err("Saia da conta atual antes de entrar em outra.".into());}
        let raw=match action {
            "account.signIn"=>{
                let mut credentials=json!({"email":email(&value)?,"password":password(&value)?});
                match auth("token?grant_type=password",credentials.clone(),None) {
                    Err(reason) if reason=="captcha_required"=>{
                        credentials["gotrue_meta_security"]=json!({"captcha_token":browser_return("captcha",None)?});
                        auth("token?grant_type=password",credentials,None)?
                    }
                    result=>result?,
                }
            },
            "account.google"=>google()?,
            _=>return Err("Operação de conta desconhecida.".into()),
        };
        let session=parse_session(raw)?;
        self.save(&session)?;
        Ok(json!({"user":session.user}))
    }
}

fn open_site(target:&str)->Result<(),String>{
    let url=Url::parse(target).map_err(|_|"Página inválida.")?;
    if url.origin().ascii_serialization()!=SITE{return Err("Origem da página não permitida.".into());}
    #[cfg(windows)] {
        use windows::{core::{PCWSTR, HSTRING}, Win32::UI::{Shell::ShellExecuteW,WindowsAndMessaging::SW_SHOWNORMAL}};
        let result=unsafe{ShellExecuteW(None,&HSTRING::from("open"),&HSTRING::from(target),PCWSTR::null(),PCWSTR::null(),SW_SHOWNORMAL)};
        if result.0 as isize>32{return Ok(());}
    }
    Err("Não foi possível abrir a página no navegador padrão.".into())
}
fn callback(data:&str,port:u16,nonce:&str,field:&str)->Option<String>{
    let mut lines=data.split("\r\n");
    let mut fields=lines.next()?.split_whitespace();
    if fields.next()!=Some("GET"){return None;}
    let path=fields.next()?;
    if !matches!(fields.next(),Some("HTTP/1.1"|"HTTP/1.0"))||fields.next().is_some() || !path.starts_with("/openkoda/callback?"){return None;}
    let expected=format!("127.0.0.1:{port}");
    let hosts=lines.filter_map(|h|h.split_once(':')).filter(|(k,_)|k.eq_ignore_ascii_case("host")).collect::<Vec<_>>();
    if hosts.len()!=1||hosts[0].1.trim()!=expected{return None;}
    let parsed=Url::parse(&format!("http://{expected}{path}")).ok()?;
    let query=parsed.query_pairs().collect::<Vec<_>>();
    if query.len()!=2||!query.iter().any(|(k,v)|k=="state"&&v==nonce){return None;}
    let limit=if field=="captcha"{2048}else{512};
    query.iter().find(|(k,v)|k==field&&v.len()>=8&&v.len()<=limit&&v.bytes().all(|b|b.is_ascii_alphanumeric()||b"-._~".contains(&b))).map(|(_,v)|v.to_string())
}
fn callback_stream(mut stream:TcpStream,port:u16,state:&str,field:&str)->Option<String>{
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut data=Vec::new();let mut chunk=[0_u8;1024];
    while !data.windows(4).any(|v|v==b"\r\n\r\n")&&data.len()<8192{
        let n=stream.read(&mut chunk).ok()?;if n==0{return None;}data.extend_from_slice(&chunk[..n]);
    }
    let code=callback(std::str::from_utf8(&data).ok()?,port,state,field);
    let message=if code.is_some(){"200 OK\r\n\r\nVolte ao OpenKoda para concluir o login."}else{"400 Bad Request\r\n\r\nSolicitacao invalida."};
    let (status,body)=message.split_once("\r\n\r\n")?;
    let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{body}");
    code
}
fn google()->Result<Value,String>{
    let verifier=Zeroizing::new(format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple()));
    let challenge=URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let code=browser_return("google",Some(&challenge))?;
    auth("token?grant_type=pkce",json!({"auth_code":code,"code_verifier":verifier.as_str()}),None)
}
fn browser_return(intent:&str,challenge:Option<&str>)->Result<String,String>{
    let listener=TcpListener::bind(("127.0.0.1",0)).map_err(|_|"Não foi possível preparar o retorno do Google.")?;
    listener.set_nonblocking(true).map_err(|_|"Retorno local indisponível.")?;
    let port=listener.local_addr().map_err(|_|"Porta local indisponível.")?.port();
    let state=Uuid::new_v4().to_string();
    let mut link=Url::parse(&format!("{SITE}/conta/")).map_err(|_|"Origem inválida.")?;
    link.query_pairs_mut().append_pair("desktop","1").append_pair("port",&port.to_string()).append_pair("state",&state)
        .append_pair("intent",intent);
    if let Some(hash)=challenge {link.query_pairs_mut().append_pair("code_challenge",hash);}
    open_site(link.as_str())?;
    let deadline=Instant::now()+Duration::from_secs(180);
    while Instant::now()<deadline {
        match listener.accept() {
            Ok((stream,peer)) if peer.ip().is_loopback()=>if let Some(code)=callback_stream(stream,port,&state,if intent=="captcha"{"captcha"}else{"code"}){
                return Ok(code)
            },
            Ok(_)=>{},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(100)),
            Err(_)=>return Err("Retorno do login interrompido.".into()),
        }
    }
    Err("O prazo de login pelo navegador expirou.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn callback_binds_origin_state_and_code() {
        let input="GET /openkoda/callback?state=opaque&code=valid-code HTTP/1.1\r\nHost: 127.0.0.1:12345\r\n\r\n";
        assert_eq!(callback(input,12345,"opaque","code").as_deref(),Some("valid-code"));
        assert!(callback(input,12345,"opaque","captcha").is_none());
        for other in [input.replace("opaque","different"),input.replace("127.0.0.1","evil.test"),
            input.replace("code=valid-code","code=valid-code&code=valid-code"),input.replace("GET ","POST ")]{
            assert!(callback(&other,12345,"opaque","code").is_none());
        }
    }
    #[cfg(windows)]
    #[test]fn encrypted_session_survives_reopen_and_logout_removes_it(){
        let dir=tempfile::tempdir().unwrap();
        let user=Uuid::new_v4();let revision=Uuid::new_v4();
        let claims=URL_SAFE_NO_PAD.encode(json!({"sub":user,"session_id":revision}).to_string());
        let token=format!("synthetic-header.{claims}.synthetic-signature");
        let account=Account::new(dir.path());
        account.save(&Session{access_token:token.clone(),refresh_token:"synthetic-refresh-token".into(),
            expires_at:chrono::Utc::now().timestamp()+600,user:Identity{id:user,email:"fixture@example.invalid".into()}}).unwrap();
        assert!(!String::from_utf8_lossy(&std::fs::read(&account.file).unwrap()).contains("synthetic-refresh-token"));
        let reopened=Account::new(dir.path());
        let (bearer,rev)=reopened.credential().unwrap();
        assert_eq!(bearer.as_str(),token);assert_eq!(rev,revision.to_string());
        reopened.remove().unwrap();assert!(reopened.credential().is_err());assert!(reopened.identity_revision().unwrap().is_none());
    }
}
