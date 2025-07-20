import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge" // Already installed in package.json

/**
 * Merges class names with Tailwind CSS classes
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Format a setting name for display (convert camelCase to Title Case)
 */
export function formatSettingName(name: string): string {
  // Handle special case acronyms (e.g., "XR" should remain uppercase)
  if (name === 'xr') return 'XR';
  
  // Replace camelCase with spaces
  const spacedName = name.replace(/([A-Z])/g, ' $1').trim();
  
  // Capitalize first letter of each word
  return spacedName.charAt(0).toUpperCase() + spacedName.slice(1);
}

/**
 * Check if a value is defined (not undefined and not null)
 */
export function isDefined<T>(value: T | undefined | null): value is T {
  return value !== undefined && value !== null;
}

/**
 * Debounce a function call
 */
export function debounce<T extends (...args: any[]) => any>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout> | null = null;
  
  return function(...args: Parameters<T>) {
    if (timeout) {
      clearTimeout(timeout);
    }
    
    timeout = setTimeout(() => {
      func(...args);
    }, wait);
  };
}

/**
 * Truncate a string to the specified length
 */
export function truncate(str: string, length: number): string {
  if (str.length <= length) {
    return str;
  }
  
  return str.slice(0, length) + '...';
}