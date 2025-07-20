/**
 * Simple logger utility with color-coded console output and log storage
 */

type LogLevel = 'debug' | 'info' | 'warn' | 'error';

interface LogEntry {
  timestamp: string;
  level: LogLevel;
  namespace: string;
  message: string;
  args: any[];
}

interface LoggerOptions {
  disabled?: boolean;
  level?: LogLevel;
  maxLogEntries?: number; // Optional limit for stored logs
}

const LOG_LEVEL_PRIORITY: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

const LOG_COLORS = {
  debug: '#8c8c8c', // gray
  info: '#4c9aff',  // blue
  warn: '#ffab00',  // orange
  error: '#ff5630', // red
};

// Central log storage (can be enhanced later if needed)
const logStorage: LogEntry[] = [];
const DEFAULT_MAX_LOG_ENTRIES = 1000;

export function createLogger(namespace: string, options: LoggerOptions = {}) {
  const { disabled = false, level = 'info', maxLogEntries = DEFAULT_MAX_LOG_ENTRIES } = options;
  const levelPriority = LOG_LEVEL_PRIORITY[level];

  function shouldLog(msgLevel: LogLevel): boolean {
    if (disabled) return false;
    return LOG_LEVEL_PRIORITY[msgLevel] >= levelPriority;
  }

  function formatMessage(message: any): string {
    if (typeof message === 'string') return message;
    if (message instanceof Error) {
      return message.stack ? message.stack : message.message;
    }
    try {
      // Attempt to stringify complex objects, handle potential circular references
      return JSON.stringify(message, (key, value) => {
        if (typeof value === 'object' && value !== null) {
          // Basic circular reference check (can be improved)
          if (value === message && key !== '') return '[Circular Reference]';
        }
        return value;
      }, 2);
    } catch (e) {
      return String(message);
    }
  }

  // Function to format arguments, handling Errors specifically
  function formatArgs(args: any[]): any[] {
      return args.map(arg => {
        if (arg instanceof Error) {
          return { message: arg.message, name: arg.name, stack: arg.stack }; // Serialize error
        }
        // Add more type handling if needed (e.g., Functions, DOM elements)
        return arg;
      });
  }


  function createLogMethod(logLevel: LogLevel) {
    return function(message: any, ...args: any[]) {
      if (!shouldLog(logLevel)) return;

      const color = LOG_COLORS[logLevel];
      const now = new Date();
      const timestamp = now.toISOString();
      const consoleTimestamp = now.toISOString().split('T')[1].slice(0, -1);
      const prefix = `%c[${consoleTimestamp}] [${namespace}]`;

      const formattedArgs = formatArgs(args);
      const formattedMessage = formatMessage(message);

      // Store the log entry
      logStorage.push({
        timestamp,
        level: logLevel,
        namespace,
        message: formattedMessage, // Store the formatted message
        args: formattedArgs, // Store formatted args
      });

      // Trim log storage if it exceeds the limit
      if (logStorage.length > maxLogEntries) {
        logStorage.shift(); // Remove the oldest entry
      }

      // Output to console
      console[logLevel === 'debug' ? 'log' : logLevel](
        `${prefix} ${formattedMessage}`,
        `color: ${color}; font-weight: bold;`,
        ...args // Log original args to console for better interactivity
      );
    };
  }

  // Function to retrieve stored logs
  function getLogs(): LogEntry[] {
    // Return a copy to prevent external modification
    return [...logStorage];
  }

  return {
    debug: createLogMethod('debug'),
    info: createLogMethod('info'),
    warn: createLogMethod('warn'),
    error: createLogMethod('error'),
    getLogs, // Expose the getLogs method
  };
}

export function createErrorMetadata(error: unknown): Record<string, any> {
  if (error instanceof Error) {
    return {
      message: error.message,
      name: error.name,
      stack: error.stack,
      // Potentially add other known properties of custom errors if any
    };
  }
  // If it's not an Error instance, try to get more details
  if (typeof error === 'object' && error !== null) {
    // Attempt to serialize the object.
    try {
      // Using Object.getOwnPropertyNames to include non-enumerable properties if error is an object
      const errorKeys = Object.getOwnPropertyNames(error);
      const serializableError = errorKeys.reduce((acc, key) => {
        acc[key] = (error as any)[key];
        return acc;
      }, {} as Record<string, any>);

      const serializedErrorString = JSON.stringify(serializableError, null, 2); // Pretty print
      return {
        message: `Non-Error object encountered. Details: ${serializedErrorString.substring(0, 500)}${serializedErrorString.length > 500 ? '...' : ''}`, // Truncate for sanity
        originalErrorType: 'Object', // Indicate it was an object
        // Consider if including the full 'error' object is too verbose or has circular refs
        // For now, relying on the stringified version.
      };
    } catch (e) {
      // JSON.stringify failed (e.g., circular references not caught by custom serializer)
      return {
        message: `Non-Error object (serialization failed): ${String(error)}`,
        originalErrorType: typeof error,
      };
    }
  }
  // Fallback for primitives or other types
  return {
    message: `Unknown error type: ${String(error)}`,
    originalErrorType: typeof error,
  };
}

export function createDataMetadata(data: Record<string, any>): Record<string, any> {
  return {
    ...data,
    timestamp: new Date().toISOString(),
  };
}