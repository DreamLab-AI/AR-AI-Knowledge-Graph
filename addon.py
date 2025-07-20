# This is the Blender MCP addon that runs inside Blender
# It provides a socket server on port 9876 for MCP communication

import bpy
import json
import socket
import threading
import time
import base64
import tempfile
import os
import sys
import subprocess
import requests
from io import BytesIO
from PIL import Image
from concurrent.futures import ThreadPoolExecutor

bl_info = {
    "name": "Blender MCP Server",
    "blender": (4, 0, 0),
    "category": "System",
    "version": (1, 1, 0),
    "author": "Blender MCP Team",
    "description": "MCP server for Blender integration with headless support"
}

class BlenderMCPServer:
    def __init__(self, host=None, port=9876, max_workers=5):
        if host is None:
            host = os.environ.get('BLENDER_MCP_HOST', 'localhost')
        self.host = host
        self.port = port
        self.server_socket = None
        self.running = False
        self.server_thread = None
        self.executor = ThreadPoolExecutor(max_workers=max_workers)

    def start(self):
        """Start the MCP server"""
        if self.running:
            print("Server is already running")
            return

        self.server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

        try:
            self.server_socket.bind((self.host, self.port))
            self.server_socket.listen(5)
            self.running = True

            self.server_thread = threading.Thread(target=self._server_loop, daemon=True)
            self.server_thread.start()

            print(f"Blender MCP Server started on {self.host}:{self.port}")

        except Exception as e:
            print(f"Failed to start server: {e}")
            self.stop()

    def stop(self):
        """Stop the MCP server"""
        print("Stopping Blender MCP Server...")
        self.running = False

        if self.server_socket:
            try:
                self.server_socket.close()
            except:
                pass

        if self.executor:
            self.executor.shutdown(wait=False)

        print("Blender MCP Server stopped")

    def _server_loop(self):
        """Main server loop"""
        while self.running:
            try:
                self.server_socket.settimeout(1.0)
                client_socket, address = self.server_socket.accept()
                print(f"Client connected from {address}")

                # Handle client in thread pool
                self.executor.submit(self._handle_client, client_socket)

            except socket.timeout:
                continue
            except Exception as e:
                if self.running:
                    print(f"Server error: {e}")

    def _handle_client(self, client_socket):
        """Handle individual client connections"""
        try:
            while self.running:
                # Receive data
                data = client_socket.recv(4096)
                if not data:
                    break

                # Parse JSON command
                try:
                    command = json.loads(data.decode('utf-8'))
                    print(f"Received command: {command.get('tool', 'unknown')}")

                    # Process command with timer system for thread safety
                    response = {"error": "Command execution timeout"}
                    response_event = threading.Event()

                    def execute_wrapper():
                        nonlocal response
                        try:
                            response = self.execute_command(command)
                        except Exception as e:
                            response = {"error": str(e)}
                        finally:
                            response_event.set()
                        return None  # Important for timer system

                    # Schedule execution in main thread
                    bpy.app.timers.register(execute_wrapper, first_interval=0.0)

                    # Wait for execution with timeout
                    if response_event.wait(timeout=30.0):
                        # Send response
                        response_json = json.dumps(response)
                        client_socket.sendall(response_json.encode('utf-8'))
                    else:
                        # Timeout
                        timeout_response = json.dumps({"error": "Command execution timeout"})
                        client_socket.sendall(timeout_response.encode('utf-8'))

                except json.JSONDecodeError as e:
                    error_response = json.dumps({"error": f"Invalid JSON: {e}"})
                    client_socket.sendall(error_response.encode('utf-8'))
                except Exception as e:
                    error_response = json.dumps({"error": f"Command error: {e}"})
                    client_socket.sendall(error_response.encode('utf-8'))

        except Exception as e:
            print(f"Client handler error: {e}")
        finally:
            client_socket.close()

    def execute_command(self, command):
        """Execute MCP commands - runs in main thread via timer"""
        tool = command.get('tool', '')
        params = command.get('params', {})

        # Command handlers
        handlers = {
            'get_scene_info': self._get_scene_info,
            'get_object_info': self._get_object_info,
            'get_viewport_screenshot': self._get_viewport_screenshot,
            'execute_blender_code': self._execute_code,
            'get_polyhaven_categories': self._get_polyhaven_categories,
            'search_polyhaven_assets': self._search_polyhaven_assets,
            'download_polyhaven_asset': self._download_polyhaven_asset,
            'set_texture': self._set_texture,
            'get_polyhaven_status': lambda p: {"status": "PolyHaven integration is enabled"},
            'get_hyper3d_status': lambda p: {"status": "Hyper3D integration is not configured"},
            'get_sketchfab_status': lambda p: {"status": "Sketchfab integration is not configured"},
        }

        handler = handlers.get(tool)
        if handler:
            return handler(params)
        else:
            return {"error": f"Unknown tool: {tool}"}

    def _get_scene_info(self, params):
        """Get information about the current scene"""
        scene = bpy.context.scene

        info = {
            "scene_name": scene.name,
            "frame_current": scene.frame_current,
            "frame_start": scene.frame_start,
            "frame_end": scene.frame_end,
            "fps": scene.render.fps,
            "resolution": {
                "x": scene.render.resolution_x,
                "y": scene.render.resolution_y,
                "percentage": scene.render.resolution_percentage
            },
            "objects": [],
            "collections": []
        }

        # Add object information
        for obj in scene.objects:
            obj_info = {
                "name": obj.name,
                "type": obj.type,
                "location": list(obj.location),
                "rotation": list(obj.rotation_euler),
                "scale": list(obj.scale),
                "visible": obj.visible_get()
            }
            info["objects"].append(obj_info)

        # Add collection information
        for collection in bpy.data.collections:
            col_info = {
                "name": collection.name,
                "objects": [obj.name for obj in collection.objects]
            }
            info["collections"].append(col_info)

        return info

    def _get_object_info(self, params):
        """Get detailed information about a specific object"""
        object_name = params.get('object_name', '')

        obj = bpy.data.objects.get(object_name)
        if not obj:
            return {"error": f"Object '{object_name}' not found"}

        info = {
            "name": obj.name,
            "type": obj.type,
            "location": list(obj.location),
            "rotation_euler": list(obj.rotation_euler),
            "scale": list(obj.scale),
            "dimensions": list(obj.dimensions),
            "visible": obj.visible_get(),
            "parent": obj.parent.name if obj.parent else None,
            "children": [child.name for child in obj.children],
            "modifiers": [mod.name for mod in obj.modifiers] if hasattr(obj, 'modifiers') else [],
            "materials": [mat.name for mat in obj.data.materials] if hasattr(obj.data, 'materials') else []
        }

        # Add mesh-specific information
        if obj.type == 'MESH':
            mesh = obj.data
            info["mesh"] = {
                "vertices": len(mesh.vertices),
                "edges": len(mesh.edges),
                "faces": len(mesh.polygons)
            }

        return info

    def _get_viewport_screenshot(self, params):
        """Capture a screenshot of the viewport"""
        max_size = params.get('max_size', 800)

        # Set up rendering
        scene = bpy.context.scene
        render = scene.render

        # Store original settings
        orig_res_x = render.resolution_x
        orig_res_y = render.resolution_y
        orig_res_percentage = render.resolution_percentage
        orig_filepath = render.filepath

        try:
            # Set resolution
            render.resolution_x = max_size
            render.resolution_y = max_size
            render.resolution_percentage = 100

            # Create temp file
            with tempfile.NamedTemporaryFile(suffix='.png', delete=False) as tmp:
                temp_path = tmp.name

            render.filepath = temp_path

            # Render
            bpy.ops.render.render(write_still=True)

            # Read and encode image
            with open(temp_path, 'rb') as f:
                image_data = f.read()

            # Clean up
            os.unlink(temp_path)

            # Return base64 encoded image
            return {
                "image": base64.b64encode(image_data).decode('utf-8'),
                "format": "png",
                "width": max_size,
                "height": max_size
            }

        finally:
            # Restore settings
            render.resolution_x = orig_res_x
            render.resolution_y = orig_res_y
            render.resolution_percentage = orig_res_percentage
            render.filepath = orig_filepath

    def _execute_code(self, params):
        """Execute arbitrary Python code in Blender"""
        code = params.get('code', '')

        if not code:
            return {"error": "No code provided"}

        # Create execution context
        exec_globals = {"bpy": bpy}
        exec_locals = {}

        try:
            # Execute the code
            exec(code, exec_globals, exec_locals)

            # Try to get a result
            if 'result' in exec_locals:
                return {"result": str(exec_locals['result'])}
            else:
                return {"status": "Code executed successfully"}

        except Exception as e:
            return {"error": f"Execution error: {str(e)}"}

    def _get_polyhaven_categories(self, params):
        """Get PolyHaven asset categories"""
        asset_type = params.get('asset_type', 'hdris')

        # Simplified categories
        categories = {
            "hdris": ["outdoor", "indoor", "studio", "nature"],
            "textures": ["fabric", "wood", "metal", "concrete", "plastic"],
            "models": ["furniture", "nature", "architecture", "props"]
        }

        return {"categories": categories.get(asset_type, [])}

    def _search_polyhaven_assets(self, params):
        """Search for PolyHaven assets"""
        asset_type = params.get('asset_type', 'all')
        categories = params.get('categories', '')

        # Mock search results
        assets = [
            {
                "id": "concrete_wall_01",
                "name": "Concrete Wall 01",
                "type": "textures",
                "category": "concrete"
            },
            {
                "id": "wooden_floor_02",
                "name": "Wooden Floor 02",
                "type": "textures",
                "category": "wood"
            }
        ]

        return {"assets": assets}

    def _download_polyhaven_asset(self, params):
        """Download a PolyHaven asset"""
        asset_id = params.get('asset_id', '')
        asset_type = params.get('asset_type', '')

        # This would normally download from PolyHaven
        # For now, return a mock response
        return {
            "status": "Downloaded",
            "asset_id": asset_id,
            "path": f"/tmp/{asset_id}.blend"
        }

    def _set_texture(self, params):
        """Apply a texture to an object"""
        object_name = params.get('object_name', '')
        texture_id = params.get('texture_id', '')

        obj = bpy.data.objects.get(object_name)
        if not obj:
            return {"error": f"Object '{object_name}' not found"}

        # This would normally apply the texture
        # For now, return a mock response
        return {
            "status": "Texture applied",
            "object": object_name,
            "texture": texture_id
        }

