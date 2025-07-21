/**
 * Deep merge utility for merging nested objects
 * This is used to properly merge settings objects with nested properties
 */

/**
 * Checks if a value is an object (but not null, array, or function)
 */
export function isObject(item: any): boolean {
  return (
    item !== null &&
    typeof item === 'object' &&
    !Array.isArray(item) &&
    !(item instanceof Date) &&
    !(item instanceof RegExp) &&
    !(item instanceof Map) &&
    !(item instanceof Set)
  );
}

/**
 * Deep merges multiple objects together
 * Later objects in the arguments list take precedence over earlier ones
 */
export function deepMerge<T extends Record<string, any>>(...objects: (T | undefined)[]): T {
  // Filter out undefined objects
  const validObjects = objects.filter(obj => obj !== undefined) as T[];

  // Return empty object if no valid objects
  if (validObjects.length === 0) {
    return {} as T;
  }

  // Return the first object if only one is provided
  if (validObjects.length === 1) {
    return { ...validObjects[0] };
  }

  // Start with a copy of the first object
  const result = { ...validObjects[0] };

  // Merge each subsequent object
  for (let i = 1; i < validObjects.length; i++) {
    const obj = validObjects[i];

    // Skip if undefined
    if (!obj) continue;

    // Merge each property
    for (const key in obj) {
      if (Object.prototype.hasOwnProperty.call(obj, key)) {
        const value = obj[key];

        // If both values are objects, recursively merge them
        if (isObject(result[key]) && isObject(value)) {
          result[key] = deepMerge(result[key], value);
        }
        // Otherwise, use the value from the current object
        else {
          result[key] = value;
        }
      }
    }
  }

  return result;
}
