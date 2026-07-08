//! D2 (PRD-023 WP-3, CANARY-VC-D2-STEER) interrupt id-namespace resolution —
//! fake-Management-API integration test.
//!
//! `AgentDetailPanel` sends `selectedAgent.id`, a claude-flow swarm `agent_id`.
//! That is a DISJOINT minted namespace from the Management-API `task_id` the
//! stop call needs — so the previous `StopTask { task_id: <swarm agent_id> }`
//! path issued `DELETE /v1/tasks/<swarm agent_id>`, which the Management API
//! 404s every time, and the canary (which observes only on success) could
//! structurally never fire.
//!
//! The fix routes the interrupt through `InterruptAgentTask`, which resolves the
//! incoming id to a concrete `task_id` server-side (a direct task_id hit, else
//! the live task whose `agent` field carries the id) before the stop. This test
//! drives the REAL orchestrator handler against a REAL local HTTP server that
//! mimics the Management-API `/v1/tasks` contract (the same hand-rolled-socket
//! idiom `tests/voice_intent_roundtrip.rs` uses to fake the D7 producer): no
//! mock framework, real request line, real headers, real body — so the resolve
//! GET and the stop DELETE are asserted exactly as they cross the wire.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use actix::Actor;
use visionclaw_server::actors::{InterruptAgentTask, TaskOrchestratorActor};
use visionclaw_server::services::management_api_client::ManagementApiClient;

#[derive(Debug, Clone)]
struct CapturedReq {
    method: String,
    path: String,
}

/// Bind an ephemeral port and serve exactly `request_count` requests from a
/// background thread, mimicking the agentbox Management API:
///   * `GET /v1/tasks`        → a one-task active list whose `agent` field is
///                              `agent_field` and whose `taskId` is `task_id`;
///   * `DELETE /v1/tasks/{id}` → 200 (task stopped).
/// Returns the bound port and the capture buffer of served requests.
fn spawn_fake_mgmt_api(
    request_count: usize,
    agent_field: &'static str,
    task_id: &'static str,
) -> (u16, Arc<Mutex<Vec<CapturedReq>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("local addr").port();
    let captured = Arc::new(Mutex::new(Vec::<CapturedReq>::new()));
    let cap = captured.clone();

    thread::spawn(move || {
        for _ in 0..request_count {
            let (stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(_) => break,
            };
            let mut write_stream = stream.try_clone().expect("clone stream");
            let mut reader = BufReader::new(stream);

            // Request line: "<METHOD> <PATH> HTTP/1.1".
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                continue;
            }
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();

            // Drain headers; capture content-length (GET/DELETE carry none here).
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    break; // end of headers
                }
                if let Some(v) = trimmed
                    .to_ascii_lowercase()
                    .strip_prefix("content-length:")
                {
                    content_length = v.trim().parse().unwrap_or(0);
                }
            }
            if content_length > 0 {
                let mut body = vec![0u8; content_length];
                let _ = reader.read_exact(&mut body);
            }

            cap.lock().unwrap().push(CapturedReq {
                method: method.clone(),
                path: path.clone(),
            });

            let body = if method == "GET" && path == "/v1/tasks" {
                // camelCase to match TaskListResponse/TaskInfo serde renames.
                format!(
                    r#"{{"activeTasks":[{{"taskId":"{task_id}","agent":"{agent_field}","task":"demo","provider":"gemini","status":"running","startTime":0,"duration":0}}],"count":1}}"#
                )
            } else {
                // DELETE /v1/tasks/{id} (or anything else) → bare success 200.
                r#"{"success":true}"#.to_string()
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = write_stream.write_all(response.as_bytes());
            let _ = write_stream.flush();
        }
    });

    (port, captured)
}

