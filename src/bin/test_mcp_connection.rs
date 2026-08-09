//! Test MCP TCP Connection
//!
//! This binary tests the MCP TCP client implementation to verify it can connect
//! to and query real MCP servers.

use log::{error, info};
use std::collections::HashMap;
use visionclaw_server::services::agent_visualization_protocol::McpServerType;
use visionclaw_server::utils::mcp_tcp_client::{create_mcp_client, test_mcp_connectivity};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Testing MCP TCP connectivity");

    let mut servers = HashMap::new();
    servers.insert(
        "localhost-9500".to_string(),
        ("localhost".to_string(), 9500),
    );
    servers.insert(
        "multi-agent-container-9500".to_string(),
        ("multi-agent-container".to_string(), 9500),
    );

    let connectivity_results = test_mcp_connectivity(&servers).await;

    for (server_id, is_connected) in &connectivity_results {
        if *is_connected {
            info!("Server {} is reachable", server_id);
        } else {
            error!("✗ Server {} is not reachable", server_id);
        }
    }

    for (server_id, (host, port)) in &servers {
        if *connectivity_results.get(server_id).unwrap_or(&false) {
            info!("Testing detailed MCP functionality with {}:{}", host, port);

            let client = create_mcp_client(&McpServerType::ClaudeFlow, host, *port);

            match client.test_connection().await {
                Ok(true) => info!("Basic connection test passed"),
                Ok(false) => error!("✗ Basic connection test failed"),
                Err(e) => error!("✗ Connection test error: {}", e),
            }

            match client.initialize_session().await {
                Ok(()) => info!("MCP session initialization passed"),
                Err(e) => error!("✗ MCP session initialization failed: {}", e),
            }

            match client.query_server_info().await {
                Ok(server_info) => {
                    info!("Server info query passed");
                    info!("  Server ID: {}", server_info.server_id);
                    info!("  Server Type: {:?}", server_info.server_type);
                    info!("  Supported Tools: {:?}", server_info.supported_tools);
                    info!("  Agent Count: {}", server_info.agent_count);
                }
                Err(e) => error!("✗ Server info query failed: {}", e),
            }

            match client.query_agent_list().await {
                Ok(agents) => {
                    info!("Agent list query passed");
                    info!("  Retrieved {} agents", agents.len());
                    for (i, agent) in agents.iter().take(3).enumerate() {
                        info!("  Agent {}: {} ({})", i + 1, agent.name, agent.agent_type);
                    }
                }
                Err(e) => error!("✗ Agent list query failed: {}", e),
            }

            match client.query_swarm_status().await {
                Ok(topology) => {
                    info!("Swarm status query passed");
                    info!("  Topology: {}", topology.topology_type);
                    info!("  Total Agents: {}", topology.total_agents);
                    info!("  Efficiency: {:.2}", topology.efficiency_score);
                }
                Err(e) => error!("✗ Swarm status query failed: {}", e),
            }

            break;
        }
    }

    if connectivity_results.values().all(|&connected| !connected) {
        error!("No MCP servers are reachable. Make sure an MCP server is running on port 9500.");
        return Err("No MCP servers available".into());
    }

    info!("MCP TCP connection testing completed");
    Ok(())
}
