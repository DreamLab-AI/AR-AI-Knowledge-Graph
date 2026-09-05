//! TCP connection persistence probes.
//!
//! Port of `tests/integration/tcp_persistence_test.py`. The Python suite had
//! both a synchronous-socket and an `asyncio` variant of the same persistence
//! check (`test_connection_persistence` / `test_async_persistence`); under
//! tokio every probe is already async, so the pair collapses into one.

use serde_json::json;
use visionclaw_integration_tests::require_server;

#[tokio::test]
async fn basic_connection_completes_a_handshake() {
    let h = require_server!();
    let Some(mut tcp) = h.tcp().await else {
        eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
        return;
    };

    let response = tcp
        .request(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "1.0",
                "capabilities": {},
                "clientInfo": { "name": "visionclaw-integration-tests", "version": "1.0" },
            },
        }))
        .await
        .expect("initialize returned no response");

    assert!(
        response.get("result").is_some(),
        "initialize response carried no `result`: {response}"
    );
}

#[tokio::test]
async fn connection_survives_repeated_requests() {
    let h = require_server!();
    let Some(mut tcp) = h.tcp().await else {
        eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
        return;
    };

    for id in 1..=10u64 {
        let response = tcp.ping(json!(id)).await.unwrap_or_else(|| {
            panic!("ping {id} of 10 returned no response — connection dropped early")
        });
        assert_eq!(
            response["id"],
            json!(id),
            "response id did not echo the request"
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    assert!(tcp.connected, "connection was torn down during the run");
}

#[tokio::test]
#[ignore = "holds the connection idle for 30s; run with --ignored"]
async fn connection_survives_an_idle_period() {
    let h = require_server!();
    let Some(mut tcp) = h.tcp().await else {
        eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
        return;
    };

    assert!(tcp.ping(json!(1)).await.is_some(), "pre-idle ping failed");
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    assert!(
        tcp.ping(json!(2)).await.is_some(),
        "post-idle ping failed — the server closed an idle connection"
    );
}

#[tokio::test]
async fn a_client_can_reconnect_after_disconnecting() {
    let h = require_server!();

    {
        let Some(mut first) = h.tcp().await else {
            eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
            return;
        };
        assert!(
            first.ping(json!(1)).await.is_some(),
            "ping on the first connection failed"
        );
    } // dropped: the socket closes here

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let mut second = h
        .tcp()
        .await
        .expect("could not reconnect after a clean disconnect");
    assert!(
        second.ping(json!(2)).await.is_some(),
        "ping on the reconnected socket failed"
    );
}

#[tokio::test]
async fn five_clients_can_hold_connections_concurrently() {
    let h = require_server!();
    if h.tcp().await.is_none() {
        eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
        return;
    }

    let mut clients = Vec::new();
    for i in 0..5u64 {
        clients.push(
            h.tcp()
                .await
                .unwrap_or_else(|| panic!("client {i} could not connect")),
        );
    }

    for (i, client) in clients.iter_mut().enumerate() {
        for j in 0..3u64 {
            let id = json!(format!("{i}-{j}"));
            let response = client
                .request(&json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "ping",
                    "params": { "client": i },
                }))
                .await
                .unwrap_or_else(|| panic!("client {i} request {j} returned no response"));
            assert_eq!(
                response["id"], id,
                "client {i} got a mismatched response id"
            );
        }
    }
}

#[tokio::test]
async fn a_one_megabyte_payload_does_not_break_the_connection() {
    let h = require_server!();
    let Some(mut tcp) = h.tcp().await else {
        eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
        return;
    };

    let large = "x".repeat(1024 * 1024);
    let response = tcp
        .request(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "process",
            "params": { "data": large },
        }))
        .await;

    assert!(
        response.is_some(),
        "the server gave no answer to a 1 MiB payload"
    );
    assert!(tcp.connected, "the connection died on a 1 MiB payload");

    assert!(
        tcp.ping(json!(2)).await.is_some(),
        "the follow-up ping failed — the connection did not recover"
    );
}

#[tokio::test]
async fn a_slow_operation_answers_within_the_request_timeout() {
    let h = require_server!();
    let Some(mut tcp) = h.tcp().await else {
        eprintln!("SKIP: {} refused the connection.", h.tcp_addr);
        return;
    };

    assert!(
        tcp.ping(json!(1)).await.is_some(),
        "the warm-up ping failed"
    );

    let response = tcp
        .request(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "slow_operation",
            "params": { "delay": 3 },
        }))
        .await;

    assert!(
        response.is_some(),
        "a 3s operation did not answer inside the {:?} request timeout",
        visionclaw_integration_tests::REQUEST_TIMEOUT
    );
}
