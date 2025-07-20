export interface HelpContent {
  id: string;
  title: string;
  description: string;
  detailedHelp?: string;
  examples?: string[];
  relatedTopics?: string[];
  videoUrl?: string;
}

export interface HelpCategory {
  id: string;
  name: string;
  description: string;
  items: HelpContent[];
}

export interface TooltipConfig {
  showDelay?: number;
  hideDelay?: number;
  showHelpIndicator?: boolean;
  theme?: 'light' | 'dark' | 'auto';
}