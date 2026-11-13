//! Extractive local context. The caller recomputes every included turn from
//! its current data-use receipt before a checkpoint can enter a reviewed packet.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn excerpt(text:&str)->String {
    let count=text.chars().count();
    if count<=256{return text.to_owned();}
    let head=text.chars().take(176).collect::<String>();
    let end=text.chars().skip(count-72).collect::<String>();
    format!("{head} … {end}")
}
pub(super) fn project(db:&Connection, assessment:&str, entries:&[(String,Vec<Value>)], forced:bool, history_budget:usize)->Result<(Vec<Value>,usize),String>{
    let originals:Vec<_>=entries.iter().flat_map(|(_,messages)|messages).cloned().collect();
    let bytes=serde_json::to_vec(&originals).map_err(|_|"Histórico inválido.")?.len();
    // Reuse the boundary requested by the user after a reload, but only while
    // the request at that boundary is still among the authorized originals.
    let latest:Option<String>=db.query_row("SELECT id FROM requests WHERE assessment=?1 AND json_type(details,'$.contextCheckpoint')='object' ORDER BY rowid DESC LIMIT 1",
        [assessment],|row|row.get(0)).optional().map_err(|_|"Não foi possível ler o cursor da compactação.")?;
    let saved_boundary=latest.and_then(|id|entries.iter().position(|(rid,_)|rid==&id)).map_or(0,|i|i+1);
    if !forced&&saved_boundary==0&&bytes<=history_budget{
        return Ok((originals,0));
    }
    if entries.is_empty(){return Ok((Vec::new(),0));}
    if history_budget<512{return Err("O modelo não tem espaço para o histórico revisado. Reduza a mensagem ou escolha um modelo com mais contexto.".into());}
    // Preserve up to two recent turns verbatim when they fit the context budget.
    let mut first_recent=entries.len();let mut recent_bytes=0;
    for (_,messages) in entries.iter().rev().take(2){
        let size=serde_json::to_vec(messages).map_err(|_|"Histórico inválido.")?.len();
        if recent_bytes+size>history_budget*55/100{break;}recent_bytes+=size;first_recent-=1;
    }
    if (forced||bytes>history_budget)&&first_recent==0{first_recent=1;}
    first_recent=first_recent.max(saved_boundary);
    if first_recent==0{return Ok((entries.iter().flat_map(|(_,m)|m.iter().cloned()).collect(),0));}
    let earlier=&entries[..first_recent];
    // Do not seed the next summary from an obsolete checkpoint. The current
    // authorized originals are the only inputs, even after revocation or edits.
    let recent:Vec<_>=entries[first_recent..].iter().flat_map(|(_,messages)|messages.iter().cloned()).collect();
    let mut extracts=std::collections::BTreeMap::new();
    // Reserve the first authorized user statement before spending space on
    // later replies. The assessment objective and selected evidence stay in
    // the separately reviewed payload and are never reconstructed from quotes.
    let opening=earlier[0].1.iter().find(|m|m["role"]=="user").and_then(|m|m["text"].as_str())
        .ok_or("O histórico autorizado não contém a mensagem inicial.")?;
    let opening_quote=excerpt(opening);
    let opening_truncated=opening_quote!=opening;
    let opening=json!({"requestId":earlier[0].0,"excerpt":opening_quote,"truncated":opening_truncated});
    let checkpoint_for=|rows:&std::collections::BTreeMap<usize,Value>| {
        let represented=1+rows.keys().filter(|index|**index!=0).count();
        json!({"kind":"extractive_history","firstAuthorizedUserTurn":opening,"turnsCovered":represented,
            "omittedTurns":earlier.len()-represented,"extracts":rows.values().collect::<Vec<_>>()})
    };
    let fits=|checkpoint:&Value| {
        // Serialize against the actual budget, including separators and escaped bytes.
        checkpoint.to_string().len()+recent.iter().map(|m|m.to_string().len()+1).sum::<usize>()+2<=history_budget
    };
    if !fits(&checkpoint_for(&extracts)){return Err("O contexto restante não comporta a mensagem inicial autorizada. Reduza a mensagem ou escolha um modelo com mais contexto.".into());}
    for (index,(request,messages)) in earlier.iter().enumerate().rev(){
        let quotes=messages.iter().filter(|m|index!=0||m["role"]!="user").map(|m|{
            let text=m["text"].as_str().unwrap_or_default();let quote=excerpt(text);
            json!({"role":m["role"],"excerpt":quote,"truncated":quote!=text})
        }).collect::<Vec<_>>();
        if quotes.is_empty(){continue;}
        extracts.insert(index,json!({"requestId":request,"quotes":quotes}));
        if !fits(&checkpoint_for(&extracts)){extracts.remove(&index);}
    }
    let checkpoint=checkpoint_for(&extracts);
    let covered=checkpoint["turnsCovered"].as_u64().unwrap_or(0) as usize;
    let encoded=checkpoint.to_string();
    let cursor=&earlier.last().unwrap().0;
    let digest=format!("{:x}",Sha256::digest(encoded.as_bytes()));
    // Co-locate the checkpoint with its existing request: project purge, backup
    // validation and restoration keep the same table, ownership and lifetime.
    db.execute("UPDATE requests SET details=json_set(details,'$.contextCheckpoint',json_object('cursor',?2,'digest',?3,'summary',?4)) WHERE assessment=?1 AND id=?2 AND coalesce(json_extract(details,'$.contextCheckpoint.digest'),'')!=?3",
        params![assessment,cursor,digest,encoded]).map_err(|_|"Não foi possível salvar a compactação. O histórico foi preservado.")?;
    let saved:Option<String>=db.query_row("SELECT json_extract(details,'$.contextCheckpoint.summary') FROM requests WHERE assessment=?1 AND id=?2 AND json_extract(details,'$.contextCheckpoint.digest')=?3",params![assessment,cursor,digest],|row|row.get(0)).optional().map_err(|_|"Não foi possível conferir a compactação.")?;
    if saved.as_deref()!=Some(encoded.as_str()){return Err("A compactação mudou. Prepare o contexto novamente.".into());}
    let mut history=vec![checkpoint];history.extend(recent);
    Ok((history,covered))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn tight_budget_pins_the_first_authorized_message_and_counts_only_represented_requests(){
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE requests(id TEXT PRIMARY KEY,assessment TEXT,details TEXT)").unwrap();
        let entries=(0..18).map(|i|{
            let id=format!("request-{i}");db.execute("INSERT INTO requests VALUES(?1,'a','{}')",[&id]).unwrap();
            let text=if i==0 {"Original scope: check invoice receipt EV-42.".to_owned()}else{format!("fact {i} {}","x".repeat(180))};
            (id,vec![json!({"role":"user","text":text}),json!({"role":"assistant","text":"y".repeat(180)})])
        }).collect::<Vec<_>>();
        let (projected,covered)=project(&db,"a",&entries,true,720).unwrap();
        assert!(serde_json::to_vec(&projected).unwrap().len()<=720);
        let summary=&projected[0];
        assert_eq!(summary["firstAuthorizedUserTurn"]["excerpt"],"Original scope: check invoice receipt EV-42.");
        assert_eq!(summary["turnsCovered"],covered);
        let recent=(projected.len()-1)/2;
        assert!(summary["omittedTurns"].as_u64().unwrap()>0);
        assert_eq!(covered+recent+summary["omittedTurns"].as_u64().unwrap() as usize,entries.len());
        assert_eq!(project(&db,"a",&entries,false,720).unwrap().0,projected);
        let allowed=project(&db,"a",&entries[9..],false,720).unwrap().0;
        assert!(!json!(allowed).to_string().contains("EV-42"));
        assert_eq!(allowed[0]["firstAuthorizedUserTurn"]["requestId"],"request-9");
    }
    #[test]fn reopen_rebuilds_only_from_authorized_inputs_and_retries_are_idempotent(){
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("chat.db");
        let entries=(0..8).map(|n|(format!("r{n}"),vec![json!({"role":"user","text":format!("fact {n}")})])).collect::<Vec<_>>();
        let first={let db=Connection::open(&path).unwrap();db.execute_batch("CREATE TABLE requests(id TEXT PRIMARY KEY,assessment TEXT,details TEXT)").unwrap();
            for (id,_) in &entries{db.execute("INSERT INTO requests VALUES(?1,'assessment','{}')",[id]).unwrap();}
            let untouched=project(&db,"assessment",&entries,false,10000).unwrap();
            assert_eq!(untouched.1,0);assert_eq!(untouched.0.len(),8);
            project(&db,"assessment",&entries,true,10000).unwrap()};
        assert_eq!(first.1,6);let reopened=Connection::open(&path).unwrap();
        assert_eq!(project(&reopened,"assessment",&entries,false,10000).unwrap(),first);
        let restricted=project(&reopened,"assessment",&entries[6..],true,10000).unwrap();
        assert_eq!(restricted.1,1);assert!(!json!(restricted.0).to_string().contains("fact 0"));
        assert_eq!(project(&reopened,"assessment",&entries[6..],false,10000).unwrap(),restricted);
        let originals:i64=reopened.query_row("SELECT count(*) FROM requests",[],|r|r.get(0)).unwrap();assert_eq!(originals,8);
    }
}
