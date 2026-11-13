//! A live chat owns one cancellable HTTP future. Dropping that future releases the
//! local connection; the provider might already have received or billed the request.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError};

#[path="ai_chat_stream.rs"]
mod stream;
pub(super) use stream::Progress;
struct ActiveChat { policy:Option<((u64,i64),bool)>, progress: Arc<Mutex<Progress>>, assessment:String, request:String, review:Option<crate::data_policy::DataReviewRef>, accepted:bool, message_id:Option<String>, token:Arc<AtomicBool> }
#[derive(Default)]
pub(super) struct ChatControl(Mutex<Option<ActiveChat>>);
impl ChatControl {
    pub fn start(&self, assessment:&str, request:&str, review:Option<&crate::data_policy::DataReviewRef>)->Result<LiveChat<'_>,String> {
        let token=Arc::new(AtomicBool::new(false));
        let mut state=self.0.lock().map_err(failure)?;
        if state.is_some() {return Err("Já existe uma conversa em execução.".into());}
        let progress=Arc::new(Mutex::new(Progress::default()));
        *state=Some(ActiveChat{policy:None,progress:progress.clone(),assessment:assessment.into(),request:request.into(),review:review.cloned(),accepted:false,message_id:None,token:token.clone()});
        Ok(LiveChat{control:self,token,progress})
    }
    pub fn inspect_with_policy(&self,payload:Value,stop:bool,store:&Store)->Result<Value,String>{
        self.status(payload,stop,Some(store))
    }
    #[cfg(test)]
    fn inspect(&self,payload:Value,stop:bool)->Result<Value,String>{self.status(payload,stop,None)}
    fn status(&self, payload:Value, stop:bool, store:Option<&Store>)->Result<Value,String> {
        #[derive(Deserialize)]
        #[serde(rename_all="camelCase",deny_unknown_fields)]
        struct Input {assessment_id:String, request_id:Option<String>}
        let input:Input=decode(payload)?;
        crate::core::valid_id(&input.assessment_id)?;
        let mut state=self.0.lock().map_err(failure)?;
        let mut current=state.as_mut().filter(|live|live.assessment==input.assessment_id&&live.accepted);
        if stop{
            let live=current.as_ref().ok_or("A resposta já terminou.")?;
            if Some(&live.request)!=input.request_id.as_ref(){return Err("A solicitação ativa mudou.".into());}
            live.token.store(true,Ordering::Release);
        }
        let show_text=if let (Some(live),Some(store))=(current.as_mut(),store){
            if stop {false} else {
                let checked=store.cached_chat_policy(live.policy,|s|{
                    let receipt=s.data_use_receipts.iter().find(|r|r.assessment_id.as_ref()==Some(&live.assessment)&&r.effect.id==live.request);
                    receipt.is_some()&&crate::data_host::local_chat_allowed(s,&live.assessment,receipt)
                })?;
                live.policy=Some(checked);checked.1
            }
        }else{false};
        let current=current.as_deref();
        let progress=current.map(|live|live.progress.lock().map_err(failure)).transpose()?;
        Ok(json!({"processingMs":progress.as_ref().and_then(|p|p.elapsed_ms()),
            "partialText":progress.as_ref().filter(|_|show_text).map(|p|p.visible()),"requestId":current.map(|live|&live.request),"stopping":current.is_some_and(|live|live.token.load(Ordering::Acquire)),
            "messageId":current.and_then(|live|live.message_id.as_ref()),"dataReview":current.and_then(|live|live.review.as_ref()).map(|review|json!({"id":review.id,"digest":review.digest}))}))
    }
}
pub(super) struct LiveChat<'a>{control:&'a ChatControl,pub token:Arc<AtomicBool>,pub progress:Arc<Mutex<Progress>>}
impl LiveChat<'_>{pub fn admit(&self,message_id:Option<String>)->Result<(),String>{
    let mut state=self.control.0.lock().map_err(failure)?;
    let live=state.as_mut().ok_or("Conversa indisponível.")?;
    live.message_id=message_id;live.accepted=true;
    Ok(())
}}
impl Drop for LiveChat<'_>{fn drop(&mut self){if let Ok(mut state)=self.control.0.lock(){state.take();}}}

