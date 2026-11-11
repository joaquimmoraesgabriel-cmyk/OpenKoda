//! Bounded TCP connection observations. Exact literal destinations only; no DNS,
//! proxy, child process, banner acquisition, protocol payload or credential.
use serde_json::{json, Value};
use std::{collections::BTreeSet, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    sync::atomic::{AtomicBool, Ordering}, time::{Duration, Instant}};

pub const ADAPTER: &str = "network.tcp-connect.v1";
const MAX_HOSTS: u128 = 16;
const MAX_CONNECTIONS: usize = 64;

fn hosts(target: &str) -> Result<Vec<IpAddr>, String> {
    let (ip, prefix) = target.split_once('/').map_or((target, None), |(a,b)| (a, Some(b)));
    let address: IpAddr = ip.parse().map_err(|_| "A observação TCP exige IP literal ou CIDR, sem DNS.")?;
    let width = if address.is_ipv4() {32u8} else {128};
    let prefix = match prefix {Some(v)=>v.parse::<u8>().map_err(|_|"Prefixo de rede inválido.")?,None=>width};
    if prefix>width || width-prefix>4 {return Err("Selecione no máximo 16 endereços por avaliação TCP (/28 IPv4 ou /124 IPv6).".into());}
    let count=1u128 << (width-prefix);
    if count>MAX_HOSTS {return Err("Limite de endereços excedido.".into());}
    let raw=match address {IpAddr::V4(v)=>u32::from(v) as u128, IpAddr::V6(v)=>u128::from(v)};
    let base=raw & !(count-1);
    let mut out=Vec::new();
    for n in 0..count {
        let ip=if width==32 {IpAddr::V4(Ipv4Addr::from((base+n) as u32))}else{IpAddr::V6(Ipv6Addr::from(base+n))};
        if ip.is_unspecified() || ip.is_multicast() || ip==IpAddr::V4(Ipv4Addr::BROADCAST) || matches!(ip,IpAddr::V6(v) if v.to_ipv4_mapped().is_some()) {
            return Err("Destinos não especificados, multicast, broadcast global e IPv4 mapeado não são aceitos.".into());
        }
        out.push(ip);
    }
    Ok(out)
}

pub fn plan_addresses(target:&str, ports:&[u16])->Result<Vec<String>,String>{
    let distinct:BTreeSet<_>=ports.iter().copied().collect();
    if ports.is_empty()||ports.len()>16||distinct.len()!=ports.len()||ports.contains(&0){return Err("Escolha de 1 a 16 portas TCP distintas, entre 1 e 65535.".into());}
    let hosts=hosts(target)?;
    if hosts.len()*ports.len()>MAX_CONNECTIONS{return Err("Limite de 64 combinações de endereço e porta por plano TCP.".into());}
    let mut addresses=hosts.into_iter().flat_map(|ip|ports.iter().map(move |p|SocketAddr::new(ip,*p).to_string())).collect::<Vec<_>>();
    addresses.sort();
    Ok(addresses)
}

pub fn validate_addresses(target:&str, addresses:&[String])->Result<Vec<SocketAddr>,String>{
    if addresses.is_empty()||addresses.len()>MAX_CONNECTIONS{return Err("Plano TCP vazio ou acima da quota.".into());}
    let parsed=addresses.iter().map(|a|a.parse::<SocketAddr>().map_err(|_|"Destino TCP inválido.".to_string())).collect::<Result<Vec<_>,_>>()?;
    let ports=parsed.iter().map(SocketAddr::port).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    if plan_addresses(target,&ports)?!=addresses{return Err("O destino TCP não corresponde ao escopo e às portas exatas do plano.".into());}
    Ok(parsed)
}

/// V2 keeps the original human-selected ports, including ports whose entire
/// host product was denied. The positive cap is checked before subtraction.
pub fn plan_addresses_selected(target:&str,ports:&[u16],rules:&[crate::scope_targets::ConnectionExclusion])->Result<(Vec<String>,Vec<crate::scope_targets::ScopeOmission>),String>{
    let candidates=plan_addresses(target,ports)?;
    let mut allowed=vec![];let mut omitted=vec![];
    for value in candidates {
        let peer=value.parse::<SocketAddr>().map_err(|_|"Destino TCP inválido.")?;
        if let Some(rule)=crate::scope_targets::connection_excluded(rules,peer){omitted.push(crate::scope_targets::connection_omission(rule,peer));}
        else{allowed.push(value);}
    }
    if allowed.is_empty(){return Err("scope_all_connections_excluded: nenhuma conexão permitida pelos limites revisados.".into());}
    Ok((allowed,omitted))
}

pub fn validate_addresses_selected(target:&str,ports:&[u16],addresses:&[String],rules:&[crate::scope_targets::ConnectionExclusion])->Result<Vec<SocketAddr>,String>{
    let (expected,_)=plan_addresses_selected(target,ports,rules)?;
    if expected!=addresses{return Err("O conjunto TCP difere dos pares exatos permitidos pelo escopo revisado.".into());}
    addresses.iter().map(|value|value.parse::<SocketAddr>().map_err(|_|"Destino TCP inválido.".into())).collect()
}

