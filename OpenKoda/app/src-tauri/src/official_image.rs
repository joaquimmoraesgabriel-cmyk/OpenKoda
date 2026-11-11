//! Verified acquisition of the Kali amd64 image advertised by Microsoft's WSL manifest.
//! A selected local file is only an optimization; its bytes must match that manifest.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest,Sha256};
use std::{fs,io::{Read,Seek,SeekFrom,Write},path::{Path,PathBuf},time::Duration};

pub const MANIFEST_URL:&str="https://api.github.com/repos/microsoft/WSL/contents/distributions/DistributionInfo.json?ref=master";
const MAX_MANIFEST:u64=4*1024*1024;
const MAX_IMAGE:u64=4*1024*1024*1024;

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct VerifiedImage {pub manifest_url:String,pub url:String,pub sha256:String,pub bytes:u64,pub local_file:String}
pub trait ImageSource:Send+Sync {fn prepare(&self,root:&Path,selected:Option<&str>)->Result<VerifiedImage,String>;}
pub struct OfficialImageSource;

fn canonical(path:&Path)->Result<PathBuf,String>{path.canonicalize().map_err(|_|"Não foi possível resolver o caminho da imagem oficial.".into())}
fn plain(path:&Path,directory:bool)->Result<(),String>{
    let metadata=fs::symlink_metadata(path).map_err(|_|"Imagem ou cache oficial indisponível.".to_string())?;
    #[cfg(windows)] let reparse={use std::os::windows::fs::MetadataExt;metadata.file_attributes()&0x400!=0};
    #[cfg(not(windows))] let reparse=false;
    if reparse||metadata.file_type().is_symlink()||if directory{!metadata.is_dir()}else{!metadata.is_file()}{return Err("Imagem/cache deve ser regular, sem links ou reparse points.".into());}Ok(())
}
fn cache_root(root:&Path)->Result<PathBuf,String>{
    let root=canonical(root)?;let cache=root.join("image-cache");
    if !cache.exists(){fs::create_dir(&cache).map_err(|_|"Não foi possível preparar o cache de imagens.".to_string())?;}
    plain(&cache,true)?;let resolved=canonical(&cache)?;
    if resolved.parent()!=Some(root.as_path()){return Err("Cache de imagens fora da instalação gerida.".into());}Ok(resolved)
}
fn locked_file(path:&Path)->Result<fs::File,String>{
    plain(path,false)?;let mut options=fs::OpenOptions::new();options.read(true);
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}
    options.open(path).map_err(|_|"A imagem não pode ser bloqueada para leitura; feche processos que a estejam alterando.".into())
}
fn hash_reader(reader:&mut impl Read,mut output:Option<&mut fs::File>)->Result<(String,u64),String>{
    let mut sha=Sha256::new();let mut bytes=0;let mut buffer=[0u8;256*1024];
    loop{let count=reader.read(&mut buffer).map_err(|_|"Aquisição da imagem foi interrompida; nenhum ambiente foi alterado.".to_string())?;if count==0{break;}bytes+=count as u64;
        if bytes>MAX_IMAGE{return Err("Imagem excede o limite de 4 GiB desta aquisição.".into());}
        sha.update(&buffer[..count]);if let Some(file)=output.as_mut(){file.write_all(&buffer[..count]).map_err(|_|"Não foi possível salvar a imagem; verifique espaço em disco.".to_string())?;}
    }Ok((format!("{:x}",sha.finalize()),bytes))
}
pub fn manifest_image(value:&Value)->Result<(String,String),String>{
    let item=value.get("ModernDistributions").and_then(|v|v.get("kali")).and_then(Value::as_array).and_then(|items|items.first()).and_then(|v|v.get("Amd64Url")).ok_or("O catálogo oficial não contém a imagem Kali amd64 esperada.")?;
    let url=item.get("Url").and_then(Value::as_str).ok_or("URL da imagem oficial ausente.")?;
    let sha=item.get("Sha256").and_then(Value::as_str).ok_or("SHA-256 da imagem oficial ausente.")?.to_ascii_lowercase();
    validate_source(url,&sha)?;Ok((url.into(),sha))
}
fn validate_source(image_url:&str,sha:&str)->Result<(),String>{
    let url=url::Url::parse(image_url).map_err(|_|"URL da imagem inválida.".to_string())?;
    if url.scheme()!="https"||url.host_str()!=Some("kali.download")||url.port().is_some()||!url.username().is_empty()||url.password().is_some()||url.query().is_some()||url.fragment().is_some()||!url.path().starts_with("/wsl-images/kali-")||!url.path().ends_with("-wsl-rootfs-amd64.wsl")||url.path().contains('%')||sha.len()!=64||!sha.bytes().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase()){return Err("Manifest não corresponde ao canal HTTPS oficial Kali amd64 permitido.".into());}Ok(())
}
pub fn verify_pinned(root:&Path,image:&VerifiedImage)->Result<fs::File,String>{
    if image.manifest_url!=MANIFEST_URL{return Err("A imagem não está vinculada ao catálogo oficial WSL permitido.".into());}
    validate_source(&image.url,&image.sha256)?;
    let expected=cache_root(root)?.join(format!("{}.wsl",image.sha256));
    if canonical(Path::new(&image.local_file))?!=expected{return Err("A imagem está fora do cache gerido ou sua identidade mudou.".into());}
    let mut file=locked_file(&expected)?;let (sha,bytes)=hash_reader(&mut file,None)?;
    if sha!=image.sha256||bytes!=image.bytes||bytes==0{return Err("A imagem mudou após o planejamento; a instalação não será iniciada.".into());}
    file.seek(SeekFrom::Start(0)).map_err(|_|"Imagem não pôde ser reposicionada.".to_string())?;Ok(file)
}
fn stage(root:&Path,url:String,sha:String,reader:&mut impl Read)->Result<VerifiedImage,String>{
    validate_source(&url,&sha)?;let cache=cache_root(root)?;let target=cache.join(format!("{sha}.wsl"));
    if target.exists(){let mut file=locked_file(&target)?;let (actual,bytes)=hash_reader(&mut file,None)?;if actual==sha&&bytes>0{return Ok(VerifiedImage{manifest_url:MANIFEST_URL.into(),url,sha256:sha,bytes,local_file:target.to_string_lossy().into_owned()});}return Err("Há uma imagem inconsistente no cache; o arquivo foi preservado para diagnóstico.".into());}
    let temporary=cache.join(format!("{}.partial",uuid::Uuid::new_v4()));let mut output=fs::OpenOptions::new().create_new(true).write(true).open(&temporary).map_err(|_|"Não foi possível criar arquivo temporário da imagem.".to_string())?;
    let result=(||{let (actual,bytes)=hash_reader(reader,Some(&mut output))?;if actual!=sha||bytes==0{return Err("O SHA-256 da imagem não corresponde ao manifest oficial. Nenhuma distribuição foi criada.".into());}
        output.sync_all().map_err(|_|"Não foi possível confirmar a imagem em disco.".to_string())?;Ok(bytes)})();drop(output);
    match result{
        Ok(bytes)=>{fs::rename(&temporary,&target).map_err(|_|"Não foi possível publicar a imagem verificada no cache.".to_string())?;Ok(VerifiedImage{manifest_url:MANIFEST_URL.into(),url,sha256:sha,bytes,local_file:target.to_string_lossy().into_owned()})},
        Err(error)=>{let _=fs::remove_file(&temporary);Err(error)}
    }
}
impl ImageSource for OfficialImageSource {
    fn prepare(&self,root:&Path,selected:Option<&str>)->Result<VerifiedImage,String>{
        let client=reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(1200)).build().map_err(|_|"Aquisição HTTPS indisponível.".to_string())?;
        let response=client.get(MANIFEST_URL).header("Accept","application/vnd.github.raw+json").header("User-Agent","OpenKoda-official-WSL-image").timeout(Duration::from_secs(45)).send().map_err(|_|"Não foi possível consultar o manifest oficial WSL pelo GitHub. Nenhum fallback não verificado foi usado.".to_string())?;
        if !response.status().is_success(){return Err("O catálogo oficial WSL recusou a consulta; tente novamente após resolver a conectividade.".into());}
        let mut bytes=Vec::new();response.take(MAX_MANIFEST+1).read_to_end(&mut bytes).map_err(|_|"Manifest oficial incompleto.".to_string())?;if bytes.len() as u64>MAX_MANIFEST{return Err("Manifest oficial excedeu o limite.".into());}
        let mut manifest:Value=serde_json::from_slice(&bytes).map_err(|_|"Manifest oficial não é JSON válido.".to_string())?;
        if manifest["encoding"]=="base64" {
            let text=manifest["content"].as_str().ok_or("Conteúdo oficial ausente.")?.replace(['\n','\r'],"");let decoded=STANDARD.decode(text).map_err(|_|"Conteúdo oficial inválido.".to_string())?;
            manifest=serde_json::from_slice(&decoded).map_err(|_|"Manifest oficial não é JSON válido.".to_string())?;
        }
        let (url,sha)=manifest_image(&manifest)?;
        let cache=cache_root(root)?;let existing=cache.join(format!("{sha}.wsl"));
        if existing.exists(){let mut input=locked_file(&existing)?;return stage(root,url,sha,&mut input);}
        if let Some(selected)=selected {
            let path=Path::new(selected);if !path.is_absolute()||path.extension().is_none_or(|s|s!="wsl"){return Err("Selecione um arquivo .wsl absoluto para conferir com o manifest oficial.".into());}
            let resolved=canonical(path)?;let mut input=locked_file(&resolved)?;return stage(root,url,sha,&mut input);
        }
        let mut response=client.get(&url).header("User-Agent","OpenKoda-official-WSL-image").send().map_err(|_|"Download oficial não iniciou; nenhuma distribuição foi criada.".to_string())?;
        if !response.status().is_success()||response.content_length().is_some_and(|length|length>MAX_IMAGE){return Err("Servidor oficial recusou a imagem ou informou tamanho acima do limite.".into());}
        stage(root,url,sha,&mut response)
    }
}