pub(super) fn http_chat(provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,stop:&AtomicBool,progress:Arc<Mutex<Progress>>)->Result<Value,String>{
    let client=reqwest::Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).no_proxy().timeout(Duration::from_secs(90)).connect_timeout(Duration::from_secs(15)).build().map_err(network_error)?;
    let builder=client.post(provider.inference_url());
    let builder=if provider==ProviderId::Anthropic{builder.header("x-api-key",key).header("anthropic-version","2023-06-01")}else{builder.bearer_auth(key)};
    let mut body=chat_body(provider,model,prompt,instructions,maximum)?;
    body["stream"]=json!(true);
    if matches!(provider,ProviderId::OpenRouter|ProviderId::DeepSeek|ProviderId::Gemini|ProviderId::Xai) {
        body["stream_options"]=json!({"include_usage":true});
    }
    interruptible_response(builder.json(&body),provider,stop,progress)
}

fn interruptible_response(task:reqwest::RequestBuilder,provider:ProviderId,stop:&AtomicBool,progress:Arc<Mutex<Progress>>)->Result<Value,String>{
    let (tx,rx)=sync_channel(1);
    let join=tauri::async_runtime::spawn(async move{
        let result=async{
            let mut response=task.send().await.map_err(network_error)?;
            let status=response.status().as_u16();
            if !(200..300).contains(&status){return Err(match status{401=>"O provedor recusou a credencial.",403=>"A credencial não tem acesso ao recurso.",429=>"Limite ou saldo da API atingido.",_=>"O provedor recusou a solicitação. Confira se o modelo aceita os anexos."}.to_owned());}
            progress.lock().map_err(failure)?.connected();
            let is_sse=response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v|v.to_str().ok())
                .is_some_and(|v|v.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case("text/event-stream"));
            let mut events=stream::Stream::new(provider,progress);
            let mut received=0usize;
            let mut bytes=Vec::new();
            while let Some(chunk)=response.chunk().await.map_err(network_error)?{
                received+=chunk.len();
                if received>MAX_RESPONSE as usize{return Err("Resposta excedeu o limite de tamanho.".into());}
                if is_sse {events.push(&chunk).map_err(|_|"Resposta interrompida durante a transmissão. O consumo é incerto.")?;} else {bytes.extend_from_slice(&chunk); }
            }
            if is_sse {return events.finish().map_err(|_|"Resposta interrompida sem confirmação final do provedor. O consumo é incerto.".into());}
            provider.normalize(serde_json::from_slice(&bytes).map_err(|_|"Resposta do provedor não é JSON válido.")?)
        }.await;
        let _=tx.send(result);
    });
    loop{
        match rx.recv_timeout(Duration::from_millis(80)){
            Ok(result)=>return result,
            Err(RecvTimeoutError::Disconnected)=>return Err("Resposta interrompida. O resultado remoto é incerto.".into()),
            Err(RecvTimeoutError::Timeout) if stop.load(Ordering::Acquire)=>{join.abort();return Err("Resposta interrompida neste dispositivo. O provedor pode ter processado a solicitação.".into());},
            _=>{}
        }
    }
}