pub fn observe(target:&str, addresses:&[String], cancel:&AtomicBool, deadline:Instant)->Result<Value,String>{
    observe_captured(target,addresses,cancel,deadline,&mut |_|Ok(()))
}
pub(crate) fn observe_captured(target:&str,addresses:&[String],cancel:&AtomicBool,deadline:Instant,capture:&mut dyn FnMut(&Value)->Result<(),String>)->Result<Value,String>{
    let destinations=validate_addresses(target,addresses)?;
    observe_destinations(target,addresses,destinations,cancel,deadline,&[],capture)
}

pub fn observe_selected(target:&str,ports:&[u16],addresses:&[String],rules:&[crate::scope_targets::ConnectionExclusion],cancel:&AtomicBool,deadline:Instant)->Result<Value,String>{
    observe_selected_captured(target,ports,addresses,rules,cancel,deadline,&mut |_|Ok(()))
}
pub(crate) fn observe_selected_captured(target:&str,ports:&[u16],addresses:&[String],rules:&[crate::scope_targets::ConnectionExclusion],cancel:&AtomicBool,deadline:Instant,capture:&mut dyn FnMut(&Value)->Result<(),String>)->Result<Value,String>{
    let destinations=validate_addresses_selected(target,ports,addresses,rules)?;
    observe_destinations(target,addresses,destinations,cancel,deadline,rules,capture)
}

