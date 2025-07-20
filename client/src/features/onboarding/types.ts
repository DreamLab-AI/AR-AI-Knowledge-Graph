export interface OnboardingStep {
  id: string;
  title: string;
  description: string;
  target?: string; // CSS selector for element to highlight
  position?: 'top' | 'bottom' | 'left' | 'right' | 'center';
  action?: () => void | Promise<void>;
  skipable?: boolean;
  nextButtonText?: string;
  prevButtonText?: string;
}

export interface OnboardingFlow {
  id: string;
  name: string;
  description: string;
  steps: OnboardingStep[];
  completionKey?: string; // localStorage key to track completion
}

export interface OnboardingState {
  isActive: boolean;
  currentFlow: OnboardingFlow | null;
  currentStepIndex: number;
  completedFlows: string[];
}