fn chat_body(provider:ProviderId,model:&str,prompt:&str,instructions:&str,maximum:u32)->Result<Value,String>{
    let mut envelope:Value=serde_json::from_str(prompt).map_err(|_|"Pacote de conversa inválido.")?;
    // Only the host creates this top-level key while hashing the exact reviewed packet.
    let image=envelope.as_object_mut().and_then(|map|map.remove("reviewedJpeg"));
    let Some(Value::String(encoded))=image else{return Ok(provider.body(model,prompt,instructions,maximum));};
    use base64::Engine;
    let mut body=provider.body(model,&envelope.to_string(),instructions,maximum);
    let url=format!("data:image/jpeg;base64,{encoded}");
    let text=envelope.to_string();
    match provider{
        ProviderId::OpenAI=>body["input"][0]["content"]=json!([{"type":"input_text","text":text},{"type":"input_image","image_url":url}]),
        ProviderId::Anthropic=>body["messages"][0]["content"]=json!([{"type":"text","text":text},{"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":encoded}}]),
        _=>body["messages"][1]["content"]=json!([{"type":"text","text":text},{"type":"image_url","image_url":{"url":url}}]),
    }
    // Enforce at the transport boundary too, before data reaches a provider.
    let bytes=base64::engine::general_purpose::STANDARD.decode(&encoded).map_err(|_|"Imagem JPEG inválida.")?;
    if !bytes.starts_with(&[0xff,0xd8,0xff]) || !bytes.ends_with(&[0xff,0xd9]) || bytes.len()>60*1024{return Err("Imagem JPEG inválida ou acima de 60 KiB.".into());}
    Ok(body)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener, thread, time::Instant};

    #[test]
    fn admission_status_identifies_the_exact_review_and_durable_message_only_after_commit(){
        let control=ChatControl::default();let aid=model::id();let rid=model::id();let mid=model::id();
        let review=crate::data_policy::DataReviewRef{id:model::id(),digest:"reviewed-packet-digest".into(),expires_at:model::now()};
        let live=control.start(&aid,&rid,Some(&review)).unwrap();
        assert!(control.inspect(json!({"assessmentId":aid}),false).unwrap()["requestId"].is_null());
        live.admit(Some(mid.clone())).unwrap();
        let status=control.inspect(json!({"assessmentId":aid}),false).unwrap();
        assert_eq!(status["requestId"],rid);assert_eq!(status["messageId"],mid);
        assert_eq!(status["dataReview"],json!({"id":review.id,"digest":review.digest}));
        let other=control.inspect(json!({"assessmentId":model::id()}),false).unwrap();
        assert!(other["requestId"].is_null());assert!(other["dataReview"].is_null());
        drop(live);
        assert!(control.inspect(json!({"assessmentId":aid}),false).unwrap()["dataReview"].is_null());
    }

    #[test]
    fn cancellation_checks_the_request_and_closes_a_pending_http_connection() {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let addr=listener.local_addr().unwrap();
        let (received,seen)=sync_channel(1);
        let server=thread::spawn(move||{
            let (mut stream,_)=listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(6))).unwrap();
            let mut bytes=[0u8;2048];
            assert!(stream.read(&mut bytes).unwrap()>0);
            received.send(()).unwrap();
            // Never respond. The client should abort the in-flight HTTP request.
            loop {match stream.read(&mut bytes){Ok(0)=>break,Ok(_)=>continue,Err(err) if err.kind()==std::io::ErrorKind::ConnectionReset=>break,Err(other)=>panic!("{other}")}}
        });
        let control=ChatControl::default();
        let aid=model::id();
        let id=model::id();
        let live=control.start(&aid,&id,None).unwrap();
        live.admit(None).unwrap();
        let stopping=live.token.clone();
        let request=reqwest::Client::builder().no_proxy().build().unwrap().get(format!("http://{addr}/fixture"));
        let caller=thread::spawn(move||interruptible_response(request,ProviderId::OpenAI,&stopping,Arc::new(Mutex::new(Progress::default()))));
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(control.inspect(json!({"assessmentId":aid,"requestId":model::id()}),true).is_err());
        assert!(!live.token.load(Ordering::Acquire));
        let start=Instant::now();
        assert_eq!(control.inspect(json!({"assessmentId":aid,"requestId":id}),true).unwrap()["stopping"],true);
        assert!(caller.join().unwrap().unwrap_err().starts_with("Resposta interrompida"));
        server.join().unwrap();
        assert!(start.elapsed()<Duration::from_secs(5));
        drop(live);
        assert!(control.inspect(json!({"assessmentId":aid}),false).unwrap()["requestId"].is_null());
    }

    #[test]
    fn reviewed_images_are_multimodal_and_user_text_cannot_inject_an_image_part() {
        use base64::Engine;
        let encoded=base64::engine::general_purpose::STANDARD.encode([0xff,0xd8,0xff,0,0xff,0xd9]);
        let plain=json!({"payload":format!("Here is a user string: reviewedJpeg {encoded}")}).to_string();
        assert!(chat_body(ProviderId::OpenRouter,"fixture",&plain,"review",10).unwrap()["messages"][1]["content"].is_string());
        let envelope=json!({"payload":"Review the synthetic image","reviewedJpeg":encoded}).to_string();
        let openai=chat_body(ProviderId::OpenAI,"fixture",&envelope,"review",10).unwrap();
        assert_eq!(openai["input"][0]["content"][1]["type"],"input_image");
        let router=chat_body(ProviderId::OpenRouter,"fixture",&envelope,"review",10).unwrap();
        assert_eq!(router["messages"][1]["content"][1]["image_url"]["url"],format!("data:image/jpeg;base64,{encoded}"));
        assert!(!router["messages"][1]["content"][0]["text"].as_str().unwrap().contains(&encoded));
    }
}
