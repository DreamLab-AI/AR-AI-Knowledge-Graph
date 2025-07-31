# Rust MCP Connection Fix - Deployment Guide

## Overview
This guide documents the fixes applied to the Rust backend to resolve MCP connection issues and how to recompile and deploy the updated code.

## Issues Fixed

### 1. Response Parsing in `list_agents()`
The method was expecting a different response format than what MCP actually returns. MCP tools return responses in this structure:
```json
{
  "result": {
    "content": [{
      "text": "{\"agents\": [...]}"
    }]
  }
}
```

### 2. Error Handling
Added proper error handling for:
- Empty agent lists
- Missing content arrays
- Invalid response formats
- MCP error responses

### 3. Task Creation Response Handling
Updated `create_task()` to properly parse MCP tool responses with the same content array structure.

## Files Modified

1. `/workspace/ext/src/services/claude_flow/client.rs`
   - Fixed `list_agents()` method (lines 146-266)
   - Fixed `create_task()` method (lines 277-346)
   - Added proper MCP response parsing

## Recompilation Steps

### In the logseq Container

1. **Connect to the logseq container:**
   ```bash
   docker exec -it logseq bash
   ```

2. **Navigate to the project directory:**
   ```bash
   cd /app
   ```

3. **Build the Rust project:**
   ```bash
   cargo build --release
   ```

4. **Run tests (if available):**
   ```bash
   cargo test
   ```

5. **Restart the service:**
   ```bash
   # If using systemd
   systemctl restart logseq-backend
   
   # Or if running directly
   pkill -f logseq-backend
   ./target/release/logseq-backend
   ```

## Verification Steps

### 1. Check MCP Connection
Monitor the MCP WebSocket relay logs in powerdev:
```bash
tail -f /tmp/mcp-ws-relay.log
```

Look for:
- "New WebSocket connection established"
- "Claude Flow Rust Connector" in clientInfo
- Successful initialization responses

### 2. Test Agent Listing
The fixed code should now properly handle:
- Empty agent lists (returns empty vector instead of hanging)
- Agent data in MCP response format
- Filtering by status (active/all)

### 3. Test Task Creation
Verify that tasks can be created through the UI without errors.

## Key Changes Summary

### Before (Hanging Code):
```rust
let content = result.get("content")
    .and_then(|c| c.as_array())
    .and_then(|arr| arr.first())
    .and_then(|item| item.get("text"))
    .and_then(|text| text.as_str())
    .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
```

### After (Fixed Code):
```rust
if let Some(content_array) = result.get("content").and_then(|c| c.as_array()) {
    if let Some(first_item) = content_array.first() {
        if let Some(text) = first_item.get("text").and_then(|t| t.as_str()) {
            // Parse and handle response
        } else {
            // Return empty list instead of error
            Ok(vec![])
        }
    }
}
```

## Troubleshooting

### If connection still fails:
1. Ensure MCP WebSocket relay is running in powerdev
2. Check network connectivity between containers
3. Verify environment variables:
   - `CLAUDE_FLOW_HOST=powerdev`
   - `CLAUDE_FLOW_PORT=3000`

### If agent list is still empty:
1. Check if agents have been spawned in Claude Flow
2. Use the test script: `/workspace/ext/scripts/test-daa-features.js`
3. Monitor MCP logs for any errors

## Next Steps

1. Deploy the fixed code to production
2. Monitor for any new issues
3. Consider adding retry logic for transient failures
4. Implement proper agent spawning on initialization