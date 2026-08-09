// Test file for MCP response parsing functionality
// This tests the type-safe JSON parsing to eliminate brittle double-parsing

use serde_json::{json, Value};
use visionclaw_server::types::mcp_responses::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_text_content_json_parsing() {
        // Test that nested JSON strings are automatically parsed
        let json_data = json!({
            "type": "text",
            "text": "{\"agents\": [{\"id\": \"agent1\", \"name\": \"Test Agent\", \"type\": \"coder\", \"status\": \"active\"}]}"
        });

        let content: McpTextContent = serde_json::from_value(json_data).unwrap();

        // The text field should now be parsed as a JSON Value, not a string
        let agents = content.text.get("agents").unwrap();
        assert!(agents.is_array());

        let agents_array = agents.as_array().unwrap();
        assert_eq!(agents_array.len(), 1);

        let agent = &agents_array[0];
        assert_eq!(agent.get("id").unwrap().as_str().unwrap(), "agent1");
        assert_eq!(agent.get("name").unwrap().as_str().unwrap(), "Test Agent");
    }

    #[test]
    fn test_full_mcp_response_parsing() {
        // Test complete MCP response structure
        let mcp_response_json = json!({
            "result": {
                "content": [{
                    "type": "text",
                    "text": "{\"agents\": [{\"id\": \"agent1\", \"name\": \"Coordinator\", \"type\": \"coordinator\", \"status\": \"active\"}, {\"id\": \"agent2\", \"name\": \"Worker\", \"type\": \"coder\", \"status\": \"idle\"}]}"
                }]
            }
        });

        // Single deserialization instead of double-parsing
        let response: McpResponse<McpContentResult> =
            serde_json::from_value(mcp_response_json).unwrap();

        assert!(response.is_success());

        if let McpResponse::Success(success) = response {
            let agent_list: AgentListResponse = success.result.extract_data().unwrap();
            assert_eq!(agent_list.agents.len(), 2);

            assert_eq!(agent_list.agents[0].id, "agent1");
            assert_eq!(agent_list.agents[0].name, "Coordinator");
            assert_eq!(agent_list.agents[0].agent_type, "coordinator");

            assert_eq!(agent_list.agents[1].id, "agent2");
            assert_eq!(agent_list.agents[1].name, "Worker");
            assert_eq!(agent_list.agents[1].agent_type, "coder");
        }
    }

    #[test]
    fn test_mcp_error_response() {
        let error_response_json = json!({
            "error": {
                "code": -32601,
                "message": "Method not found",
                "data": null
            }
        });

        let response: McpResponse<McpContentResult> =
            serde_json::from_value(error_response_json).unwrap();
        assert!(response.is_error());

        match response {
            McpResponse::Error(error) => {
                assert_eq!(error.error.code, -32601);
                assert_eq!(error.error.message, "Method not found");
            }
            _ => panic!("Expected error response"),
        }
    }

    #[test]
    fn test_robust_error_handling() {
        // Test that invalid JSON in text field is handled gracefully
        let invalid_json = json!({
            "result": {
                "content": [{
                    "type": "text",
                    "text": "{invalid json here}"
                }]
            }
        });

        let response: Result<McpResponse<McpContentResult>, _> =
            serde_json::from_value(invalid_json);

        // Should fail gracefully at the deserialization level
        assert!(response.is_err());
    }

    #[test]
    fn test_empty_content_handling() {
        let empty_content_json = json!({
            "result": {
                "content": []
            }
        });

        let response: McpResponse<McpContentResult> =
            serde_json::from_value(empty_content_json).unwrap();

        if let McpResponse::Success(success) = response {
            let result: Result<AgentListResponse, McpParseError> = success.result.extract_data();
            assert!(result.is_err());

            match result {
                Err(McpParseError::MissingContent) => {
                    // This is the expected error
                }
                _ => panic!("Expected MissingContent error"),
            }
        }
    }

    #[test]
    fn test_backwards_compatibility() {
        // Ensure we can still parse old formats for fallback
        let legacy_format = json!({
            "agents": [
                {"id": "legacy1", "name": "Legacy Agent", "type": "coder", "status": "active"}
            ]
        });

        // Direct parsing should work
        let agent_list: AgentListResponse = serde_json::from_value(legacy_format).unwrap();
        assert_eq!(agent_list.agents.len(), 1);
        assert_eq!(agent_list.agents[0].id, "legacy1");
    }

    #[test]
    fn test_multiple_content_items() {
        let multi_content_json = json!({
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "{\"agents\": [{\"id\": \"agent1\", \"name\": \"Agent 1\", \"type\": \"coder\", \"status\": \"active\"}]}"
                    },
                    {
                        "type": "text",
                        "text": "{\"agents\": [{\"id\": \"agent2\", \"name\": \"Agent 2\", \"type\": \"tester\", \"status\": \"idle\"}]}"
                    }
                ]
            }
        });

        let response: McpResponse<McpContentResult> =
            serde_json::from_value(multi_content_json).unwrap();

        if let McpResponse::Success(success) = response {
            // Test extracting all data (not just first)
            let all_agent_lists: Vec<AgentListResponse> =
                success.result.extract_all_data().unwrap();
            assert_eq!(all_agent_lists.len(), 2);

            assert_eq!(all_agent_lists[0].agents[0].id, "agent1");
            assert_eq!(all_agent_lists[1].agents[0].id, "agent2");
        }
    }

    #[test]
    fn test_performance_vs_double_parsing() {
        use std::time::Instant;

        let test_response = json!({
            "result": {
                "content": [{
                    "type": "text",
                    "text": "{\"agents\": [{\"id\": \"perf_test\", \"name\": \"Performance Test\", \"type\": \"coder\", \"status\": \"active\"}]}"
                }]
            }
        });

        // Measure new single-pass parsing
        let start = Instant::now();
        for _ in 0..1000 {
            let _response: McpResponse<McpContentResult> =
                serde_json::from_value(test_response.clone()).unwrap();
        }
        let single_pass_time = start.elapsed();

        // Measure old double-parsing approach (simulated)
        let start = Instant::now();
        for _ in 0..1000 {
            let first_parse: Value = serde_json::from_value(test_response.clone()).unwrap();
            if let Some(result) = first_parse.get("result") {
                if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                    if let Some(first_content) = content.first() {
                        if let Some(text) = first_content.get("text").and_then(|t| t.as_str()) {
                            let _second_parse: Value = serde_json::from_str(text).unwrap();
                        }
                    }
                }
            }
        }
        let double_pass_time = start.elapsed();

        println!("Single-pass parsing: {:?}", single_pass_time);
        println!("Double-pass parsing: {:?}", double_pass_time);

        // Single-pass should be faster or at least not significantly slower
        assert!(single_pass_time <= double_pass_time * 2); // Allow 2x tolerance
    }
}