#[actix_rt::test]
async fn swarm_agent_id_resolves_to_task_id_and_issues_stop() {
    // The panel sends a claude-flow swarm agent_id (NOT a task id). The
    // Management API's task list joins them: the task whose `agent` field is
    // this swarm agent_id has the disjoint task_id `task-abc123`.
    const SWARM_AGENT_ID: &str = "agent-swarm-7f3a"; // claude-flow shape, not a task_id
    const TASK_ID: &str = "task-abc123"; // disjoint Management-API id

    // Two requests expected: the resolve GET, then the stop DELETE.
    let (port, captured) = spawn_fake_mgmt_api(2, SWARM_AGENT_ID, TASK_ID);

    let client = ManagementApiClient::new("127.0.0.1".to_string(), port, "test-key".to_string());
    let addr = TaskOrchestratorActor::new(client).start();

    // Interrupt by the SWARM AGENT ID — exactly what AgentDetailPanel sends.
    let result = addr
        .send(InterruptAgentTask {
            id: SWARM_AGENT_ID.to_string(),
        })
        .await
        .expect("actor mailbox delivered");

    // 1) It resolved the swarm agent_id to the disjoint task_id and reported it.
    assert_eq!(
        result,
        Ok(TASK_ID.to_string()),
        "a swarm agent_id must resolve to its Management-API task_id"
    );

    // 2) Wire proof: a GET /v1/tasks (resolve), then a DELETE on the RESOLVED
    //    task_id (stop) — and NEVER a DELETE on the raw swarm agent_id (the 404).
    let reqs = captured.lock().unwrap().clone();
    assert!(
        reqs.iter().any(|r| r.method == "GET" && r.path == "/v1/tasks"),
        "resolution must consult GET /v1/tasks; saw {reqs:?}"
    );
    assert!(
        reqs.iter()
            .any(|r| r.method == "DELETE" && r.path == format!("/v1/tasks/{TASK_ID}")),
        "stop must DELETE the RESOLVED task_id /v1/tasks/{TASK_ID}; saw {reqs:?}"
    );
    assert!(
        !reqs
            .iter()
            .any(|r| r.method == "DELETE" && r.path.contains(SWARM_AGENT_ID)),
        "must NOT DELETE the raw swarm agent_id (that is the 404 bug); saw {reqs:?}"
    );
}

#[actix_rt::test]
async fn direct_task_id_still_resolves_and_stops() {
    // Backward compatibility: an id that IS already a Management-API task_id
    // (e.g. an agent spawned via spawn_agent_hybrid, whose swarmId == task_id)
    // still resolves — here via the live-list task_id hit — and stops.
    const TASK_ID: &str = "task-direct-9";

    let (port, captured) = spawn_fake_mgmt_api(2, "agent-unrelated", TASK_ID);

    let client = ManagementApiClient::new("127.0.0.1".to_string(), port, "test-key".to_string());
    let addr = TaskOrchestratorActor::new(client).start();

    let result = addr
        .send(InterruptAgentTask {
            id: TASK_ID.to_string(),
        })
        .await
        .expect("actor mailbox delivered");

    assert_eq!(result, Ok(TASK_ID.to_string()));

    let reqs = captured.lock().unwrap().clone();
    assert!(
        reqs.iter()
            .any(|r| r.method == "DELETE" && r.path == format!("/v1/tasks/{TASK_ID}")),
        "a direct task_id must DELETE /v1/tasks/{TASK_ID}; saw {reqs:?}"
    );
}

#[actix_rt::test]
async fn unresolvable_id_errors_without_stopping() {
    // Honesty: an id that matches neither a task_id nor any task's agent field
    // errors — it must NOT blind-stop an unrelated task. Only the resolve GET is
    // issued; no DELETE follows.
    let (port, captured) = spawn_fake_mgmt_api(1, "agent-other", "task-other");

    let client = ManagementApiClient::new("127.0.0.1".to_string(), port, "test-key".to_string());
    let addr = TaskOrchestratorActor::new(client).start();

    let result = addr
        .send(InterruptAgentTask {
            id: "agent-nonexistent".to_string(),
        })
        .await
        .expect("actor mailbox delivered");

    assert!(
        result.is_err(),
        "an unresolvable id must error, not silently succeed; got {result:?}"
    );

    let reqs = captured.lock().unwrap().clone();
    assert!(
        !reqs.iter().any(|r| r.method == "DELETE"),
        "no DELETE may be issued for an unresolvable id; saw {reqs:?}"
    );
}
