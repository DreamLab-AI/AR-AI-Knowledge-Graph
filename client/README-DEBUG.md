# Debug System Documentation

## Overview

The debug system provides comprehensive control over console output in the client application. It allows fine-grained control over what debug information is displayed, making it easier to debug specific features without overwhelming console output.

## Quick Start

### Enable Debug Mode

1. **Via Environment Variables** (Recommended for deployment):
   ```bash
   VITE_DEBUG=true npm start
   VITE_DEBUG_PRESET=verbose npm start
   ```

2. **Via Browser Console**:
   ```javascript
   window.debugControl.enable()
   window.debugControl.presets.verbose()
   ```

3. **Via UI** (Development only):
   - Press `Ctrl+Shift+D` to open the Debug Control Panel
   - Toggle categories and settings as needed

## Architecture

### Core Components

1. **debugState** (`/src/utils/debugState.ts`)
   - Manages global debug enabled/disabled state
   - Handles data and performance debug flags
   - Persists settings to localStorage

2. **gatedConsole** (`/src/utils/console.ts`)
   - Drop-in replacement for console methods
   - Automatically gates output based on debug settings
   - Provides category-specific logging methods

3. **DebugControlPanel** (`/src/components/DebugControlPanel.tsx`)
   - UI for controlling debug settings
   - Real-time toggle of debug categories
   - Preset configurations

## Debug Categories

- **General**: Default category for uncategorized logs
- **Voice**: Voice interaction and audio services
- **WebSocket**: WebSocket connections and messages
- **Performance**: Performance monitoring and metrics
- **Data**: Data processing and binary protocols
- **3D Rendering**: Three.js and 3D visualization
- **Authentication**: Auth flows and security
- **Error**: Error messages (always shown in development)

## Usage Examples

### Basic Usage

```typescript
import { gatedConsole } from '@/utils/console';

// Simple replacement
gatedConsole.log('This will only show when debug is enabled');
gatedConsole.error('This error respects debug settings');
```

### Category-Specific Logging

```typescript
// Voice-related logging
gatedConsole.voice.log('Voice service connected');
gatedConsole.voice.error('Microphone access denied');

// WebSocket logging
gatedConsole.websocket.log('Connected to server');
gatedConsole.websocket.warn('Connection unstable');

// Performance logging
gatedConsole.perf.log('Render time:', performance.now());
```

### Advanced Options

```typescript
// Force output regardless of debug settings
gatedConsole.log({ force: true }, 'Critical system error');

// Use custom namespace with logger
gatedConsole.log({ namespace: 'AuthService' }, 'User authenticated');

// Specify category explicitly
gatedConsole.log({ category: DebugCategory.DATA }, 'Processing binary data');
```

## Configuration

### Environment Variables

```bash
# Enable/disable debug mode
VITE_DEBUG=true

# Use a preset configuration
VITE_DEBUG_PRESET=verbose  # Options: minimal, standard, verbose, off

# Enable specific categories (comma-separated)
VITE_DEBUG_CATEGORIES=voice,websocket,error

# Replace global console (development only)
VITE_DEBUG_REPLACE_CONSOLE=true
```

### Programmatic Configuration

```typescript
// Enable debug
window.debugControl.enable();

// Enable specific categories
window.debugControl.enableCategory('voice');
window.debugControl.enableCategory('websocket');

// Use presets
window.debugControl.presets.minimal();    // Errors only
window.debugControl.presets.standard();   // Errors + general
window.debugControl.presets.verbose();    // Everything
window.debugControl.presets.off();        // Nothing

// Enable all categories
window.debugControl.enableAllCategories();

// Check current state
window.debugControl.isEnabled();
window.debugControl.getEnabledCategories();
```

## Migration Guide

### Migrating from console.* to gatedConsole

1. **Add Import**:
   ```typescript
   import { gatedConsole } from '@/utils/console';
   ```

2. **Replace Console Calls**:
   ```typescript
   // Before
   console.log('Connected');
   console.error('Failed:', error);
   
   // After
   gatedConsole.log('Connected');
   gatedConsole.error('Failed:', error);
   ```

3. **Use Category-Specific Methods**:
   ```typescript
   // Before (in voice-related file)
   console.log('Voice connected');
   
   // After
   gatedConsole.voice.log('Voice connected');
   ```

### Bulk Migration Script

```bash
# Example sed command for basic replacement
sed -i 's/console\.log(/gatedConsole.log(/g' src/**/*.ts
sed -i 's/console\.error(/gatedConsole.error(/g' src/**/*.ts

# Don't forget to add imports!
```

## Best Practices

1. **Use Appropriate Categories**: Choose the most specific category for your logs
2. **Avoid Sensitive Data**: Never log passwords, tokens, or PII
3. **Use Error Level Appropriately**: Reserve `.error()` for actual errors
4. **Clean Up Debug Code**: Remove temporary debug logs before committing
5. **Document Debug Flags**: If adding new categories, document them

## Production Considerations

1. **Default Off**: Debug is disabled by default in production
2. **Performance**: Gated console has minimal overhead when disabled
3. **Security**: Don't expose sensitive data even in debug logs
4. **Storage**: Debug settings persist in localStorage

## Troubleshooting

### Debug Not Working?

1. Check if debug is enabled: `window.debugControl.isEnabled()`
2. Check category is enabled: `window.debugControl.getEnabledCategories()`
3. Verify import is correct: `import { gatedConsole } from '@/utils/console'`
4. Clear localStorage if settings are corrupted

### Too Much Output?

1. Disable specific categories
2. Use the "minimal" preset
3. Target specific namespaces

### Performance Impact?

- When disabled, gatedConsole has negligible overhead
- Consider disabling performance category if it's causing issues
- Use development-only debug code for heavy operations

## Future Enhancements

- [ ] Remote debug configuration
- [ ] Log export functionality  
- [ ] Integration with error reporting services
- [ ] Custom category creation UI
- [ ] Log filtering and search
- [ ] Network request logging category