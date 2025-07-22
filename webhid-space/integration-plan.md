# WebHID to Three.js Integration Plan

## Overview
This document outlines the technical architecture for integrating SpacePilot 6DOF controller with VisionFlow's Three.js visualization system using WebHID API.

## Current State Analysis

### WebHID Driver (space-driver.js)
- **Vendor ID**: 0x046d (Logitech)
- **Supported Devices**: SpaceBall 5000, SpaceExplorer, and similar
- **Input Reports**:
  - Report ID 1: Translation (x, y, z) as Int16Array
  - Report ID 2: Rotation (rx, ry, rz) as Int16Array  
  - Report ID 3: Buttons (16 buttons) as Uint16Array
- **Events**: CustomEvents for translate, rotate, and buttons
- **Architecture**: EventTarget-based, singleton pattern

### VisionFlow Three.js Components
- **Camera System**: PerspectiveCamera with OrbitControls
- **Scene Graph**: HologramVisualization with nested group transforms
- **State Management**: useSettingsStore for configuration
- **Rendering**: React Three Fiber (@react-three/fiber)

## Integration Architecture

### 1. Core Integration Module
Create `/workspace/ext/client/src/features/visualisation/controls/SpacePilotController.ts`

```typescript
interface SpacePilotConfig {
  translationSensitivity: number;
  rotationSensitivity: number;
  deadzone: number;
  smoothing: number;
  mode: 'camera' | 'object' | 'navigation';
}

class SpacePilotController {
  private camera: THREE.Camera;
  private controls: OrbitControls;
  private config: SpacePilotConfig;
  private smoothedValues: SmoothingBuffer;
  
  // Event handlers for WebHID
  handleTranslation(detail: {x: number, y: number, z: number}): void;
  handleRotation(detail: {rx: number, ry: number, rz: number}): void;
  handleButtons(detail: {buttons: string[]}): void;
}
```

### 2. Input Processing Pipeline

#### A. Raw Input Normalization
- Convert Int16 values (-32768 to 32767) to normalized floats (-1 to 1)
- Apply deadzone filtering to eliminate drift
- Scale by user-configurable sensitivity

#### B. Smoothing and Filtering
- Implement exponential moving average for smooth motion
- Optional low-pass filter for jittery input
- Frame-rate independent processing

#### C. Coordinate System Mapping
```
SpacePilot → Three.js
X (left/right) → Camera X (strafe)
Y (up/down) → Camera Y (elevation)
Z (forward/back) → Camera Z (dolly)
RX (pitch) → Camera rotation X
RY (yaw) → Camera rotation Y  
RZ (roll) → Camera rotation Z (optional)
```

### 3. Control Modes

#### Camera Control Mode
- Direct camera manipulation
- Smooth transitions with damping
- Respect OrbitControls constraints
- Integration with existing mouse controls

#### Object Control Mode
- Manipulate selected 3D objects
- Transform in world or local space
- Support for multi-object selection
- Undo/redo integration

#### Navigation Mode
- Fly-through navigation
- Speed ramping based on input magnitude
- Collision detection (optional)
- Waypoint recording

### 4. React Integration

Create custom hook: `useSpacePilot.ts`
```typescript
export function useSpacePilot(options?: SpacePilotOptions) {
  const { camera, scene, gl } = useThree();
  const [isConnected, setIsConnected] = useState(false);
  const [currentMode, setCurrentMode] = useState<ControlMode>('camera');
  
  useEffect(() => {
    // Initialize SpaceDriver connection
    // Set up event listeners
    // Return cleanup function
  }, []);
  
  return {
    isConnected,
    currentMode,
    setMode,
    calibrate,
    resetView
  };
}
```

### 5. UI Integration

#### Status Indicator Component
```tsx
<SpacePilotStatus 
  connected={isConnected}
  mode={currentMode}
  sensitivity={sensitivity}
/>
```

#### Settings Panel Integration
- Add SpacePilot section to settings
- Sensitivity sliders for each axis
- Deadzone configuration
- Mode selection
- Button mapping configuration

### 6. Event Flow Architecture

```
SpaceDriver.js → CustomEvent → SpacePilotController → Three.js Scene
     ↓                              ↓
  WebHID API                   Settings Store
     ↓                              ↓
  USB Device                   Persistence
```

### 7. Performance Considerations

- Use requestAnimationFrame for smooth updates
- Batch transformations to minimize reflows
- Implement level-of-detail for complex scenes
- Optional WebWorker for input processing

### 8. Error Handling

- Graceful fallback when device disconnected
- Clear user feedback for connection status
- Automatic reconnection attempts
- Debug mode for troubleshooting

## Implementation Phases

### Phase 1: Basic Integration (MVP)
1. Create SpacePilotController class
2. Implement camera control mode
3. Add connection status UI
4. Basic sensitivity settings

### Phase 2: Enhanced Features
1. Object manipulation mode
2. Advanced smoothing algorithms
3. Button mapping system
4. Settings persistence

### Phase 3: Advanced Integration
1. Navigation mode with physics
2. Gesture recognition
3. Macro recording
4. Multi-device support

## Technical Dependencies

- WebHID API (already implemented in space-driver.js)
- Three.js (already in project)
- @react-three/fiber (already in project)
- No additional dependencies required

## Testing Strategy

1. Unit tests for input normalization
2. Integration tests with mock WebHID
3. E2E tests with real device (manual)
4. Performance benchmarks

## Security Considerations

- Validate all input ranges
- Sanitize button commands
- Rate limiting for rapid inputs
- Permission handling for WebHID

## Future Enhancements

1. Support for additional 3D input devices
2. Haptic feedback (if supported)
3. LED control for device feedback
4. Cloud-based settings sync
5. Mobile companion app support