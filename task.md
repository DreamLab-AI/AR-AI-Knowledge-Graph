connect the mock data to the visualization component, BotsVisualizationEnhanced.tsx, which will render the bots and their roles in a visually appealing way.

The visualization component, BotsVisualizationEnhanced.tsx, further clarifies these types by grouping them into two logical categories: Primary Agent Roles (generally rendered in shades of green) and Meta/Coordinating Roles (rendered in shades of gold and yellow).
Here is the complete list, categorized as the system implements it:
Meta & Coordinating Roles (Gold/Yellow Palette)
These agents are typically responsible for orchestration, analysis, and high-level strategy within the bots.
coordinator: The central orchestrator or manager of a task or sub-bots.
analyst: Responsible for analyzing data, performance, or results.
architect: Designs the overall structure or approach for a task.
optimizer: Focuses on improving the efficiency or output of other agents.
monitor: Observes the state of the bots and reports on its health and progress.
Primary Agent Roles (Green Palette)
These agents are the primary "workers" that perform the core tasks assigned to the bots.
coder: Writes or generates code.
tester: Tests and validates the output of other agents, particularly coders.
researcher: Gathers information and performs research tasks.
reviewer: Reviews code or content for quality, correctness, and adherence to standards.
documenter: Writes documentation for the work being done.
specialist: A general-purpose agent with a specific, specialized skill set not covered by other types.

also, find all of the examples of hard coded values in the client code and connect them to the control panel, allowing users to modify these values dynamically. This will enhance the flexibility and usability of the visualization component.

here is a list to get you started:

Here is a simple list of the most significant hardcoded elements and their file locations.
client/src/app/MainLayout.tsx
3D Scene & Camera Setup:
Camera position, fov, near, and far clipping planes.
Scene backgroundColor.
ambientLight and directionalLight intensity values.
client/src/features/bots/components/BotsVisualization.tsx
Node Visuals (VisionFlow Graph):
The entire color palette for different agent types (e.g., coder: '#2ECC71').
The base size for agent nodes (0.5).
The multiplier for how much workload affects node size (* 1.5).
The thresholds for determining node health color (e.g., health > 80 is green).
Edge Visuals (VisionFlow Graph):
The default opacity of communication links (0.2).
The size and color of the animated particles on active edges (size: 0.4, color: "#FFD700").
Animation/Behavior Logic:
The time threshold for an edge to be considered "active" (5000ms).
The time threshold for cleaning up stale edges (30000ms).
client/src/features/graph/components/GraphManager.tsx
Node Visuals (Logseq Graph):
The color palette for different node types (e.g., folder: '#FFD700').
The initial colors for the hologram material (baseColor: '#0066ff').
Node Sizing Logic:
The hardcoded multipliers that determine a node's size based on its connection count (* 0.3) and its "importance" (getTypeImportance function).
client/src/app/Quest3ARLayout.tsx
AR Rendering Parameters:
The default AR update rate (30 fps).
The specific update rate for Quest 3 (72 fps).
The maximum render distance for nodes in AR (100).
UI Element Styling:
The position, colors, and opacity of the AR status indicators and debug info panels.
client/src/features/visualisation/components/SpacePilotSimpleIntegration.tsx
Controller Configuration:
The entire config object, which includes translationSpeed, rotationSpeed, deadzone, smoothing, and axis inversion settings.
client/src/features/visualisation/renderers/HologramManager.tsx
Default Hologram Visuals:
Default colors, opacities, rotation speeds, and geometry segments for the HologramRing and HologramSphere components.
client/src/features/graph/components/FlowingEdges.tsx
Default Edge Material Properties:
The default edge color (#56b6c2), opacity (0.6), and linewidth (2).

also, reconnect and improve the hologram rendering system. Currently the UI switch turn on the node animation which is incorrect and the holoograms are not integrated into the code at all.
