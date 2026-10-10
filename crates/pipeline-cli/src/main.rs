use std::env;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use pipeline_engine::ProjectEngine;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("serve") if args.len() == 4 => {
            let address: SocketAddr = args[3].parse().map_err(|_| "invalid socket address")?;
            pipeline_api::serve(&args[2], address)
        }
        Some("call") if args.len() == 3 => {
            let address: SocketAddr = args[2].parse().map_err(|_| "invalid socket address")?;
            if !address.ip().is_loopback() { return Err("agent bridge must be loopback".into()); }
            let token=env::var("PIPELINE_AGENT_TOKEN").map_err(|_|"PIPELINE_AGENT_TOKEN is required")?;
            if token.len()!=64 || !token.chars().all(|c|c.is_ascii_hexdigit()) {return Err("invalid agent token".into());}
            let mut body=String::new();
            io::stdin().take(1_048_577).read_to_string(&mut body).map_err(|e|e.to_string())?;
            if body.len()>1_048_576 {return Err("request too large".into());}
            let value: serde_json::Value=serde_json::from_str(&body).map_err(|e|e.to_string())?;
            if value.get("token").is_some() {return Err("token must come from PIPELINE_AGENT_TOKEN".into());}
            let content=send(address,&token,&body)?;
            println!("{content}");
            Ok(())
        }
        Some("policy-list") if args.len() == 4 => {
            let engine = ProjectEngine::open(&args[2]).map_err(|e| e.to_string())?;
            for request in engine.load_review(&args[3]).map_err(|e| e.to_string())?.policy_requests {
                println!("id={} revision={} class={} digest={} target={:?} effect={:?}",
                    request.id, request.revision, request.action_class,
                    request.command_digest, request.target, request.effect_summary);
            }
            Ok(())
        }
        Some("task-retry") if args.len() == 7 => {
            let mut engine = ProjectEngine::open(&args[2]).map_err(|e| e.to_string())?;
            let revision = args[5].parse::<i64>().map_err(|_| "invalid task revision")?;
            let reason = args[6].trim();
            if reason.is_empty() { return Err("retry reason is required".into()); }
            engine.request_task_changes(&args[3], &args[4], revision, reason)
                .map_err(|e| e.to_string())?;
            println!("task={} retry requested with reason", args[4]);
            Ok(())
        }
        Some("policy-resolve") if args.len() == 8 => {
            let mut engine = ProjectEngine::open(&args[2]).map_err(|e| e.to_string())?;
            let request = engine.load_review(&args[3]).map_err(|e| e.to_string())?
                .policy_requests.into_iter().find(|request| request.id == args[4])
                .ok_or("pending policy request not found in project")?;
            let revision = args[5].parse::<i64>().map_err(|_| "invalid revision")?;
            if request.revision != revision || request.command_digest != args[6] {
                return Err("policy request revision or command digest changed".into());
            }
            let approve = match args[7].as_str() {
                "approve" => true,
                "deny" => false,
                _ => return Err("policy decision must be approve or deny".into()),
            };
            let expiry = if approve {
                Some(SystemTime::now().duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?.as_secs() as i64 + 600)
            } else { None };
            let resolved = engine.resolve_action_request(&request.id, revision, approve, expiry)
                .map_err(|e| e.to_string())?;
            println!("id={} status={} revision={} digest={} target={:?}",
                resolved.id, resolved.status, resolved.revision,
                resolved.command_digest, resolved.target);
            Ok(())
        }
        _ => Err("usage: pipeline-cli serve <database> <127.0.0.1:port> | call <127.0.0.1:port> (JSON on stdin; token in PIPELINE_AGENT_TOKEN) | policy-list <database> <project_id> | policy-resolve <database> <project_id> <request_id> <revision> <command_digest> <approve|deny> | task-retry <database> <project_id> <task_id> <task_revision> <reason>".into()),
    }
}

fn send(address: SocketAddr, token: &str, body: &str) -> Result<String, String> {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    write!(stream,"POST /v1/command HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).map_err(|e|e.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    let (headers, content) = response
        .split_once("\r\n\r\n")
        .ok_or("malformed HTTP response")?;
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(content.to_owned());
    }
    Ok(content.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn cli_protocol_passes_token_in_header_and_json_in_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut buffer = [0_u8; 2048];
            let mut request = String::new();
            while !request.contains("\r\n\r\n{\"operation\":\"project.get\"}") {
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0, "client closed before sending body");
                request.push_str(&String::from_utf8_lossy(&buffer[..count]));
            }
            assert!(request.contains("Authorization: Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
            assert!(request.contains("\r\n\r\n{\"operation\":\"project.get\"}"));
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}").unwrap();
        });
        let response = send(address, &"a".repeat(64), r#"{"operation":"project.get"}"#).unwrap();
        assert_eq!(response, r#"{"ok":true}"#);
        server.join().unwrap();
    }
}
