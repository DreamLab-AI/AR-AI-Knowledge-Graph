import React, { useEffect } from 'react';
import { X, Keyboard } from 'lucide-react';
import { Button } from '@/features/design-system/components/Button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/features/design-system/components/Card';
import { useKeyboardShortcutsList, formatShortcut } from '@/hooks/useKeyboardShortcuts';
import { cn } from '@/utils/classNameUtils';
import { useFocusTrap, useAnnounce } from '@/utils/accessibility';

interface KeyboardShortcutsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function KeyboardShortcutsModal({ isOpen, onClose }: KeyboardShortcutsModalProps) {
  const shortcuts = useKeyboardShortcutsList();
  const containerRef = useFocusTrap(isOpen);
  const announce = useAnnounce();

  useEffect(() => {
    if (isOpen) {
      announce('Keyboard shortcuts modal opened', 'polite');
    }
  }, [isOpen, announce]);

  useEffect(() => {
    if (!isOpen) return;

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      role="dialog"
      aria-modal="true"
      aria-labelledby="keyboard-shortcuts-title"
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Modal */}
      <Card
        ref={containerRef as any}
        className="relative z-10 w-full max-w-2xl max-h-[80vh] overflow-hidden"
      >
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-3">
          <div className="flex items-center gap-2">
            <Keyboard className="h-5 w-5" />
            <CardTitle id="keyboard-shortcuts-title" className="text-xl">Keyboard Shortcuts</CardTitle>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            className="h-8 w-8"
            aria-label="Close keyboard shortcuts modal"
          >
            <X className="h-4 w-4" />
          </Button>
        </CardHeader>

        <CardContent className="overflow-auto max-h-[calc(80vh-100px)]">
          {shortcuts.size === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              No keyboard shortcuts available
            </div>
          ) : (
            <div className="space-y-6">
              {Array.from(shortcuts).map(([category, categoryShortcuts]) => (
                <div key={category}>
                  <h3 className="text-sm font-semibold text-muted-foreground mb-3">
                    {category}
                  </h3>
                  <div className="space-y-2">
                    {categoryShortcuts.map((shortcut, index) => (
                      <div
                        key={index}
                        className={cn(
                          "flex items-center justify-between py-2 px-3 rounded-md",
                          "hover:bg-muted/50 transition-colors"
                        )}
                      >
                        <span className="text-sm">{shortcut.description}</span>
                        <kbd className={cn(
                          "px-2 py-1 text-xs font-mono rounded",
                          "bg-muted border border-border",
                          "text-muted-foreground"
                        )}>
                          {formatShortcut(shortcut)}
                        </kbd>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}