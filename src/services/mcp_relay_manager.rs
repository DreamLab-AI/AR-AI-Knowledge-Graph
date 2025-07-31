use std::process::Command;
use log::{info, error, warn};

/// Manages the MCP WebSocket relay in the powerdev container
pub struct McpRelayManager;

impl McpRelayManager {
    /// Check if the MCP relay is running in the powerdev container
    pub fn check_relay_status() -> bool {
        info!("Checking MCP relay status in powerdev container...");
        
        let output = Command::new("docker")
            .args(&["exec", "powerdev", "pgrep", "-f", "mcp-ws-relay"])
            .output();
            
        match output {
            Ok(result) => {
                let is_running = result.status.success();
                if is_running {
                    info!("MCP relay is running in powerdev container");
                } else {
                    warn!("MCP relay is not running in powerdev container");
                }
                is_running
            }
            Err(e) => {
                error!("Failed to check MCP relay status: {}", e);
                false
            }
        }
    }
    
    /// Start the MCP relay in the powerdev container if not already running
    pub fn ensure_relay_running() -> Result<(), String> {
        if Self::check_relay_status() {
            info!("MCP relay already running, no action needed");
            return Ok(());
        }
        
        info!("Starting MCP relay in powerdev container...");
        
        // Start the relay in the background
        let output = Command::new("docker")
            .args(&[
                "exec", "-d", "powerdev",
                "bash", "-c",
                "cd /tmp && node mcp-ws-relay.js > /tmp/mcp-ws-relay.log 2>&1"
            ])
            .output();
            
        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("Successfully started MCP relay in powerdev container");
                    
                    // Give it a moment to start
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    
                    // Verify it's running
                    if Self::check_relay_status() {
                        Ok(())
                    } else {
                        Err("MCP relay started but not running".to_string())
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    Err(format!("Failed to start MCP relay: {}", stderr))
                }
            }
            Err(e) => {
                Err(format!("Failed to execute docker command: {}", e))
            }
        }
    }
    
    /// Get the logs from the MCP relay
    pub fn get_relay_logs(lines: usize) -> Result<String, String> {
        let output = Command::new("docker")
            .args(&[
                "exec", "powerdev",
                "tail", "-n", &lines.to_string(),
                "/tmp/mcp-ws-relay.log"
            ])
            .output();
            
        match output {
            Ok(result) => {
                if result.status.success() {
                    Ok(String::from_utf8_lossy(&result.stdout).to_string())
                } else {
                    Err(format!("Failed to get logs: {}", String::from_utf8_lossy(&result.stderr)))
                }
            }
            Err(e) => {
                Err(format!("Failed to execute docker command: {}", e))
            }
        }
    }
    
    /// Check if powerdev container is running
    pub fn check_powerdev_container() -> bool {
        let output = Command::new("docker")
            .args(&["ps", "-q", "-f", "name=powerdev"])
            .output();
            
        match output {
            Ok(result) => {
                !result.stdout.is_empty()
            }
            Err(_) => false
        }
    }
}

/// Ensure MCP relay is available before starting ClaudeFlowActor
pub async fn ensure_mcp_ready() -> Result<(), String> {
    // First check if powerdev container exists
    if !McpRelayManager::check_powerdev_container() {
        return Err("powerdev container is not running".to_string());
    }
    
    // Try to ensure relay is running
    McpRelayManager::ensure_relay_running()?;
    
    // Additional wait for relay to be fully ready
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    Ok(())
}