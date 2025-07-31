# Debug Control Panel

A comprehensive UI component for managing debug settings and logging categories in the application.

## Features

- **Master Debug Toggle**: Enable/disable all debug functionality
- **Category-specific Controls**: Fine-grained control over debug categories:
  - General: General debug messages and logs
  - Voice: Voice recognition and WebRTC debugging
  - WebSocket: WebSocket connection and message debugging
  - Performance: Performance metrics and timing logs
  - Data: Data flow and state management debugging
  - 3D Rendering: 3D scene and rendering debugging
  - Authentication: Authentication and authorization debugging
  - Errors: Error messages and stack traces
- **Special Debug Modes**:
  - Data Debug: Log all data flow and state changes
  - Performance Debug: Enable performance profiling and timing logs
- **Debug Presets**: Quick configurations for common scenarios:
  - Off: Disable all debugging
  - Minimal: Only errors enabled
  - Standard: Errors and general debugging
  - Verbose: All categories and features enabled

## Usage

### Keyboard Shortcut

Press `Ctrl+Shift+D` to toggle the Debug Control Panel.

### Integration

The component is automatically included in the main App component and is available globally throughout the application.

### Programmatic Control

You can also control debug settings programmatically using the `debugControl` API:

```typescript
import { debugControl, DebugCategory } from '@/utils/console';

// Enable/disable main debug
debugControl.enable();
debugControl.disable();

// Control specific categories
debugControl.enableCategory(DebugCategory.VOICE);
debugControl.disableCategory(DebugCategory.VOICE);

// Use presets
debugControl.presets.verbose();
debugControl.presets.minimal();

// Enable special modes
debugControl.enableData();
debugControl.enablePerformance();
```

### Using Gated Console

Once debug settings are configured, use the gated console for automatic filtering:

```typescript
import { gatedConsole } from '@/utils/console';

// These will only log if debugging is enabled
gatedConsole.log('General debug message');
gatedConsole.voice.log('Voice-specific debug');
gatedConsole.websocket.log('WebSocket debug');
gatedConsole.perf.log('Performance metric');
gatedConsole.data.log('Data flow debug');
```

## State Persistence

All debug settings are persisted to localStorage and will be restored on page reload. Settings are also synchronized across browser tabs.

## Development

The component uses:
- Radix UI Dialog for the modal
- Radix UI Switch for toggle controls
- Design system components for consistent styling
- Keyboard shortcuts hook for hotkey support

The debug state is managed through:
- `debugState` - Main debug toggle and special modes
- `categoryDebugState` - Individual category toggles
- `debugControl` - Unified API for all debug controls