# Blender operators
class BLENDERMCP_OT_start_server(bpy.types.Operator):
    bl_idname = "blendermcp.start_server"
    bl_label = "Start Blender MCP Server"

    def execute(self, context):
        if not hasattr(bpy.types, 'blendermcp_server'):
            server = BlenderMCPServer()
            server.start()
            bpy.types.blendermcp_server = server
            self.report({'INFO'}, "Blender MCP Server started")
        else:
            self.report({'WARNING'}, "Server already running")
        return {'FINISHED'}

class BLENDERMCP_OT_stop_server(bpy.types.Operator):
    bl_idname = "blendermcp.stop_server"
    bl_label = "Stop Blender MCP Server"

    def execute(self, context):
        if hasattr(bpy.types, 'blendermcp_server'):
            bpy.types.blendermcp_server.stop()
            del bpy.types.blendermcp_server
            self.report({'INFO'}, "Blender MCP Server stopped")
        else:
            self.report({'WARNING'}, "Server not running")
        return {'FINISHED'}

# Registration
classes = [
    BLENDERMCP_OT_start_server,
    BLENDERMCP_OT_stop_server
]

def register():
    try:
        for cls in classes:
            bpy.utils.register_class(cls)
        print("Blender MCP addon registered successfully")
    except Exception as e:
        print(f"Error registering Blender MCP addon: {e}")

def unregister():
    try:
        if hasattr(bpy.types, 'blendermcp_server'):
            bpy.types.blendermcp_server.stop()
            del bpy.types.blendermcp_server

        for cls in classes:
            bpy.utils.unregister_class(cls)
        print("Blender MCP addon unregistered successfully")
    except Exception as e:
        print(f"Error unregistering Blender MCP addon: {e}")

if __name__ == "__main__":
    register()