// Integration test for the complete flow
#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test that demonstrates the elimination of brittle double-parsing
    #[test]
    fn test_brittle_parsing_elimination() {
        // This is the problematic pattern we're fixing:
        // 1. Parse JSON response
        // 2. Extract "text" field as string
        // 3. Parse that string as JSON again

        let nested_json_response = json!({
            "result": {
                "content": [{
                    "type": "text",
                    "text": "{\"agents\":[{\"id\":\"robust_test\",\"name\":\"Robust Agent\",\"type\":\"coordinator\",\"status\":\"active\"}]}"
                }]
            }
        });

        // OLD BRITTLE WAY (what we're replacing):
        // let first_parse: Value = serde_json::from_value(nested_json_response.clone()).unwrap();
        // let text = first_parse.get("result").unwrap()
        //     .get("content").unwrap().as_array().unwrap()
        //     .first().unwrap()
        //     .get("text").unwrap().as_str().unwrap();
        // let second_parse: Value = serde_json::from_str(text).unwrap(); // BRITTLE!
        // let agents = second_parse.get("agents").unwrap();

        // NEW ROBUST WAY (type-safe single pass):
        let mcp_response: McpResponse<McpContentResult> =
            serde_json::from_value(nested_json_response).unwrap();
        let agent_list: AgentListResponse =
            mcp_response.into_result().unwrap().extract_data().unwrap();

        assert_eq!(agent_list.agents.len(), 1);
        assert_eq!(agent_list.agents[0].id, "robust_test");
        assert_eq!(agent_list.agents[0].name, "Robust Agent");

        // Key improvements:
        // 1. Single deserialization pass
        // 2. Type-safe structures
        // 3. Proper error handling
        // 4. No string manipulation
        // 5. Works with any JSON formatting
    }
}
