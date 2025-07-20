#!/usr/bin/env python3
"""Test connection to Blender MCP server running in the host system."""

import socket
import json
import time
import sys

def test_blender_connection(host='host.docker.internal', port=9876):
    """Test connection to Blender MCP server."""
    print(f"Testing connection to Blender MCP server at {host}:{port}")
    
    try:
        # Create a socket connection
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)  # 5 second timeout
        
        # Try to connect
        print("Attempting to connect...")
        sock.connect((host, port))
        print("✅ Successfully connected to Blender MCP server!")
        
        # Try sending a simple command
        command = {
            "type": "get_scene_info"
        }
        
        print(f"Sending command: {command}")
        sock.send(json.dumps(command).encode('utf-8') + b'\n')
        
        # Wait for response
        response = sock.recv(4096).decode('utf-8')
        print(f"Received response: {response}")
        
        sock.close()
        return True
        
    except socket.timeout:
        print("❌ Connection timed out. Make sure Blender is running with the MCP addon enabled.")
        return False
    except ConnectionRefusedError:
        print("❌ Connection refused. Make sure:")
        print("   1. Blender is running on the host")
        print("   2. The BlenderMCP addon is installed and enabled")
        print("   3. You clicked 'Connect to Claude' in the BlenderMCP panel")
        return False
    except Exception as e:
        print(f"❌ Error: {type(e).__name__}: {e}")
        return False

if __name__ == "__main__":
    # Test connection to host
    print("=== Testing Blender MCP Connection ===")
    print("Note: This test assumes Blender is running on the host system")
    print("      with the BlenderMCP addon active and connected.\n")
    
    # Try different host configurations
    hosts_to_try = [
        ('host.docker.internal', 9876),  # Docker Desktop on Mac/Windows
        ('172.17.0.1', 9876),            # Default Docker bridge on Linux
        ('localhost', 9876),              # Direct localhost (unlikely to work from container)
    ]
    
    success = False
    for host, port in hosts_to_try:
        if test_blender_connection(host, port):
            success = True
            print(f"\n✅ Successfully connected using {host}:{port}")
            break
        print()
    
    if not success:
        print("\n❌ Could not connect to Blender MCP server.")
        print("\nTroubleshooting steps:")
        print("1. Ensure Blender is running on your host system")
        print("2. Install the addon.py file in Blender (Edit > Preferences > Add-ons)")
        print("3. Enable 'Interface: Blender MCP' addon")
        print("4. In the 3D View sidebar (press N), find the 'BlenderMCP' tab")
        print("5. Click 'Connect to Claude'")
        print("6. The server should be listening on port 9876")
        
    sys.exit(0 if success else 1)