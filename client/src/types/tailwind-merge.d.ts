declare module 'tailwind-merge' {
  /**
   * Merges multiple Tailwind CSS class lists into a single class list.
   * @param classLists - Array of class lists to merge.
   * @returns - Merged class list.
   */
  export function twMerge(...classLists: (string | undefined | null | false)[]): string;

  /**
   * Creates a custom instance of twMerge with custom configuration.
   * @param config - Configuration object.
   * @returns - A custom twMerge function.
   */
  export function twMergeConfig(config: Record<string, unknown>): typeof twMerge;
}