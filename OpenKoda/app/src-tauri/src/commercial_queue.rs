//! A managed inference may be retried only while the server confirms it has
//! not been dispatched. Transport failures and ordinary rate limits are final.
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WaitTicket {
    code: String,
    request_id: Uuid,
    queue_token: Uuid,
    position: u32,
    retry_after_seconds: u64,
    expires_at: String,
}

struct Waiting {
    cancellation: Option<RequestBuilder>,
    token: Option<Uuid>,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        // Aborting the enclosing chat or scan drops this future too. Expiry on
        // the server reclaims the slot even if the client goes offline.
        if let Some(cancel) = self.cancellation.take() {
            tauri::async_runtime::spawn(async move {
                let _ = cancel.timeout(Duration::from_secs(3)).send().await;
            });
        }
    }
}

async fn ticket(response: &mut Response, id: Uuid) -> Result<WaitTicket, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "A conexão com a fila foi interrompida.")?
    {
        if bytes.len() + chunk.len() > 4096 {
            return Err("Resposta inválida da fila de assinatura.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let error = "Limite da assinatura ou de solicitações atingido. Consulte o histórico de uso.";
    let t: WaitTicket = serde_json::from_slice(&bytes).map_err(|_| error)?;
    if t.code != "capacity_wait"
        || t.request_id != id
        || t.queue_token.is_nil()
        || !(1..=64).contains(&t.position)
        || !(1..=10).contains(&t.retry_after_seconds)
    {
        return Err(error.into());
    }
    Ok(t)
}

pub(crate) async fn send(
    request: RequestBuilder,
    request_id: &str,
    on_position: impl Fn(Option<u32>),
) -> Result<Response, String> {
    let id = Uuid::parse_str(request_id).map_err(|_| "Identificador de solicitação inválido.")?;
    let limit = Instant::now() + Duration::from_secs(125);
    let mut waiting = Waiting {
        cancellation: None,
        token: None,
    };
    let duplicate = || {
        request
            .try_clone()
            .ok_or("O pedido não pode aguardar na fila.".to_owned())
    };
    loop {
        let mut attempt = duplicate()?;
        if let Some(token) = waiting.token {
            attempt = attempt.header("X-Koda-Queue-Token", token.to_string());
        }
        let mut response = attempt
            .send()
            .await
            .map_err(|_| "Conexão com o serviço de assinatura interrompida.")?;
        if response.status() != StatusCode::TOO_MANY_REQUESTS {
            waiting.cancellation = None;
            on_position(None);
            return Ok(response);
        }
        let t = ticket(&mut response, id).await?;
        if waiting.token.is_some_and(|token| token != t.queue_token) {
            return Err("O identificador da fila mudou. Inicie outra solicitação.".into());
        }
        if waiting.token.is_none() {
            waiting.token = Some(t.queue_token);
            waiting.cancellation = Some(duplicate()?.json(&json!({
                "action":"cancel_wait", "requestId":id, "queueToken":t.queue_token
            })));
        }
        let pause = Duration::from_secs(t.retry_after_seconds);
        let expires = chrono::DateTime::parse_from_rfc3339(&t.expires_at)
            .map_err(|_| "Prazo de espera da fila inválido.")?;
        if Instant::now() + pause >= limit || expires <= chrono::Utc::now() {
            return Err("O tempo de espera da fila terminou. Tente novamente.".into());
        }
        on_position(Some(t.position));
        tokio::time::sleep(pause).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{mpsc, Mutex};
    use std::thread;

    fn server(
        replies: Vec<(u16, serde_json::Value)>,
    ) -> (String, mpsc::Receiver<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let (events, requests) = mpsc::channel();
        thread::spawn(move || {
            for (status, value) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                    headers.push_str(&line.to_ascii_lowercase());
                }
                let count = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                let mut body = vec![0; count];
                reader.read_exact(&mut body).unwrap();
                events
                    .send((headers, String::from_utf8(body).unwrap()))
                    .unwrap();
                let bytes = value.to_string();
                write!(stream,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",bytes.len()).unwrap();
            }
        });
        (url, requests)
    }
    fn pending(id: Uuid, token: Uuid) -> serde_json::Value {
        json!({"code":"capacity_wait","requestId":id,"queueToken":token,"position":2,
            "retryAfterSeconds":1, "expiresAt":(chrono::Utc::now()+chrono::Duration::seconds(90)).to_rfc3339()})
    }
    #[test]
    fn only_capacity_wait_replays_identical_request_with_ticket() {
        let id = Uuid::new_v4();
        let token = Uuid::new_v4();
        let (url, rx) = server(vec![
            (429, pending(id, token)),
            (200, json!({"accepted":true})),
        ]);
        let body = json!({"action":"scan","requestId":id,"messages":["example"]});
        let request = reqwest::Client::new().post(url).json(&body);
        let positions = Mutex::new(Vec::new());
        let status = tauri::async_runtime::block_on(send(request, &id.to_string(), |p| {
            positions.lock().unwrap().push(p)
        }))
        .unwrap()
        .status();
        assert_eq!(status, 200);
        assert_eq!(positions.into_inner().unwrap(), vec![Some(2), None]);
        let (first, body_first) = rx.recv().unwrap();
        let (second, body_second) = rx.recv().unwrap();
        assert!(!first.contains("x-koda-queue-token"));
        assert!(second.contains(&format!("x-koda-queue-token: {token}")));
        assert_eq!(body_first, body_second);
        for denied in [
            json!({"code":"quota_exceeded"}),
            pending(Uuid::new_v4(), token),
        ] {
            let (url, rx) = server(vec![(429, denied)]);
            assert!(tauri::async_runtime::block_on(send(
                reqwest::Client::new().post(url).json(&body),
                &id.to_string(),
                |_| {}
            ))
            .is_err());
            assert_eq!(rx.iter().count(), 1);
        }
    }
    #[test]
    fn aborting_wait_sends_only_the_authenticated_cancellation() {
        let id = Uuid::new_v4();
        let token = Uuid::new_v4();
        let (url, rx) = server(vec![
            (429, pending(id, token)),
            (200, json!({"cancelled":true})),
        ]);
        let (ready, waiting) = mpsc::channel();
        let task = tauri::async_runtime::spawn(async move {
            let request = reqwest::Client::new()
                .post(url)
                .bearer_auth("local-test-only")
                .json(&json!({"action":"chat","requestId":id,"messages":["private text"]}));
            send(request, &id.to_string(), |p| {
                if p.is_some() {
                    ready.send(()).unwrap();
                }
            })
            .await
        });
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        task.abort();
        let _first = rx.recv_timeout(Duration::from_secs(4)).unwrap();
        let (headers, body) = rx.recv_timeout(Duration::from_secs(4)).unwrap();
        assert!(headers.contains("authorization: bearer local-test-only"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
            json!({"action":"cancel_wait","requestId":id,"queueToken":token})
        );
    }
}