#[cfg(test)]mod tests{
    use super::*;
    const URL:&str="https://kali.download/wsl-images/kali-2026.2/kali-linux-2026.2-wsl-rootfs-amd64.wsl";
    fn manifest(url:&str,sha:&str)->Value{serde_json::json!({"ModernDistributions":{"kali":[{"Amd64Url":{"Url":url,"Sha256":sha}}]}})}
    #[test]fn manifest_requires_exact_official_architecture_and_origin(){let sha="a".repeat(64);assert!(manifest_image(&manifest(URL,&sha)).is_ok());for bad in ["http://kali.download/wsl-images/kali-x/kali-wsl-rootfs-amd64.wsl","https://evil.test/wsl-images/kali-x/kali-wsl-rootfs-amd64.wsl","https://kali.download/wsl-images/kali-x/kali-wsl-rootfs-arm64.wsl"]{assert!(manifest_image(&manifest(bad,&sha)).is_err());}assert!(manifest_image(&manifest(URL,"forged")).is_err());}
    #[test]fn selected_bytes_are_verified_and_changed_cache_is_refused(){let dir=tempfile::tempdir().unwrap();let content=b"synthetic image";let sha=format!("{:x}",Sha256::digest(content));let image=stage(dir.path(),URL.into(),sha,&mut std::io::Cursor::new(content)).unwrap();assert!(verify_pinned(dir.path(),&image).is_ok());fs::write(&image.local_file,b"tampered").unwrap();assert!(verify_pinned(dir.path(),&image).is_err());}
    #[test]fn mismatched_image_never_becomes_installable(){let dir=tempfile::tempdir().unwrap();assert!(stage(dir.path(),URL.into(),"a".repeat(64),&mut std::io::Cursor::new(b"wrong")).is_err());assert_eq!(fs::read_dir(dir.path().join("image-cache")).unwrap().count(),0);}
    #[test]fn plan_cannot_substitute_arbitrary_local_path(){let dir=tempfile::tempdir().unwrap();let content=b"fixture";let sha=format!("{:x}",Sha256::digest(content));let mut image=stage(dir.path(),URL.into(),sha,&mut std::io::Cursor::new(content)).unwrap();let other=dir.path().join("other.wsl");fs::copy(&image.local_file,&other).unwrap();image.local_file=other.to_string_lossy().into_owned();assert!(verify_pinned(dir.path(),&image).is_err());}
}
