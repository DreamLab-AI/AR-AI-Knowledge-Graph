import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { HelpCircle, X } from 'lucide-react';
import { cn } from '../../../utils/classNameUtils';
import { Button } from '../../design-system/components/Button';
import { HelpContent } from '../types';
import { helpRegistry } from '../HelpRegistry';

interface HelpContextValue {
  showHelp: (helpId: string) => void;
  hideHelp: () => void;
  toggleHelp: () => void;
  isHelpOpen: boolean;
  currentHelp: HelpContent | null;
}

const HelpContext = createContext<HelpContextValue | null>(null);

export function useHelp() {
  const context = useContext(HelpContext);
  if (!context) {
    throw new Error('useHelp must be used within HelpProvider');
  }
  return context;
}

interface HelpProviderProps {
  children: ReactNode;
}

export function HelpProvider({ children }: HelpProviderProps) {
  const [isHelpOpen, setIsHelpOpen] = useState(false);
  const [currentHelp, setCurrentHelp] = useState<HelpContent | null>(null);

  const showHelp = useCallback((helpId: string) => {
    const help = helpRegistry.getHelp(helpId);
    if (help) {
      setCurrentHelp(help);
      setIsHelpOpen(true);
    }
  }, []);

  const hideHelp = useCallback(() => {
    setIsHelpOpen(false);
    setTimeout(() => setCurrentHelp(null), 300); // Clear after animation
  }, []);

  const toggleHelp = useCallback(() => {
    setIsHelpOpen(prev => !prev);
  }, []);

  return (
    <HelpContext.Provider value={{ showHelp, hideHelp, toggleHelp, isHelpOpen, currentHelp }}>
      {children}
      <HelpPanel />
    </HelpContext.Provider>
  );
}

function HelpPanel() {
  const { isHelpOpen, currentHelp, hideHelp } = useHelp();

  if (!isHelpOpen || !currentHelp) return null;

  return (
    <div className={cn(
      "fixed right-4 top-20 z-40 w-96 max-h-[calc(100vh-6rem)]",
      "bg-background border rounded-lg shadow-xl",
      "animate-in slide-in-from-right duration-200"
    )}>
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b">
        <div className="flex items-center gap-2">
          <HelpCircle className="h-5 w-5 text-primary" />
          <h3 className="font-semibold">Help</h3>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={hideHelp}
          className="h-8 w-8"
        >
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Content */}
      <div className="p-4 overflow-y-auto max-h-[calc(100vh-12rem)]">
        <h4 className="font-medium text-lg mb-2">{currentHelp.title}</h4>
        <p className="text-sm text-muted-foreground mb-4">
          {currentHelp.description}
        </p>

        {currentHelp.detailedHelp && (
          <div className="mb-4">
            <h5 className="font-medium text-sm mb-1">Details</h5>
            <p className="text-sm text-muted-foreground">
              {currentHelp.detailedHelp}
            </p>
          </div>
        )}

        {currentHelp.examples && currentHelp.examples.length > 0 && (
          <div className="mb-4">
            <h5 className="font-medium text-sm mb-1">Examples</h5>
            <ul className="list-disc list-inside text-sm text-muted-foreground space-y-1">
              {currentHelp.examples.map((example, i) => (
                <li key={i}>{example}</li>
              ))}
            </ul>
          </div>
        )}

        {currentHelp.relatedTopics && currentHelp.relatedTopics.length > 0 && (
          <div>
            <h5 className="font-medium text-sm mb-1">Related Topics</h5>
            <div className="flex flex-wrap gap-2">
              {currentHelp.relatedTopics.map((topic, i) => (
                <span
                  key={i}
                  className="px-2 py-1 text-xs bg-muted rounded-md"
                >
                  {topic}
                </span>
              ))}
            </div>
          </div>
        )}

        {currentHelp.videoUrl && (
          <div className="mt-4">
            <a
              href={currentHelp.videoUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-primary hover:underline flex items-center gap-1"
            >
              Watch video tutorial
              <svg className="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
              </svg>
            </a>
          </div>
        )}
      </div>
    </div>
  );
}