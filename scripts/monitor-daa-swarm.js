#!/usr/bin/env node
/**
 * Monitor DAA swarm in real-time
 * Shows agent activity, task progress, and system metrics
 */

const WebSocket = require('ws');
const readline = require('readline');

// ANSI color codes
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  dim: '\x1b[2m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
  white: '\x1b[37m'
};

async function monitorSwarm() {
  const ws = new WebSocket('ws://localhost:3000/ws');
  let requestId = 1;
  let monitoring = true;
  
  // Clear screen and move cursor
  const clearScreen = () => process.stdout.write('\x1b[2J\x1b[H');
  
  const sendRequest = (method, params) => {
    return new Promise((resolve, reject) => {
      const id = `mon-${requestId++}`;
      const request = { jsonrpc: "2.0", id, method, params };
      
      const handler = (data) => {
        try {
          const response = JSON.parse(data);
          if (response.id === id) {
            ws.removeListener('message', handler);
            resolve(response);
          }
        } catch (e) {
          // Ignore parse errors
        }
      };
      
      ws.on('message', handler);
      ws.send(JSON.stringify(request));
      
      setTimeout(() => {
        ws.removeListener('message', handler);
        resolve(null);
      }, 5000);
    });
  };
  
  const displayStatus = async () => {
    clearScreen();
    console.log(`${colors.bright}${colors.cyan}╔═══════════════════════════════════════════════════════════════╗${colors.reset}`);
    console.log(`${colors.bright}${colors.cyan}║          🤖 DAA SWARM MONITOR - REAL-TIME STATUS             ║${colors.reset}`);
    console.log(`${colors.bright}${colors.cyan}╚═══════════════════════════════════════════════════════════════╝${colors.reset}\n`);
    
    try {
      // Get swarm status
      const swarmStatus = await sendRequest('tools/call', {
        name: 'swarm_status',
        arguments: { verbose: true }
      });
      
      if (swarmStatus?.result?.content?.[0]?.text) {
        const status = JSON.parse(swarmStatus.result.content[0].text);
        
        console.log(`${colors.bright}🐝 SWARM STATUS${colors.reset}`);
        console.log(`├── ID: ${colors.yellow}${status.swarmId || 'Not initialized'}${colors.reset}`);
        console.log(`├── Topology: ${colors.blue}${status.topology || 'N/A'}${colors.reset}`);
        console.log(`├── Active Agents: ${colors.green}${status.activeAgents || 0}${colors.reset}`);
        console.log(`└── Status: ${status.status === 'active' ? colors.green : colors.red}${status.status || 'inactive'}${colors.reset}\n`);
      }
      
      // Get agent list
      const agentList = await sendRequest('tools/call', {
        name: 'agent_list',
        arguments: {}
      });
      
      if (agentList?.result?.content?.[0]?.text) {
        const agents = JSON.parse(agentList.result.content[0].text);
        
        console.log(`${colors.bright}👥 ACTIVE AGENTS${colors.reset}`);
        if (agents.agents && agents.agents.length > 0) {
          agents.agents.forEach((agent, idx) => {
            const statusColor = agent.status === 'active' ? colors.green : 
                               agent.status === 'busy' ? colors.yellow : colors.dim;
            const icon = agent.type === 'coordinator' ? '👑' :
                        agent.type === 'researcher' ? '🔍' :
                        agent.type === 'coder' ? '💻' :
                        agent.type === 'analyst' ? '📊' : '🤖';
            
            console.log(`${idx === agents.agents.length - 1 ? '└──' : '├──'} ${icon} ${agent.name || agent.id}`);
            console.log(`${idx === agents.agents.length - 1 ? '   ' : '│  '} ├── Type: ${colors.magenta}${agent.type}${colors.reset}`);
            console.log(`${idx === agents.agents.length - 1 ? '   ' : '│  '} ├── Status: ${statusColor}${agent.status}${colors.reset}`);
            console.log(`${idx === agents.agents.length - 1 ? '   ' : '│  '} └── Capabilities: ${colors.dim}${(agent.capabilities || []).join(', ') || 'none'}${colors.reset}`);
          });
        } else {
          console.log(`└── ${colors.dim}No active agents${colors.reset}`);
        }
        console.log('');
      }
      
      // Get performance metrics
      const perfReport = await sendRequest('tools/call', {
        name: 'performance_report',
        arguments: { format: 'summary' }
      });
      
      if (perfReport?.result?.content?.[0]?.text) {
        try {
          const perf = JSON.parse(perfReport.result.content[0].text);
          console.log(`${colors.bright}📊 PERFORMANCE METRICS${colors.reset}`);
          console.log(`├── CPU Usage: ${colors.yellow}${perf.cpu || 'N/A'}%${colors.reset}`);
          console.log(`├── Memory: ${colors.blue}${perf.memory || 'N/A'}MB${colors.reset}`);
          console.log(`├── Tasks Completed: ${colors.green}${perf.tasksCompleted || 0}${colors.reset}`);
          console.log(`└── Avg Response Time: ${colors.cyan}${perf.avgResponseTime || 'N/A'}ms${colors.reset}\n`);
        } catch (e) {
          // Ignore parse errors
        }
      }
      
      // Get memory usage
      const memoryStats = await sendRequest('tools/call', {
        name: 'memory_usage',
        arguments: { action: 'list', namespace: 'default' }
      });
      
      if (memoryStats?.result?.content?.[0]?.text) {
        try {
          const memory = JSON.parse(memoryStats.result.content[0].text);
          console.log(`${colors.bright}💾 MEMORY USAGE${colors.reset}`);
          console.log(`├── Total Keys: ${colors.yellow}${memory.count || 0}${colors.reset}`);
          console.log(`└── Namespaces: ${colors.blue}${(memory.namespaces || ['default']).join(', ')}${colors.reset}\n`);
        } catch (e) {
          // Ignore parse errors
        }
      }
      
    } catch (error) {
      console.log(`${colors.red}Error fetching status: ${error.message}${colors.reset}`);
    }
    
    console.log(`${colors.dim}─────────────────────────────────────────────────────────────────${colors.reset}`);
    console.log(`${colors.dim}Press 'q' to quit, 'r' to refresh manually${colors.reset}`);
  };
  
  // Set up keyboard input
  readline.emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY) {
    process.stdin.setRawMode(true);
  }
  
  process.stdin.on('keypress', (str, key) => {
    if (key.name === 'q' || (key.ctrl && key.name === 'c')) {
      monitoring = false;
      clearScreen();
      console.log('Monitoring stopped.');
      ws.close();
      process.exit(0);
    } else if (key.name === 'r') {
      displayStatus();
    }
  });
  
  await new Promise((resolve) => {
    ws.on('open', async () => {
      console.log('Connecting to DAA swarm...');
      
      // Initialize connection
      await sendRequest('initialize', {
        capabilities: { tools: { listChanged: true } },
        clientInfo: { name: 'DAA Monitor', version: '1.0.0' },
        protocolVersion: '2024-11-05'
      });
      
      // Start monitoring loop
      const monitorLoop = async () => {
        while (monitoring) {
          await displayStatus();
          await new Promise(r => setTimeout(r, 2000)); // Update every 2 seconds
        }
      };
      
      monitorLoop();
    });
    
    ws.on('error', (error) => {
      console.error(`${colors.red}WebSocket error: ${error.message}${colors.reset}`);
      resolve();
    });
  });
}

// Run the monitor
monitorSwarm().catch((error) => {
  console.error('Monitor error:', error);
  process.exit(1);
});