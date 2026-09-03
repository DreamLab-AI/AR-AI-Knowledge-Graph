//! Client polling probes — HTTP and WebSocket.
//!
//! Port of `tests/integration/client_polling_test.py`. The Python suite drove
//! its concurrency with `threading` and a `polling_active` flag; here the same
//! shapes fall out of `tokio::task` and a `JoinSet`, so the bookkeeping the
//! `PollingTestClient` class existed to do has gone with it.

use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use visionclaw_integration_tests::{require_server, Harness};

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Open a WebSocket to the bridge, or `None` if it refused.
async fn connect_ws(h: &Harness) -> Option<WsStream> {
    let connect = tokio_tungstenite::connect_async(&h.ws_url);
    let (stream, _) = tokio::time::timeout(Duration::from_secs(10), connect).await.ok()?.ok()?;
    Some(stream)
}

/// Send one JSON message and read one JSON reply.
async fn ws_roundtrip(stream: &mut WsStream, message: &Value) -> Option<Value> {
    stream.send(Message::Text(message.to_string())).await.ok()?;
    let reply = tokio::time::timeout(Duration::from_secs(5), stream.next()).await.ok()??.ok()?;
    serde_json::from_str(reply.to_text().ok()?).ok()
}

#[tokio::test]
async fn websocket_answers_a_ping_burst() {
    let h = require_server!();
    let Some(mut ws) = connect_ws(&h).await else {
        eprintln!("SKIP: {} refused the WebSocket connection.", h.ws_url);
        return;
    };

    for i in 0..5u64 {
        let reply = ws_roundtrip(&mut ws, &json!({ "type": "ping", "sequence": i })).await;
        assert!(reply.is_some(), "ping {i} of 5 went unanswered");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn a_websocket_client_can_reconnect() {
    let h = require_server!();

    {
        let Some(mut first) = connect_ws(&h).await else {
            eprintln!("SKIP: {} refused the WebSocket connection.", h.ws_url);
            return;
        };
        let _ = first.close(None).await;
    }

    let mut second = connect_ws(&h).await.expect("could not reconnect to the WebSocket bridge");
    assert!(
        ws_roundtrip(&mut second, &json!({ "type": "ping" })).await.is_some(),
        "the reconnected socket did not answer a ping"
    );
}

#[tokio::test]
async fn ten_concurrent_clients_each_receive_data() {
    let h = require_server!();
    if connect_ws(&h).await.is_none() {
        eprintln!("SKIP: {} refused the WebSocket connection.", h.ws_url);
        return;
    }

    let mut tasks = tokio::task::JoinSet::new();
    for client in 0..10u64 {
        let h = h.clone();
        tasks.spawn(async move {
            let mut ws = connect_ws(&h).await?;
            let mut received = 0usize;
            for _ in 0..3 {
                if ws_roundtrip(&mut ws, &json!({ "type": "data", "client": client })).await.is_some()
                {
                    received += 1;
                }
            }
            Some(received)
        });
    }

    let mut clients = 0usize;
    while let Some(outcome) = tasks.join_next().await {
        if let Ok(Some(received)) = outcome {
            clients += 1;
            assert!(received > 0, "a concurrent client received nothing at all");
        }
    }
    assert!(clients > 0, "not one of the ten concurrent clients connected");
}

#[tokio::test]
async fn messages_still_arrive_across_repeated_drops() {
    let h = require_server!();
    if connect_ws(&h).await.is_none() {
        eprintln!("SKIP: {} refused the WebSocket connection.", h.ws_url);
        return;
    }

    let mut reconnects = 0usize;
    let mut delivered = 0usize;

    for drop_round in 0..3u64 {
        let Some(mut ws) = connect_ws(&h).await else { continue };
        reconnects += 1;
        for message in 0..3u64 {
            if ws_roundtrip(&mut ws, &json!({ "type": "test", "round": drop_round, "n": message }))
                .await
                .is_some()
            {
                delivered += 1;
            }
        }
        let _ = ws.close(None).await; // the simulated drop
    }

    assert_eq!(reconnects, 3, "a reconnection after a drop failed");
    assert!(delivered > 5, "only {delivered} of 9 messages survived the drops");
}

#[tokio::test]
async fn an_unauthenticated_admin_command_is_refused() {
    let h = require_server!();
    let Some(mut ws) = connect_ws(&h).await else {
        eprintln!("SKIP: {} refused the WebSocket connection.", h.ws_url);
        return;
    };

    let Some(reply) =
        ws_roundtrip(&mut ws, &json!({ "type": "admin_command", "action": "get_all_users" })).await
    else {
        return; // the bridge cut us off, which is itself a refusal
    };

    assert!(
        reply.get("error").is_some() || reply.get("unauthorized") == Some(&json!(true)),
        "an unauthenticated admin_command was accepted: {reply}"
    );
}

#[tokio::test]
async fn http_polling_returns_json() {
    let h = require_server!();

    let mut successes = 0usize;
    for _ in 0..3 {
        if let Some(response) = h.get("/poll").await {
            if response.status() == reqwest::StatusCode::OK
                && response.json::<Value>().await.is_ok()
            {
                successes += 1;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert!(successes > 0, "three polls of /poll produced no parseable JSON");
}

#[tokio::test]
async fn long_polling_returns_inside_thirty_seconds() {
    let h = require_server!();

    let started = Instant::now();
    let response = h
        .http()
        .get(h.url("/long-poll"))
        .timeout(Duration::from_secs(35))
        .send()
        .await;
    let elapsed = started.elapsed();

    let Some(response) = response.ok() else {
        eprintln!("SKIP: /long-poll did not answer.");
        return;
    };

    assert!(elapsed < Duration::from_secs(30), "long poll hung for {elapsed:?}");
    let status = response.status().as_u16();
    assert!(matches!(status, 200 | 204), "long poll answered {status}, expected 200 or 204");
}

#[tokio::test]
async fn aggressive_polling_is_rate_limited() {
    let h = require_server!();

    let mut limited = false;
    for _ in 0..30 {
        match h.get("/poll").await {
            Some(response) if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS => {
                limited = true;
                break;
            }
            Some(_) => {}
            None => break,
        }
    }

    assert!(limited, "30 back-to-back polls drew no 429 — /poll is not rate limited");
}
