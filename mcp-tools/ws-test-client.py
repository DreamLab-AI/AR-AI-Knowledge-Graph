#!/usr/bin/env python3
"""
MCP WebSocket Test Client
Tests WebSocket connectivity and data streaming from MCP Orchestrator
"""

import asyncio
import json
import sys
from datetime import datetime
from typing import Dict, Any
import websockets
import click
from colorama import init, Fore, Style
from tabulate import tabulate

init(autoreset=True)

class MCPWebSocketClient:
    def __init__(self, url: str):
        self.url = url
        self.ws = None
        self.client_id = None
        self.stats = {
            'messages_received': 0,
            'agents_count': 0,
            'communications_count': 0,
            'last_update': None,
            'connection_time': None
        }

    async def connect(self):
        """Connect to WebSocket server"""
        try:
            print(f"{Fore.BLUE}Connecting to {self.url}...")
            self.ws = await websockets.connect(self.url)
            self.stats['connection_time'] = datetime.now()
            print(f"{Fore.GREEN}✓ Connected successfully!")
            return True
        except Exception as e:
            print(f"{Fore.RED}✗ Connection failed: {e}")
            return False

    async def handle_message(self, message: Dict[str, Any]):
        """Handle incoming WebSocket message"""
        self.stats['messages_received'] += 1
        msg_type = message.get('type', 'unknown')

        if msg_type == 'welcome':
            self.client_id = message.get('clientId')
            print(f"{Fore.CYAN}Welcome message received!")
            print(f"Client ID: {self.client_id}")
            self.display_data(message.get('data', {}))

        elif msg_type == 'mcp-update':
            print(f"\n{Fore.YELLOW}[{datetime.now().strftime('%H:%M:%S')}] MCP Update received")
            self.display_data(message.get('data', {}))

        elif msg_type == 'mcp-response':
            print(f"{Fore.GREEN}MCP Response for {message.get('tool')}")
            print(json.dumps(message.get('result', {}), indent=2))

        elif msg_type == 'mcp-error':
            print(f"{Fore.RED}MCP Error for {message.get('tool')}: {message.get('error')}")

        elif msg_type == 'pong':
            print(f"{Fore.BLUE}Pong received at {message.get('timestamp')}")

    def display_data(self, data: Dict[str, Any]):
        """Display MCP data in a formatted way"""
        # Update stats
        agents = data.get('agents', [])
        communications = data.get('communications', [])
        self.stats['agents_count'] = len(agents)
        self.stats['communications_count'] = len(communications)
        self.stats['last_update'] = data.get('lastUpdate', 'N/A')

        # Display agents table
        if agents:
            print(f"\n{Fore.CYAN}Agents ({len(agents)} active):")
            agent_data = []
            for agent in agents[:5]:  # Show first 5
                agent_data.append([
                    agent.get('id', 'N/A')[:12],
                    agent.get('type', 'N/A'),
                    agent.get('status', 'N/A'),
                    f"{agent.get('health', 0)}%",
                    f"{agent.get('cpuUsage', 0):.1f}%",
                    f"{agent.get('memoryUsage', 0):.1f}%"
                ])

            headers = ['ID', 'Type', 'Status', 'Health', 'CPU', 'Memory']
            print(tabulate(agent_data, headers=headers, tablefmt='grid'))

        # Display token usage
        token_usage = data.get('tokenUsage', {})
        if token_usage:
            print(f"\n{Fore.CYAN}Token Usage:")
            print(f"Total: {token_usage.get('total', 0):,}")
            by_agent = token_usage.get('byAgent', {})
            if by_agent:
                for agent_type, tokens in by_agent.items():
                    print(f"  {agent_type}: {tokens:,}")

        # Display recent communications
        if communications:
            print(f"\n{Fore.CYAN}Recent Communications ({len(communications)} total):")
            comm_data = []
            for comm in communications[:3]:  # Show first 3
                comm_data.append([
                    comm.get('timestamp', 'N/A')[-8:],
                    comm.get('sender', 'N/A')[:12],
                    ', '.join([r[:8] for r in comm.get('receivers', [])])[:20],
                    f"{comm.get('metadata', {}).get('size', 0)} bytes"
                ])

            headers = ['Time', 'Sender', 'Receivers', 'Size']
            print(tabulate(comm_data, headers=headers, tablefmt='grid'))

        # Display system health
        system_health = data.get('systemHealth', {})
        if system_health:
            print(f"\n{Fore.CYAN}System Health: {Fore.GREEN if system_health.get('overall') == 'healthy' else Fore.YELLOW}{system_health.get('overall', 'unknown')}")

    async def send_message(self, message: Dict[str, Any]):
        """Send a message to the server"""
        if self.ws:
            await self.ws.send(json.dumps(message))
            print(f"{Fore.BLUE}Sent: {message['type']}")

    async def ping(self):
        """Send ping message"""
        await self.send_message({'type': 'ping'})

    async def request_tool(self, tool: str):
        """Request MCP tool execution"""
        await self.send_message({
            'type': 'mcp-request',
            'requestId': f'test-{datetime.now().timestamp()}',
            'tool': tool
        })

    async def subscribe(self, topics: list):
        """Subscribe to specific topics"""
        await self.send_message({
            'type': 'subscribe',
            'topics': topics
        })

    async def listen(self):
        """Listen for messages"""
        async for message in self.ws:
            try:
                data = json.loads(message)
                await self.handle_message(data)
            except json.JSONDecodeError:
                print(f"{Fore.RED}Invalid JSON received: {message}")
            except Exception as e:
                print(f"{Fore.RED}Error handling message: {e}")

    def display_stats(self):
        """Display connection statistics"""
        print(f"\n{Fore.CYAN}Connection Statistics:")
        print(f"Messages received: {self.stats['messages_received']}")
        print(f"Active agents: {self.stats['agents_count']}")
        print(f"Communications tracked: {self.stats['communications_count']}")
        print(f"Last update: {self.stats['last_update']}")
        if self.stats['connection_time']:
            duration = datetime.now() - self.stats['connection_time']
            print(f"Connection duration: {duration}")

    async def close(self):
        """Close WebSocket connection"""
        if self.ws:
            await self.ws.close()
            print(f"{Fore.YELLOW}Connection closed")

