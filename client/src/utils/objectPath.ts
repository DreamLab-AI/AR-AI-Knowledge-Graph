/**
 * Utility functions for working with object paths
 * Uses dot notation to access nested properties (e.g., "visualisation.nodes.baseColor")
 */

type NestedObject = Record<string, any>;

/**
 * Gets a value from a nested object using a dot notation path
 * @param obj The object to get the value from
 * @param path The path to the value, using dot notation (e.g., "visualisation.nodes.baseColor")
 * @param defaultValue A default value to return if the path doesn't exist
 * @returns The value at the path, or the default value if not found
 */
export function get(obj: NestedObject, path: string, defaultValue?: any): any {
  if (!path || !obj) {
    return defaultValue;
  }

  const keys = path.split('.');
  let current = obj;

  for (let i = 0; i < keys.length; i++) {
    const key = keys[i];
    
    // Handle array indices in path
    if (key.includes('[') && key.includes(']')) {
      const arrayKey = key.substring(0, key.indexOf('['));
      const indexStr = key.substring(key.indexOf('[') + 1, key.indexOf(']'));
      const index = parseInt(indexStr, 10);
      
      if (current[arrayKey] === undefined || 
          !Array.isArray(current[arrayKey]) || 
          current[arrayKey][index] === undefined) {
        return defaultValue;
      }
      
      current = current[arrayKey][index];
      continue;
    }
    
    if (current[key] === undefined) {
      return defaultValue;
    }
    
    current = current[key];
  }
  
  return current;
}

/**
 * Sets a value in a nested object using a dot notation path
 * Creates the object structure if it doesn't exist
 * @param obj The object to set the value in
 * @param path The path to set, using dot notation (e.g., "visualisation.nodes.baseColor")
 * @param value The value to set
 */
export function set(obj: NestedObject, path: string, value: any): void {
  if (!path || !obj) {
    return;
  }

  const keys = path.split('.');
  let current = obj;

  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    
    // Handle array indices in path
    if (key.includes('[') && key.includes(']')) {
      const arrayKey = key.substring(0, key.indexOf('['));
      const indexStr = key.substring(key.indexOf('[') + 1, key.indexOf(']'));
      const index = parseInt(indexStr, 10);
      
      if (current[arrayKey] === undefined) {
        current[arrayKey] = [];
      }
      
      if (!Array.isArray(current[arrayKey])) {
        current[arrayKey] = [];
      }
      
      if (current[arrayKey][index] === undefined) {
        current[arrayKey][index] = {};
      }
      
      current = current[arrayKey][index];
      continue;
    }
    
    if (current[key] === undefined || typeof current[key] !== 'object' || current[key] === null) {
      current[key] = {};
    }
    
    current = current[key];
  }
  
  // Handle the last key
  const lastKey = keys[keys.length - 1];
  
  // Check if last key is an array index
  if (lastKey.includes('[') && lastKey.includes(']')) {
    const arrayKey = lastKey.substring(0, lastKey.indexOf('['));
    const indexStr = lastKey.substring(lastKey.indexOf('[') + 1, lastKey.indexOf(']'));
    const index = parseInt(indexStr, 10);
    
    if (current[arrayKey] === undefined) {
      current[arrayKey] = [];
    }
    
    if (!Array.isArray(current[arrayKey])) {
      current[arrayKey] = [];
    }
    
    current[arrayKey][index] = value;
  } else {
    current[lastKey] = value;
  }
}