#!/usr/bin/env node
/**
 * Test Dynamic Agent Architecture (DAA) features via MCP
 * This connects to the MCP relay and demonstrates DAA capabilities
 */

const WebSocket = require('ws');

async function testDAA() {
  console.log('🚀 Testing DAA Features via MCP WebSocket...\n');
  
  const ws = new WebSocket('ws://localhost:3000/ws');
  
  // Helper to send requests
  let requestId = 1;
  const sendRequest = (method, params) => {
    return new Promise((resolve, reject) => {
      const id = `req-${requestId++}`;
      const request = {
        jsonrpc: "2.0",
        id,
        method,
        params
      };
      
      console.log('📤 Sending:', JSON.stringify(request, null, 2));
      
      const handler = (data) => {
        const response = JSON.parse(data);
        if (response.id === id) {
          ws.removeListener('message', handler);
          console.log('📨 Response:', JSON.stringify(response, null, 2));
          resolve(response);
        }
      };
      
      ws.on('message', handler);
      ws.send(JSON.stringify(request));
      
      // Timeout after 10 seconds
      setTimeout(() => {
        ws.removeListener('message', handler);
        reject(new Error('Request timeout'));
      }, 10000);
    });
  };
  
  await new Promise((resolve) => {
    ws.on('open', async () => {
      console.log('✅ Connected to MCP WebSocket\n');
      
      try {
        // 1. Initialize connection
        console.log('1️⃣  Initializing MCP connection...\n');
        await sendRequest('initialize', {
          capabilities: { tools: { listChanged: true } },
          clientInfo: { name: 'DAA Test Client', version: '1.0.0' },
          protocolVersion: '2024-11-05'
        });
        
        // 2. Initialize a swarm
        console.log('\n2️⃣  Initializing DAA Swarm...\n');
        await sendRequest('tools/call', {
          name: 'swarm_init',
          arguments: {
            topology: 'hierarchical',
            maxAgents: 8,
            strategy: 'adaptive'
          }
        });
        
        // 3. Create a DAA agent with specialized capabilities
        console.log('\n3️⃣  Creating specialized DAA agent...\n');
        await sendRequest('tools/call', {
          name: 'daa_agent_create',
          arguments: {
            id: 'research-specialist',
            capabilities: ['deep-analysis', 'pattern-recognition', 'knowledge-synthesis'],
            cognitivePattern: 'lateral',
            enableMemory: true,
            learningRate: 0.8
          }
        });
        
        // 4. Create another agent for collaboration
        console.log('\n4️⃣  Creating collaborative agent...\n');
        await sendRequest('tools/call', {
          name: 'daa_agent_create',
          arguments: {
            id: 'code-specialist',
            capabilities: ['code-generation', 'optimization', 'debugging'],
            cognitivePattern: 'systems',
            enableMemory: true,
            learningRate: 0.7
          }
        });
        
        // 5. Check swarm status
        console.log('\n5️⃣  Checking swarm status...\n');
        await sendRequest('tools/call', {
          name: 'swarm_status',
          arguments: { verbose: true }
        });
        
        // 6. List all agents
        console.log('\n6️⃣  Listing all agents...\n');
        const agentList = await sendRequest('tools/call', {
          name: 'agent_list',
          arguments: {}
        });
        
        // 7. Store something in memory
        console.log('\n7️⃣  Storing data in persistent memory...\n');
        await sendRequest('tools/call', {
          name: 'memory_usage',
          arguments: {
            action: 'store',
            key: 'daa-test/config',
            value: JSON.stringify({
              testRun: new Date().toISOString(),
              agents: ['research-specialist', 'code-specialist'],
              topology: 'hierarchical'
            }),
            namespace: 'daa-demo'
          }
        });
        
        // 8. Retrieve from memory
        console.log('\n8️⃣  Retrieving from memory...\n');
        await sendRequest('tools/call', {
          name: 'memory_usage',
          arguments: {
            action: 'retrieve',
            key: 'daa-test/config',
            namespace: 'daa-demo'
          }
        });
        
        // 9. Orchestrate a task
        console.log('\n9️⃣  Orchestrating a collaborative task...\n');
        await sendRequest('tools/call', {
          name: 'task_orchestrate',
          arguments: {
            task: 'Analyze the codebase and identify optimization opportunities',
            strategy: 'parallel',
            priority: 'high'
          }
        });
        
        // 10. Check neural status
        console.log('\n🔟  Checking neural network status...\n');
        await sendRequest('tools/call', {
          name: 'neural_status',
          arguments: {}
        });
        
        console.log('\n✅ DAA feature test completed!\n');
        ws.close();
        resolve();
        
      } catch (error) {
        console.error('\n❌ Test failed:', error);
        ws.close();
        resolve();
      }
    });
    
    ws.on('error', (error) => {
      console.error('❌ WebSocket error:', error);
      resolve();
    });
  });
}

// Run the test
testDAA().then(() => {
  console.log('Test completed');
  process.exit(0);
}).catch((error) => {
  console.error('Test error:', error);
  process.exit(1);
});