@click.command()
@click.option('--url', default='ws://mcp-orchestrator:9001', help='WebSocket URL')
@click.option('--interactive', '-i', is_flag=True, help='Interactive mode')
@click.option('--duration', '-d', default=0, type=int, help='Run for N seconds (0=forever)')
async def main(url: str, interactive: bool, duration: int):
    """MCP WebSocket Test Client"""
    client = MCPWebSocketClient(url)

    if not await client.connect():
        sys.exit(1)

    try:
        if interactive:
            # Interactive mode
            print(f"\n{Fore.CYAN}Interactive mode. Commands:")
            print("  p - Send ping")
            print("  a - Request agents list")
            print("  t - Request token usage")
            print("  m - Request memory/communications")
            print("  h - Request system health")
            print("  s - Show statistics")
            print("  q - Quit")
            print()

            # Start listening in background
            listen_task = asyncio.create_task(client.listen())

            while True:
                # Non-blocking input check
                await asyncio.sleep(0.1)

                # Check if listen task has failed
                if listen_task.done():
                    break

                # Simple command handling (would need proper async input in production)
                print(f"{Fore.GREEN}> ", end='', flush=True)
                try:
                    cmd = await asyncio.wait_for(
                        asyncio.get_event_loop().run_in_executor(None, input),
                        timeout=0.1
                    )

                    if cmd == 'p':
                        await client.ping()
                    elif cmd == 'a':
                        await client.request_tool('agents/list')
                    elif cmd == 't':
                        await client.request_tool('analysis/token-usage')
                    elif cmd == 'm':
                        await client.request_tool('memory/query')
                    elif cmd == 'h':
                        await client.request_tool('system/health')
                    elif cmd == 's':
                        client.display_stats()
                    elif cmd == 'q':
                        break

                except asyncio.TimeoutError:
                    pass
        else:
            # Non-interactive mode
            if duration > 0:
                print(f"{Fore.BLUE}Running for {duration} seconds...")
                await asyncio.wait_for(client.listen(), timeout=duration)
            else:
                print(f"{Fore.BLUE}Running indefinitely (Ctrl+C to stop)...")
                await client.listen()

    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}Interrupted by user")
    except Exception as e:
        print(f"{Fore.RED}Error: {e}")
    finally:
        client.display_stats()
        await client.close()

if __name__ == '__main__':
    asyncio.run(main())