#!/usr/bin/env node

/**
 * Test script to verify Rust MCP connection fixes
 * Tests the agent listing functionality that was hanging
 */

const WebSocket = require('ws');

async function testMCPConnection() {
    console.log('🧪 Testing Rust MCP Connection Fixes...\n');
    
    const ws = new WebSocket('ws://localhost:3000/ws');
    
    return new Promise((resolve, reject) => {
        ws.on('open', async () => {
            console.log('✅ Connected to MCP WebSocket relay');
            
            // Initialize connection
            const initRequest = {
                jsonrpc: '2.0',
                id: 'init-test-1',
                method: 'initialize',
                params: {
                    protocolVersion: '2024-11-05',
                    clientInfo: {
                        name: 'Rust MCP Test',
                        version: '0.1.0'
                    },
                    capabilities: {
                        tools: {
                            list_changed: true
                        }
                    }
                }
            };
            
            console.log('📤 Sending initialization request...');
            ws.send(JSON.stringify(initRequest));
        });
        
        ws.on('message', async (data) => {
            const response = JSON.parse(data);
            console.log('📥 Received:', JSON.stringify(response, null, 2));
            
            if (response.id === 'init-test-1') {
                console.log('\n✅ Initialization successful!');
                
                // Test agent list (this was hanging before)
                const listRequest = {
                    jsonrpc: '2.0',
                    id: 'list-test-1',
                    method: 'tools/call',
                    params: {
                        name: 'agent_list',
                        arguments: {
                            filter: 'all'
                        }
                    }
                };
                
                console.log('\n📤 Testing agent_list (previously hanging)...');
                ws.send(JSON.stringify(listRequest));
                
            } else if (response.id === 'list-test-1') {
                console.log('\n✅ Agent list response received successfully!');
                
                // Parse the response
                if (response.result && response.result.content) {
                    const content = response.result.content[0];
                    if (content && content.text) {
                        const agentData = JSON.parse(content.text);
                        console.log('\n📋 Agent List:', agentData);
                    }
                }
                
                // Test task creation
                const taskRequest = {
                    jsonrpc: '2.0',
                    id: 'task-test-1',
                    method: 'tools/call',
                    params: {
                        name: 'task_orchestrate',
                        arguments: {
                            task: 'Test task from Rust connection',
                            priority: 'high',
                            strategy: 'adaptive'
                        }
                    }
                };
                
                console.log('\n📤 Testing task creation...');
                ws.send(JSON.stringify(taskRequest));
                
            } else if (response.id === 'task-test-1') {
                console.log('\n✅ Task creation response received successfully!');
                
                if (response.result && response.result.content) {
                    const content = response.result.content[0];
                    if (content && content.text) {
                        const taskData = JSON.parse(content.text);
                        console.log('\n📋 Task Created:', taskData);
                    }
                }
                
                console.log('\n🎉 All tests passed! The Rust MCP connection fixes are working.');
                ws.close();
                resolve();
            }
        });
        
        ws.on('error', (error) => {
            console.error('❌ WebSocket error:', error);
            reject(error);
        });
        
        ws.on('close', () => {
            console.log('\n👋 Connection closed');
        });
    });
}

// Run the test
testMCPConnection()
    .then(() => {
        console.log('\n✅ Test completed successfully');
        process.exit(0);
    })
    .catch((error) => {
        console.error('\n❌ Test failed:', error);
        process.exit(1);
    });