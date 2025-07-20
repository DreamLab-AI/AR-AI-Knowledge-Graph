/**
 * Utility function to format setting names for display
 * Converts camelCase or snake_case to Title Case with spaces
 */
export const formatSettingName = (name: string): string => {
  // Replace camelCase with spaces
  const spacedName = name.replace(/([A-Z])/g, ' $1')
    // Replace underscores with spaces
    .replace(/_/g, ' ')
    // Trim any extra spaces
    .trim();
  
  // Capitalize first letter of each word
  return spacedName
    .split(' ')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
};