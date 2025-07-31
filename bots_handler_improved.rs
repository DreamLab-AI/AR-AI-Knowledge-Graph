// Improved initialize_swarm handler with better error handling
pub async fn initialize_swarm(
    state: web::Data<AppState>,
    request: web::Json<InitializeSwarmRequest>,
) -> impl Responder {
    info!("=== INITIALIZE SWARM ENDPOINT CALLED ===");
    info!("Received swarm initialization request: {:?}", request);
    
    // Validate input parameters
    if request.max_agents == 0 || request.max_agents > 100 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Invalid max_agents value. Must be between 1 and 100.",
            "code": "INVALID_PARAMETERS"
        }));
    }

    if request.agent_types.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "At least one agent type must be specified.",
            "code": "INVALID_PARAMETERS"
        }));
    }

    // Get the Claude Flow actor
    let claude_flow_addr = match &state.claude_flow_addr {
        Some(addr) => addr,
        None => {
            error!("Claude Flow actor not available in application state");
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "success": false,
                "error": "Claude Flow service is not initialized. Please ensure the PowerDev container is running.",
                "code": "SERVICE_UNAVAILABLE",
                "details": {
                    "service": "claude-flow",
                    "suggestion": "Check that the MCP WebSocket relay is running on powerdev:3000"
                }
            }));
        }
    };

    // Send initialization message to ClaudeFlowActor with timeout
    let init_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        claude_flow_addr.send(InitializeSwarm {
            topology: request.topology.clone(),
            max_agents: request.max_agents,
            strategy: request.strategy.clone(),
            enable_neural: request.enable_neural,
            agent_types: request.agent_types.clone(),
            custom_prompt: request.custom_prompt.clone(),
        })
    ).await;

    match init_result {
        Ok(Ok(Ok(_))) => {
            info!("Swarm initialization completed successfully");
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Swarm initialized successfully",
                "data": {
                    "topology": request.topology,
                    "max_agents": request.max_agents,
                    "agent_types": request.agent_types,
                    "neural_enabled": request.enable_neural
                }
            }))
        }
        Ok(Ok(Err(e))) => {
            error!("Swarm initialization failed: {}", e);
            
            // Parse error to provide specific feedback
            let (error_code, suggestion) = if e.contains("Connection refused") {
                ("CONNECTION_REFUSED", "Ensure the MCP WebSocket relay is running on powerdev:3000")
            } else if e.contains("timeout") {
                ("TIMEOUT", "The operation took too long. Try reducing the number of agents.")
            } else if e.contains("Failed to connect") {
                ("CONNECTION_FAILED", "Cannot connect to Claude Flow. Check network connectivity.")
            } else {
                ("INITIALIZATION_FAILED", "Check the Claude Flow logs for more details.")
            };

            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to initialize swarm: {}", e),
                "code": error_code,
                "details": {
                    "suggestion": suggestion,
                    "topology": request.topology,
                    "requested_agents": request.max_agents
                }
            }))
        }
        Ok(Err(e)) => {
            error!("Failed to communicate with Claude Flow actor: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Internal communication error with Claude Flow service",
                "code": "ACTOR_COMMUNICATION_ERROR"
            }))
        }
        Err(_) => {
            error!("Swarm initialization timed out after 30 seconds");
            HttpResponse::GatewayTimeout().json(serde_json::json!({
                "success": false,
                "error": "Swarm initialization timed out. The operation took too long to complete.",
                "code": "TIMEOUT",
                "details": {
                    "timeout_seconds": 30,
                    "suggestion": "Try reducing the number of agents or check Claude Flow service health."
                }
            }))
        }
    }
}

// Add health check endpoint
pub async fn check_claude_flow_health(state: web::Data<AppState>) -> impl Responder {
    let claude_flow_available = state.claude_flow_addr.is_some();
    
    if !claude_flow_available {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "unhealthy",
            "service": "claude-flow",
            "error": "Claude Flow actor not initialized"
        }));
    }

    // Try to get active agents as a health check
    if let Some(claude_flow_addr) = &state.claude_flow_addr {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            claude_flow_addr.send(GetActiveAgents)
        ).await {
            Ok(Ok(Ok(agents))) => {
                HttpResponse::Ok().json(serde_json::json!({
                    "status": "healthy",
                    "service": "claude-flow",
                    "connected": true,
                    "active_agents": agents.len(),
                    "details": {
                        "mcp_endpoint": "powerdev:3000",
                        "transport": "websocket"
                    }
                }))
            }
            _ => {
                HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "status": "degraded",
                    "service": "claude-flow",
                    "connected": false,
                    "error": "Cannot communicate with Claude Flow service",
                    "suggestion": "Check if MCP WebSocket relay is running on powerdev:3000"
                }))
            }
        }
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "unhealthy",
            "service": "claude-flow",
            "error": "Service not available"
        }))
    }
}