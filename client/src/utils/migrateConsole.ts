/**
 * Utility script to help migrate console.* calls to gatedConsole
 * This provides helper functions and a guide for refactoring
 */

import { DebugCategory } from './console';

/**
 * Mapping of file patterns to suggested debug categories
 */
export const FILE_CATEGORY_MAP: Record<string, DebugCategory> = {
  'voice': DebugCategory.VOICE,
  'audio': DebugCategory.VOICE,
  'websocket': DebugCategory.WEBSOCKET,
  'socket': DebugCategory.WEBSOCKET,
  'auth': DebugCategory.AUTH,
  'login': DebugCategory.AUTH,
  'render': DebugCategory.RENDERING,
  'three': DebugCategory.RENDERING,
  '3d': DebugCategory.RENDERING,
  'performance': DebugCategory.PERFORMANCE,
  'perf': DebugCategory.PERFORMANCE,
  'data': DebugCategory.DATA,
  'binary': DebugCategory.DATA,
};

/**
 * Determine the appropriate debug category based on file path or content
 */
export function suggestCategory(filePath: string, lineContent?: string): DebugCategory {
  const lowerPath = filePath.toLowerCase();
  const lowerContent = lineContent?.toLowerCase() || '';
  
  // Check file path patterns
  for (const [pattern, category] of Object.entries(FILE_CATEGORY_MAP)) {
    if (lowerPath.includes(pattern) || lowerContent.includes(pattern)) {
      return category;
    }
  }
  
  // Check for error-related content
  if (lowerContent.includes('error') || lowerContent.includes('catch')) {
    return DebugCategory.ERROR;
  }
  
  return DebugCategory.GENERAL;
}

/**
 * Migration patterns for common console usage
 */
export const MIGRATION_PATTERNS = [
  {
    pattern: /console\.log\(/g,
    replacement: (category: DebugCategory) => 
      category === DebugCategory.GENERAL 
        ? 'gatedConsole.log(' 
        : `gatedConsole.${getCategoryMethod(category)}.log(`,
  },
  {
    pattern: /console\.error\(/g,
    replacement: (category: DebugCategory) => 
      category === DebugCategory.GENERAL 
        ? 'gatedConsole.error(' 
        : `gatedConsole.${getCategoryMethod(category)}.error(`,
  },
  {
    pattern: /console\.warn\(/g,
    replacement: (category: DebugCategory) => 
      category === DebugCategory.GENERAL 
        ? 'gatedConsole.warn(' 
        : `gatedConsole.${getCategoryMethod(category)}.warn(`,
  },
  {
    pattern: /console\.info\(/g,
    replacement: (category: DebugCategory) => 
      category === DebugCategory.GENERAL 
        ? 'gatedConsole.info(' 
        : `gatedConsole.${getCategoryMethod(category)}.log(`,
  },
  {
    pattern: /console\.debug\(/g,
    replacement: (category: DebugCategory) => 
      category === DebugCategory.GENERAL 
        ? 'gatedConsole.debug(' 
        : `gatedConsole.${getCategoryMethod(category)}.log(`,
  },
];

/**
 * Get the category method name for gatedConsole
 */
function getCategoryMethod(category: DebugCategory): string {
  switch (category) {
    case DebugCategory.VOICE:
      return 'voice';
    case DebugCategory.WEBSOCKET:
      return 'websocket';
    case DebugCategory.PERFORMANCE:
      return 'perf';
    case DebugCategory.DATA:
      return 'data';
    default:
      return '';
  }
}

/**
 * Generate import statement for a file
 */
export function generateImport(existingImports: string, fromPath: string): string {
  const relativeImport = calculateRelativeImport(fromPath, 'src/utils/console');
  return `import { gatedConsole } from '${relativeImport}';`;
}

/**
 * Calculate relative import path
 */
function calculateRelativeImport(from: string, to: string): string {
  // Simplified implementation - in real usage, use path module
  const fromParts = from.split('/');
  const toParts = to.split('/');
  
  // Remove filename from 'from' path
  fromParts.pop();
  
  // Find common base
  let commonIndex = 0;
  while (commonIndex < fromParts.length && 
         commonIndex < toParts.length && 
         fromParts[commonIndex] === toParts[commonIndex]) {
    commonIndex++;
  }
  
  // Build relative path
  const upCount = fromParts.length - commonIndex;
  const ups = '../'.repeat(upCount);
  const remaining = toParts.slice(commonIndex).join('/');
  
  return ups + remaining;
}

/**
 * Migration guide text
 */
export const MIGRATION_GUIDE = `
# Console to GatedConsole Migration Guide

## Quick Reference

### Basic Replacements:
- console.log(...) → gatedConsole.log(...)
- console.error(...) → gatedConsole.error(...)
- console.warn(...) → gatedConsole.warn(...)

### Category-specific Replacements:
- Voice/Audio: console.log(...) → gatedConsole.voice.log(...)
- WebSocket: console.log(...) → gatedConsole.websocket.log(...)
- Performance: console.log(...) → gatedConsole.perf.log(...)
- Data: console.log(...) → gatedConsole.data.log(...)

### Import Statement:
import { gatedConsole } from '../utils/console';

## Step-by-Step Migration:

1. Add the import statement at the top of the file
2. Identify the appropriate category based on the file/feature
3. Replace console.* calls with gatedConsole.* equivalents
4. Test that debug output works with debug enabled
5. Verify output is hidden when debug is disabled

## Examples:

### Before:
\`\`\`typescript
console.log('WebSocket connected');
console.error('Connection failed:', error);
\`\`\`

### After:
\`\`\`typescript
import { gatedConsole } from '../utils/console';

gatedConsole.websocket.log('WebSocket connected');
gatedConsole.websocket.error('Connection failed:', error);
\`\`\`

## Advanced Usage:

### Force output (bypasses debug settings):
\`\`\`typescript
gatedConsole.log({ force: true }, 'Critical error - always show');
\`\`\`

### Custom namespace with logger:
\`\`\`typescript
gatedConsole.log({ namespace: 'MyComponent' }, 'Component initialized');
\`\`\`

## Testing Migration:

1. Enable debug: window.debugControl.enable()
2. Enable specific category: window.debugControl.enableCategory('voice')
3. Check output appears correctly
4. Disable debug: window.debugControl.disable()
5. Verify output is suppressed
`;

/**
 * Create a migration summary for a file
 */
export function createMigrationSummary(
  filePath: string,
  consoleCallCount: number,
  suggestedCategory: DebugCategory
): string {
  return `
File: ${filePath}
Console calls found: ${consoleCallCount}
Suggested category: ${suggestedCategory}
Import needed: ${generateImport('', filePath)}

Migration pattern:
- Replace console.* with gatedConsole.${getCategoryMethod(suggestedCategory) || ''}.*
`;
}