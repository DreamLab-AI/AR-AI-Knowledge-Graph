#!/usr/bin/env node

/**
 * Script to help migrate console.* calls to gatedConsole
 * Usage: node scripts/migrate-console.js [options] <file-or-directory>
 */

const fs = require('fs');
const path = require('path');

// ANSI color codes
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
};

// Category mappings
const categoryPatterns = {
  voice: ['voice', 'audio', 'microphone', 'speaker'],
  websocket: ['websocket', 'socket', 'ws:', 'connection'],
  auth: ['auth', 'login', 'token', 'session'],
  performance: ['performance', 'perf', 'timing', 'metric'],
  data: ['data', 'binary', 'parse', 'serialize'],
  '3d': ['three', 'render', '3d', 'mesh', 'scene'],
};

// Parse command line arguments
const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const verbose = args.includes('--verbose');
const autoCategory = !args.includes('--no-auto-category');
const targetPath = args.find(arg => !arg.startsWith('--')) || './src';

// Statistics
let filesProcessed = 0;
let filesModified = 0;
let consoleCalls = 0;
let consoleCallsMigrated = 0;

function log(message, color = colors.reset) {
  console.log(`${color}${message}${colors.reset}`);
}

function detectCategory(filePath, lineContent) {
  const lowerPath = filePath.toLowerCase();
  const lowerContent = lineContent.toLowerCase();
  
  for (const [category, patterns] of Object.entries(categoryPatterns)) {
    for (const pattern of patterns) {
      if (lowerPath.includes(pattern) || lowerContent.includes(pattern)) {
        return category;
      }
    }
  }
  
  if (lowerContent.includes('error') || lowerContent.includes('catch')) {
    return 'error';
  }
  
  return null;
}

function generateImportPath(fromFile) {
  const fromDir = path.dirname(fromFile);
  const toFile = 'src/utils/console';
  
  // Calculate relative path
  const fromParts = fromDir.split(path.sep);
  const toParts = toFile.split('/');
  
  // Find where paths diverge
  let commonLength = 0;
  while (
    commonLength < fromParts.length && 
    commonLength < toParts.length && 
    fromParts[commonLength] === toParts[commonLength]
  ) {
    commonLength++;
  }
  
  // Build relative path
  const upCount = fromParts.length - commonLength;
  const ups = '../'.repeat(upCount);
  const down = toParts.slice(commonLength).join('/');
  
  return ups + down;
}

function processFile(filePath) {
  if (!filePath.match(/\.(ts|tsx|js|jsx)$/)) return;
  
  filesProcessed++;
  
  try {
    let content = fs.readFileSync(filePath, 'utf8');
    const originalContent = content;
    const lines = content.split('\n');
    
    let fileModified = false;
    let hasGatedConsoleImport = content.includes('from \'./utils/console\'') || 
                                content.includes('from \'../utils/console\'') ||
                                content.includes('from \'../../utils/console\'');
    
    // Process each line
    const modifiedLines = lines.map((line, index) => {
      const consoleMatch = line.match(/console\.(log|error|warn|info|debug)\(/);
      
      if (consoleMatch) {
        consoleCalls++;
        const method = consoleMatch[1];
        const category = autoCategory ? detectCategory(filePath, line) : null;
        
        if (verbose) {
          log(`  Line ${index + 1}: console.${method} → ${category ? `gatedConsole.${category}.${method}` : `gatedConsole.${method}`}`, colors.yellow);
        }
        
        let newLine = line;
        if (category && categoryPatterns[category]) {
          // Special handling for category shortcuts
          const categoryMethod = category === '3d' ? 'data' : category;
          newLine = line.replace(
            `console.${method}(`,
            `gatedConsole.${categoryMethod}.${method}(`
          );
        } else {
          newLine = line.replace(
            `console.${method}(`,
            `gatedConsole.${method}(`
          );
        }
        
        if (!dryRun) {
          fileModified = true;
          consoleCallsMigrated++;
          return newLine;
        }
      }
      
      return line;
    });
    
    // Add import if needed and file was modified
    if (fileModified && !hasGatedConsoleImport) {
      const importPath = generateImportPath(filePath);
      const importStatement = `import { gatedConsole } from '${importPath}';\n`;
      
      // Find where to insert import (after other imports)
      let insertIndex = 0;
      for (let i = 0; i < modifiedLines.length; i++) {
        if (modifiedLines[i].match(/^import/)) {
          insertIndex = i + 1;
        } else if (insertIndex > 0 && !modifiedLines[i].match(/^import/)) {
          break;
        }
      }
      
      modifiedLines.splice(insertIndex, 0, importStatement);
      
      if (verbose) {
        log(`  Added import: ${importStatement.trim()}`, colors.green);
      }
    }
    
    // Write file if modified
    if (fileModified && !dryRun) {
      content = modifiedLines.join('\n');
      fs.writeFileSync(filePath, content, 'utf8');
      filesModified++;
      log(`✓ ${filePath}`, colors.green);
    } else if (fileModified && dryRun) {
      log(`✓ ${filePath} (dry run - would be modified)`, colors.yellow);
    }
    
  } catch (error) {
    log(`✗ Error processing ${filePath}: ${error.message}`, colors.red);
  }
}

function processDirectory(dirPath) {
  const entries = fs.readdirSync(dirPath, { withFileTypes: true });
  
  for (const entry of entries) {
    const fullPath = path.join(dirPath, entry.name);
    
    // Skip node_modules and build directories
    if (entry.name === 'node_modules' || entry.name === 'dist' || entry.name === 'build') {
      continue;
    }
    
    if (entry.isDirectory()) {
      processDirectory(fullPath);
    } else if (entry.isFile()) {
      processFile(fullPath);
    }
  }
}

// Main execution
log(`\n${colors.bright}Console Migration Tool${colors.reset}`, colors.blue);
log(`Target: ${targetPath}`);
log(`Mode: ${dryRun ? 'Dry Run' : 'Live'}`);
log(`Auto-categorize: ${autoCategory ? 'Yes' : 'No'}`);
log(`\nProcessing...\n`);

const startTime = Date.now();

if (fs.statSync(targetPath).isDirectory()) {
  processDirectory(targetPath);
} else {
  processFile(targetPath);
}

const duration = Date.now() - startTime;

// Summary
log(`\n${colors.bright}Summary:${colors.reset}`);
log(`Files processed: ${filesProcessed}`);
log(`Files modified: ${dryRun ? `${filesModified} (would be)` : filesModified}`);
log(`Console calls found: ${consoleCalls}`);
log(`Console calls migrated: ${dryRun ? `${consoleCallsMigrated} (would be)` : consoleCallsMigrated}`);
log(`Time: ${duration}ms`);

if (dryRun) {
  log(`\n${colors.yellow}This was a dry run. Run without --dry-run to apply changes.${colors.reset}`);
}

// Usage help
if (args.includes('--help')) {
  log(`
Usage: node scripts/migrate-console.js [options] <file-or-directory>

Options:
  --dry-run          Show what would be changed without modifying files
  --verbose          Show detailed information about changes
  --no-auto-category Don't automatically assign debug categories
  --help             Show this help message

Examples:
  node scripts/migrate-console.js --dry-run src/
  node scripts/migrate-console.js --verbose src/services/
  node scripts/migrate-console.js src/components/MyComponent.tsx
`);
}