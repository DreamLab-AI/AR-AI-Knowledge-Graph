# REST API Reference

## Overview
The REST API provides endpoints for graph data management, content operations, and system status.

## Base URL
```
http://localhost:4000/api
```
Default is http://localhost:4000/api when running locally via Docker with Nginx. The actual backend runs on port 3001 (or as configured in settings.yaml/env) and is proxied by Nginx. For production, it's typically https://www.visionflow.info/api (or your configured domain).

## Authentication

All API requests primarily use Nostr authentication.

#### Login
```http
POST /api/auth/nostr
```

**Request Body:**
```json
{
  "id": "event_id_hex_string",
  "pubkey": "user_hex_pubkey",
  "created_at": 1678886400, // Unix timestamp (seconds)
  "kind": 22242,
  "tags": [
    ["relay", "wss://some.relay.com"],
    ["challenge", "a_random_challenge_string"]
  ],
  "content": "LogseqXR Authentication",
  "sig": "event_signature_hex_string"
}
```
Refers to `src/services/nostr_service.rs::AuthEvent`.

**Response:**
```json
{
  "user": {
    "pubkey": "user_hex_pubkey",
    "npub": "user_npub_string", // Optional
    "isPowerUser": true // boolean
  },
  "token": "session_token_string",
  "expiresAt": 1234567890, // Unix timestamp (seconds)
  "features": ["feature1", "feature2"] // List of enabled features for the user
}
```
Matches `AuthResponse` from `src/handlers/nostr_handler.rs`.

#### Verify Token
```http
POST /api/auth/nostr/verify
```

**Request Body:**
```json
{
  "pubkey": "user_hex_pubkey",
  "token": "session_token_string"
}
```
Matches `ValidateRequest` from `src/handlers/nostr_handler.rs`.

**Response Body:**
```json
{
  "valid": true, // boolean
  "user": { // Optional
    "pubkey": "user_hex_pubkey",
    "npub": "user_npub_string",
    "isPowerUser": false
  },
  "features": ["feature1"] // List of enabled features if valid
}
```
Matches `VerifyResponse` from `src/handlers/nostr_handler.rs`.

#### Logout
```http
DELETE /api/auth/nostr
```

**Request Body:**
```json
{
  "pubkey": "user_hex_pubkey",
  "token": "session_token_string"
}
```
Matches `ValidateRequest` from `src/handlers/nostr_handler.rs`.

## Graph API

### Get Graph Data
```http
GET /api/graph/data
```

Returns `GraphResponse` from `src/handlers/api_handler/graph/mod.rs`:
```json
{
  "nodes": [
    // Array of crate::models::node::Node
  ],
  "edges": [
    // Array of crate::models::edge::Edge
  ],
  "metadata": {
    // HashMap<String, crate::models::metadata::Metadata>
  }
}
```
Note: The `Node` model used in this response is defined in `src/models/node.rs` and uses a `u32` for the `id` field.

### Get Paginated Graph Data
```http
GET /api/graph/data/paginated
```

**Query Parameters:**
- `page`: Page number (integer, default: 1)
- `pageSize`: Items per page (integer, default: 100, camelCase)
- `sort`: Sort field (string, optional)
- `filter`: Filter expression (string, optional)

