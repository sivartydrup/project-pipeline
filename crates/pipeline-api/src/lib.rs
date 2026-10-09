//! Local, capability-scoped agent bridge. All writes pass through ProjectEngine.

use pipeline_engine::{AgentCommand, AgentReply, EngineError, ProjectEngine, StoreError};
use serde_json::json;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use tiny_http::{Header, Method, Response, Server, StatusCode};

pub fn call(database: impl AsRef<Path>, command: &AgentCommand) -> Result<AgentReply, EngineError> {
    let mut engine = ProjectEngine::open(database)?;
    engine.agent_command(command)
}

pub fn serve(database: impl AsRef<Path>, address: SocketAddr) -> Result<(), String> {
    serve_requests(database, address, usize::MAX)
}

fn serve_requests(
    database: impl AsRef<Path>,
    address: SocketAddr,
    max_requests: usize,
) -> Result<(), String> {
    if !address.ip().is_loopback() {
        return Err("agent bridge must bind to a loopback address".into());
    }
    let server = Server::http(address).map_err(|e| e.to_string())?;
    for mut request in server.incoming_requests().take(max_requests) {
        let peer_loopback = request.remote_addr().is_some_and(|a| a.ip().is_loopback());
        let (status, body) = if !peer_loopback {
            (403, json!({"error":"non-loopback peer"}))
        } else if request.method() != &Method::Post || request.url() != "/v1/command" {
            (404, json!({"error":"use POST /v1/command"}))
        } else {
            let auth = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .map(|h| h.value.as_str().to_owned());
            let mut input = String::new();
            let read = request
                .as_reader()
                .take(1_048_577)
                .read_to_string(&mut input);
            if read.is_err() || input.len() > 1_048_576 {
                (413, json!({"error":"request too large or invalid UTF-8"}))
            } else {
                match (
                    auth.as_deref().and_then(|s| s.strip_prefix("Bearer ")),
                    serde_json::from_str::<AgentCommand>(&input),
                ) {
                    (Some(token), Ok(mut command)) if command.token.is_empty() => {
                        command.token = token.to_owned();
                        match call(database.as_ref(), &command) {
                            Ok(reply) => (200, json!(reply)),
                            Err(error) => {
                                let status = match &error {
                                    EngineError::Store(StoreError::RevisionConflict { .. }) => 409,
                                    EngineError::Store(StoreError::AgentDenied(reason))
                                        if reason == "invalid token" =>
                                    {
                                        401
                                    }
                                    EngineError::Store(StoreError::AgentDenied(_)) => 403,
                                    _ => 400,
                                };
                                (status, json!({"error":error.to_string()}))
                            }
                        }
                    }
                    _ => (
                        400,
                        json!({"error":"bearer token and valid command body required"}),
                    ),
                }
            }
        };
        let response = Response::from_string(body.to_string())
            .with_status_code(StatusCode(status))
            .with_header(
                Header::from_bytes("Content-Type", "application/json")
                    .map_err(|_| "header error")?,
            );
        request.respond(response).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    fn post(address: SocketAddr, token: &str, body: &str) -> String {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(stream,"POST /v1/command HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn rejects_non_loopback_bind() {
        let address: SocketAddr = "0.0.0.0:0".parse().unwrap();
        assert!(serve("unused.sqlite", address).is_err());
    }

    #[test]
    fn http_route_requires_bearer_token_and_valid_body() {
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let worker = thread::spawn(move || serve_requests("unused.sqlite", address, 1));
        let mut stream = loop {
            match TcpStream::connect(address) {
                Ok(stream) => break stream,
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        };
        stream
            .write_all(
                b"POST /v1/command HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 2\r\n\r\n{}",
            )
            .unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.contains("400 Bad Request"));
        assert!(response.contains("bearer token"));
        assert!(worker.join().unwrap().is_ok());
    }

    #[test]
    fn loopback_route_enforces_grant_and_revision_contract() {
        let folder = tempfile::tempdir().unwrap();
        let checkout = folder.path().join("checkout");
        std::fs::create_dir(&checkout).unwrap();
        let db = folder.path().join("pipeline.sqlite");
        {
            let mut store = pipeline_store::Store::open(&db).unwrap();
            store
                .create_project(
                    "p",
                    "Project",
                    checkout.to_str().unwrap(),
                    "owner",
                    "create",
                )
                .unwrap();
        }
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection
            .execute(
                "UPDATE projects SET active_scope_revision=1 WHERE id='p'",
                [],
            )
            .unwrap();
        connection.execute("INSERT INTO tasks(id,project_id,title,outcome,status,scope_revision,logical_id) VALUES ('t','p','Task','Done','ready',1,'T')",[]).unwrap();
        drop(connection);
        let grant = pipeline_store::Store::open(&db)
            .unwrap()
            .issue_agent_grant(
                "p",
                "T",
                &checkout,
                &["project.get".into(), "task.block".into()],
                60,
            )
            .unwrap();
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let worker = thread::spawn(move || serve_requests(db, address, 3));
        let mut connected = false;
        for _ in 0..100 {
            if TcpStream::connect(address).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(connected);
        // The readiness probe consumes one request only after it sends HTTP. A bare connect is ignored by tiny_http.
        let read =
            json!({"project_id":"p","run_id":grant.run_id,"operation":"project.get"}).to_string();
        let response = post(address, &grant.token, &read);
        assert!(response.contains("200 OK"), "{response}");
        assert!(response.contains("Project"));
        let block=json!({"project_id":"p","run_id":grant.run_id,"operation":"task.block","idempotency_key":"block1","expected_revision":1,"payload":{"reason":"waiting"}}).to_string();
        let response = post(address, &grant.token, &block);
        assert!(response.contains("200 OK"), "{response}");
        let stale=json!({"project_id":"p","run_id":grant.run_id,"operation":"task.block","idempotency_key":"block2","expected_revision":1,"payload":{"reason":"again"}}).to_string();
        let response = post(address, &grant.token, &stale);
        assert!(response.contains("409 Conflict"), "{response}");
        assert!(worker.join().unwrap().is_ok());
    }
}