#[allow(clippy::too_many_arguments)]
fn observe_destinations(target:&str,addresses:&[String],destinations:Vec<SocketAddr>,cancel:&AtomicBool,deadline:Instant,rules:&[crate::scope_targets::ConnectionExclusion],capture:&mut dyn FnMut(&Value)->Result<(),String>)->Result<Value,String>{
    let mut observations=Vec::new();
    let mut stop_reason=None;
    for endpoint in destinations {
        if crate::scope_targets::connection_excluded(rules,endpoint).is_some(){return Err("scope_connection_excluded: conexão recusada antes do socket.".into());}
        if cancel.load(Ordering::SeqCst){stop_reason=Some("canceled");break;}
        let remaining=deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero(){stop_reason=Some("timeout");break;}
        let start=Instant::now();
        let (outcome,code)=match TcpStream::connect_timeout(&endpoint,remaining.min(Duration::from_millis(1500))) {
            Ok(stream)=>{drop(stream);("connected",None)},
            Err(error)=>{
                let name=match error.kind(){std::io::ErrorKind::ConnectionRefused=>"refused",std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock=>"timeout",_=>"error"};
                (name,error.raw_os_error())
            }
        };
        let observation=json!({"endpoint":endpoint.to_string(),"outcome":outcome,"osErrorCode":code,"elapsedMs":start.elapsed().as_millis()});
        capture(&json!({"kind":"tcp_connection","observation":observation}))?;
        observations.push(observation);
    }
    if cancel.load(Ordering::SeqCst){stop_reason=Some("canceled");}
    Ok(json!({"adapter":ADAPTER,"target":target,"plannedEndpoints":addresses,"observations":observations,
        "capturePartial":stop_reason.is_some(),"stopReason":stop_reason,"protocol":"TCP","applicationBytesSent":0,
        "dnsUsed":false,"childProcesses":false,"parser":"openkoda.tcp-observation.v1",
        "limits":"Conexão TCP seguida de fechamento. Porta acessível não prova vulnerabilidade nem identifica serviço; timeout não prova porta fechada. Nenhum payload, banner, UDP, raw socket ou ferramenta externa."}))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn capture_failure_stops_before_the_next_socket(){
        let first=std::net::TcpListener::bind("127.0.0.1:0").unwrap();first.set_nonblocking(true).unwrap();
        let second=std::net::TcpListener::bind("127.0.0.1:0").unwrap();second.set_nonblocking(true).unwrap();
        let peers=plan_addresses("127.0.0.1",&[first.local_addr().unwrap().port(),second.local_addr().unwrap().port()]).unwrap();let mut captured=vec![];
        let result=observe_captured("127.0.0.1",&peers,&AtomicBool::new(false),Instant::now()+Duration::from_secs(5),&mut |record|{captured.push(record.clone());Err("capture persistence failure".into())});
        assert_eq!(result.unwrap_err(),"capture persistence failure");assert_eq!(captured.len(),1);assert_eq!(captured[0]["observation"]["endpoint"],peers[0]);
        let connected=usize::from(first.accept().is_ok())+usize::from(second.accept().is_ok());assert_eq!(connected,1);
    }
    fn deny(address:&str,ports:Option<Vec<u16>>)->crate::scope_targets::ConnectionExclusion{crate::scope_targets::ConnectionExclusion{address:address.into(),ports,reason:"Sentinel destination denied".into()}}
    #[test] fn selected_product_subtracts_pairs_without_weakening_positive_caps(){
        let rules=vec![deny("192.0.2.0/30",Some(vec![80])),deny("192.0.2.1",Some(vec![443]))];
        let (allowed,omitted)=plan_addresses_selected("192.0.2.0/30",&[80,443],&rules).unwrap();
        assert_eq!(allowed,vec!["192.0.2.0:443","192.0.2.2:443","192.0.2.3:443"]);assert_eq!(omitted.len(),5);
        validate_addresses_selected("192.0.2.0/30",&[80,443],&allowed,&rules).unwrap();
        let mut forged=allowed.clone();forged.push("192.0.2.1:443".into());forged.sort();assert!(validate_addresses_selected("192.0.2.0/30",&[80,443],&forged,&rules).is_err());
        assert!(validate_addresses_selected("192.0.2.0/30",&[80,443],&allowed[..2],&rules).is_err());
        assert!(plan_addresses_selected("192.0.2.0/28",&[1,2,3,4,5],&[deny("192.0.2.0/28",None)]).unwrap_err().contains("64"));
    }
    #[test] fn excluded_listener_receives_no_connection_even_with_forged_saved_peer(){
        let denied=std::net::TcpListener::bind("127.0.0.1:0").unwrap();denied.set_nonblocking(true).unwrap();let denied_peer=denied.local_addr().unwrap();
        let allowed=std::net::TcpListener::bind("127.0.0.1:0").unwrap();allowed.set_nonblocking(true).unwrap();let allowed_peer=allowed.local_addr().unwrap();
        let rules=vec![deny("127.0.0.1",Some(vec![denied_peer.port()]))];let ports=[denied_peer.port(),allowed_peer.port()];
        let (peers,omissions)=plan_addresses_selected("127.0.0.1",&ports,&rules).unwrap();assert_eq!(peers,vec![allowed_peer.to_string()]);assert_eq!(omissions.len(),1);
        let result=observe_selected("127.0.0.1",&ports,&peers,&rules,&AtomicBool::new(false),Instant::now()+Duration::from_secs(3)).unwrap();assert_eq!(result["observations"][0]["outcome"],"connected");assert!(allowed.accept().is_ok());assert_eq!(denied.accept().unwrap_err().kind(),std::io::ErrorKind::WouldBlock);
        assert!(observe_selected("127.0.0.1",&ports,&[denied_peer.to_string()],&rules,&AtomicBool::new(false),Instant::now()+Duration::from_secs(3)).is_err());
        assert!(plan_addresses_selected("127.0.0.1",&[denied_peer.port()],&rules).is_err());assert_eq!(denied.accept().unwrap_err().kind(),std::io::ErrorKind::WouldBlock);
    }
    #[test] fn destinations_are_exact_bounded_and_without_dns(){
        assert_eq!(plan_addresses("127.0.0.1",&[443,80]).unwrap(),vec!["127.0.0.1:443","127.0.0.1:80"]);
        assert_eq!(plan_addresses("2001:db8::/126",&[443]).unwrap().len(),4);
        for (target,ports) in [("example.test",vec![80]),("10.0.0.0/8",vec![80]),("::/0",vec![80]),("0.0.0.0",vec![80]),("ff02::1",vec![80]),("::ffff:127.0.0.1",vec![80]),("127.0.0.1",vec![0]),("127.0.0.1",vec![80,80])]{assert!(plan_addresses(target,&ports).is_err(),"{target}");}
        assert!(plan_addresses("192.0.2.0/28",&[1,2,3,4,5]).is_err());
    }
    #[test] fn substitutions_cannot_expand_the_selected_target(){
        assert!(validate_addresses("127.0.0.1",&["127.0.0.2:80".into()]).is_err());
        assert!(validate_addresses("::1",&["[::1%2]:80".into()]).is_err());
        assert!(validate_addresses("127.0.0.1",&["127.0.0.1:80".into(),"127.0.0.1:80".into()]).is_err());
    }
    #[test] fn local_connection_sends_no_application_payload(){
        use std::io::Read;
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let endpoint=listener.local_addr().unwrap();
        let thread=std::thread::spawn(move||{let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut buffer=[0u8;1];stream.read(&mut buffer).unwrap()});
        let result=observe("127.0.0.1",&[endpoint.to_string()],&AtomicBool::new(false),Instant::now()+Duration::from_secs(5)).unwrap();
        assert_eq!(result["observations"][0]["outcome"],"connected");assert_eq!(result["capturePartial"],false);assert_eq!(thread.join().unwrap(),0);
    }
    #[test] fn canceled_and_expired_work_preserves_explicit_empty_partial(){
        let addresses=vec!["127.0.0.1:1".into()];
        let canceled=observe("127.0.0.1",&addresses,&AtomicBool::new(true),Instant::now()+Duration::from_secs(1)).unwrap();
        assert_eq!(canceled["stopReason"],"canceled");assert_eq!(canceled["observations"],json!([]));
        let expired=observe("127.0.0.1",&addresses,&AtomicBool::new(false),Instant::now()).unwrap();
        assert_eq!(expired["stopReason"],"timeout");assert_eq!(expired["capturePartial"],true);
    }
}
