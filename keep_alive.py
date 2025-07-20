#!/usr/bin/env python3
"""
Keep-alive script for Blender MCP Server.
This script starts the MCP server and keeps Blender running.
"""

import bpy
import time
import signal
import sys

def signal_handler(sig, frame):
    """Handle shutdown gracefully"""
    print("\nShutting down Blender MCP Server...")
    if hasattr(bpy.types, 'blendermcp_server'):
        bpy.ops.blendermcp.stop_server()
    sys.exit(0)

# Register signal handler
signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)

# Enable the addon and start the MCP server
print("Enabling Blender MCP Server addon...")
bpy.ops.preferences.addon_enable(module="blender_mcp_server")

print("Starting Blender MCP Server...")
bpy.ops.blendermcp.start_server()

# Keep the script running
print("Blender MCP Server is running. Press Ctrl+C to stop.")
try:
    while True:
        time.sleep(1)
except KeyboardInterrupt:
    signal_handler(None, None)