**Response:**
```json
{
  "nodes": [...],
  "edges": [...],
  "metadata": {},
  "totalPages": 0,
  "currentPage": 1,
  "total_nodes": 0,
  "page_size": 100
}

### Update Graph
```http
POST /api/graph/update
```

This endpoint triggers a full re-fetch of files from the source (e.g., GitHub) by `FileService`, updates the `MetadataStore`, and then rebuilds the entire graph structure in `GraphService`. It does not accept client-side graph data for partial updates.

### Refresh Graph
```http
POST /api/graph/refresh
```
This endpoint rebuilds the graph structure in `GraphService` using the currently existing `MetadataStore` on the server. It does not re-fetch files.

## Files API

### Process Files
```http
POST /api/files/process
```

Triggers fetching and processing of Markdown files.

**Response:**
```json
{
  "status": "success",
  "processed_files": ["file1.md", "file2.md"]
}
```

### Get File Content
```http
GET /api/files/get_content/{filename}
```

### Upload Content
```http
POST /api/files/upload
```

The endpoint `/api/files/upload` is not defined in `src/handlers/api_handler/files/mod.rs`. This section should be removed or marked as "Not Implemented / Deprecated".
(Marking as Not Implemented for now)
**This endpoint is not implemented.**

## Settings API

### Get Public Settings
```http
GET /api/user-settings
```

Returns `UISettings` (camelCase) as defined in `src/models/ui_settings.rs`, derived from the server's `AppFullSettings`. This endpoint does not require authentication.

### Get User-Specific Settings
```http
GET /api/user-settings/sync
```

Requires authentication.
**GET Response:** Returns `UISettings`. For power users, these are global settings. For regular users, these are their persisted settings.
**POST Request Body:** `ClientSettingsPayload` from `src/models/client_settings_payload.rs` (camelCase).
**POST Response:** The updated `UISettings`.

### Get Visualisation Settings by Category
```http
GET /api/visualisation/settings/{category}
```

The handler `get_visualisation_settings` (mapped to `/api/visualisation/get_settings/{category}` in `src/handlers/api_handler/visualisation/mod.rs`) actually returns the entire `AppFullSettings` struct, not just a category. The path parameter `{category}` is not used by this specific handler.
The handler `get_category_settings` (mapped to `/api/visualisation/settings/{category}`) does return a specific category.
The documentation path `/api/visualisation/settings/{category}` matches `get_category_settings`. This endpoint returns a specific category as a JSON object.

### Update API Keys
```http
POST /api/auth/nostr/api-keys
```

**Request Body:**
```json
{
  "perplexity": "optional_api_key_string",
  "openai": "optional_api_key_string",
  "ragflow": "optional_api_key_string"
}
```
Matches `ApiKeysRequest` from `src/handlers/nostr_handler.rs`.

**Response:**
```json
{
  "pubkey": "user_hex_pubkey",
  "npub": "user_npub_string", // Optional
  "isPowerUser": true // boolean
}
```
Returns `UserResponseDTO`.

## AI Services

### RAGFlow Chat
```http
POST /api/ragflow/chat
```

**Request Body:**
```json
{
  "question": "Your question here",
  "sessionId": "optional-previous-session-id-string",
  "stream": false // Optional boolean
}
```
Matches `RagflowChatRequest` from `src/models/ragflow_chat.rs`.

**Response:**
```json
{
  "answer": "The response from RAGFlow AI",
  "sessionId": "session-id-string"
}
```
Matches `RagflowChatResponse` from `src/models/ragflow_chat.rs`.


## Bots API

### Initialize Swarm (Hive Mind)
```http
POST /api/bots/initialize-swarm
```

Spawns a Claude Flow hive mind with specified configuration and task.

**Request Body:**
```json
{
  "topology": "mesh" | "hierarchical" | "ring" | "star",
  "maxAgents": 3-20,
  "strategy": "adaptive",
  "enableNeural": true,
  "agentTypes": [
    "coordinator",
    "researcher", 
    "coder",
    "analyst",
    "tester",
    "architect",
    "optimizer",
    "reviewer",
    "documenter"
  ],
  "customPrompt": "Build a REST API with authentication" // Required task description
}
```

**Response:**
```json
{
  "success": true,
  "message": "Swarm initialization started"
}
```

**Error Response:**
```json
{
  "success": false,
  "error": "Failed to initialize swarm: <error message>"
}
```

### Get Bots Data
```http
GET /api/bots/data
```

Returns current swarm agent data as a graph structure.

**Response:**
```json
{
  "nodes": [
    {
      "id": 1000,
      "label": "Coordinator Alpha",
      "metadata_id": "agent-1",
      "data": {
        "position": {"x": 0, "y": 0, "z": 0},
        "velocity": {"x": 0, "y": 0, "z": 0},
        "mass": 5,
        "flags": 1
      },
      "metadata": {
        "type": "coordinator",
        "status": "active",
        "health": "95.0",
        "cpu_usage": "45.0",
        "workload": "0.7"
      }
    }
  ],
  "edges": [
    {
      "id": "edge-1",
      "source": 1000,
      "target": 1001,
      "weight": 1.0,
      "edge_type": "15 msgs"
    }
  ]
}
```

### Update Bots Data
```http
POST /api/bots/update
```

Updates the swarm visualization with new agent data.

**Request Body:**
```json
{
  "nodes": [
    {
      "id": "agent-1",
      "type": "coordinator",
      "status": "active",
      "name": "Coordinator Alpha",
      "cpu_usage": 45.0,
      "health": 95.0,
      "workload": 0.7
    }
  ],
  "edges": [
    {
      "id": "edge-1",
      "source": "agent-1",
      "target": "agent-2",
      "data_volume": 1024.0,
      "message_count": 15
    }
  ]
}
```

## System Status

### Health Check
```http
GET /api/health
```

**Response:**
```json
{
  "status": "healthy",
  "metadata_count": 123,
  "nodes_count": 456,
  "edges_count": 789
}
```

### Physics Simulation Status
```http
GET /api/health/physics
```

**Response:**
```json
{
  "status": "string (e.g., 'running', 'idle', 'error')",
  "details": "string (e.g., 'Simulation is active with X nodes' or error message)",
  "timestamp": 1234567890 // Unix timestamp
}
```
Returns `PhysicsSimulationStatus` from `src/handlers/health_handler.rs`.


## Error Responses

Error responses are often simple JSON like `{"error": "message string"}` or `{"status": "error", "message": "message string"}`.
The structured format `{"error": {"code": ..., "message": ..., "details": ...}}` is not consistently used across all handlers.

### Common HTTP Status Codes for Errors
- `400 Bad Request`: Invalid parameters or request payload.
- `401 Unauthorized`: Invalid or missing authentication token.
- `403 Forbidden`: Valid token but insufficient permissions for the requested operation.
- `404 Not Found`: The requested resource or endpoint does not exist.
- `422 Unprocessable Entity`: The request was well-formed but could not be processed (e.g., semantic errors in Nostr event).
- `500 Internal Server Error`: A generic error occurred on the server.
- `503 Service Unavailable`: The server is temporarily unable to handle the request (e.g., during maintenance or if a dependent service is down).

## Related Documentation
- [WebSocket API](./websocket.md)
- [Development Setup](../development/setup.md)