import React, { useMemo, useCallback } from 'react';
import { FixedSizeList as List } from 'react-window';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../design-system/components/Card';
import { ChevronDown, Check } from 'lucide-react';
import { SettingControlComponent } from './SettingControlComponent';
import { useSettingsStore } from '../../../store/settingsStore';
import { cn } from '../../../utils/cn';
import { LoadingSpinner } from '../../design-system/components';

interface SettingItem {
  key: string;
  path: string;
  definition: any;
  isPowerUser?: boolean;
}

interface VirtualizedSettingsGroupProps {
  title: string;
  description?: string;
  items: SettingItem[];
  isPowerUser?: boolean;
  isExpanded: boolean;
  onToggle: () => void;
  savedNotification: string | null;
  loadingSettings: Set<string>;
  onSettingChange: (path: string, value: any) => void;
  groupIndex: number;
}

// Memoized row component for virtualization
const SettingRow = React.memo(({
  index,
  style,
  data
}: {
  index: number;
  style: React.CSSProperties;
  data: {
    items: SettingItem[];
    isPowerUser: boolean;
    savedNotification: string | null;
    loadingSettings: Set<string>;
    onSettingChange: (path: string, value: any) => void;
    getSettingValue: (path: string) => any;
  }
}) => {
  const { items, isPowerUser, savedNotification, loadingSettings, onSettingChange, getSettingValue } = data;
  const item = items[index];

  if (item.isPowerUser && !isPowerUser) return null;

  const value = getSettingValue(item.path);
  const isLoading = loadingSettings.has(item.path);

  return (
    <div style={style} className="px-4">
      <div className="relative">
        <div className="relative">
          {isLoading && <LoadingSpinner />}
          <SettingControlComponent
            path={item.path}
            settingDef={item.definition}
            value={value}
            onChange={(newValue) => onSettingChange(item.path, newValue)}
          />
        </div>
        {savedNotification === item.path && !isLoading && (
          <div className="absolute -top-1 -right-1 flex items-center gap-1 text-xs text-green-600 bg-green-50 px-2 py-1 rounded z-20">
            <Check className="h-3 w-3" />
            Saved
          </div>
        )}
      </div>
    </div>
  );
}, (prevProps, nextProps) => {
  // Custom comparison for better performance
  const prevItem = prevProps.data.items[prevProps.index];
  const nextItem = nextProps.data.items[nextProps.index];

  if (prevItem.path !== nextItem.path) return false;

  const prevValue = prevProps.data.getSettingValue(prevItem.path);
  const nextValue = nextProps.data.getSettingValue(nextItem.path);

  const prevLoading = prevProps.data.loadingSettings.has(prevItem.path);
  const nextLoading = nextProps.data.loadingSettings.has(nextItem.path);

  const prevSaved = prevProps.data.savedNotification === prevItem.path;
  const nextSaved = nextProps.data.savedNotification === nextItem.path;

  return (
    prevValue === nextValue &&
    prevLoading === nextLoading &&
    prevSaved === nextSaved &&
    prevProps.data.isPowerUser === nextProps.data.isPowerUser
  );
});

SettingRow.displayName = 'SettingRow';

export const VirtualizedSettingsGroup = React.memo(({
  title,
  description,
  items,
  isPowerUser,
  isExpanded,
  onToggle,
  savedNotification,
  loadingSettings,
  onSettingChange,
  groupIndex
}: VirtualizedSettingsGroupProps) => {
  // Create a stable getter for setting values
  const getSettingValue = useCallback((path: string) => {
    return useSettingsStore.getState().get(path);
  }, []);

  // Filter items for power users
  const visibleItems = useMemo(() =>
    items.filter(item => !item.isPowerUser || isPowerUser),
    [items, isPowerUser]
  );

  // Memoized data object for virtualized list
  const listData = useMemo(() => ({
    items: visibleItems,
    isPowerUser,
    savedNotification,
    loadingSettings,
    onSettingChange,
    getSettingValue
  }), [visibleItems, isPowerUser, savedNotification, loadingSettings, onSettingChange, getSettingValue]);

  if (isPowerUser !== undefined && !isPowerUser) return null;

  // Calculate optimal height for the virtual list
  const itemHeight = 80; // Approximate height of each setting row
  const maxVisibleItems = 8; // Show max 8 items before scrolling
  const listHeight = Math.min(visibleItems.length * itemHeight, maxVisibleItems * itemHeight);

  return (
    <Card className="mb-3 overflow-hidden">
      <CardHeader
        className="cursor-pointer py-3 px-4 hover:bg-muted/50 transition-colors"
        onClick={onToggle}
      >
        <div className="flex items-center justify-between">
          <div className="flex-1">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              {title}
              {isPowerUser && (
                <span className="text-xs px-1.5 py-0.5 bg-primary/10 text-primary rounded">
                  Pro
                </span>
              )}
            </CardTitle>
            {description && (
              <CardDescription className="text-xs mt-1">
                {description}
              </CardDescription>
            )}
          </div>
          <ChevronDown
            className={cn(
              "h-4 w-4 transition-transform duration-200",
              isExpanded ? "" : "-rotate-90"
            )}
          />
        </div>
      </CardHeader>

      {isExpanded && visibleItems.length > 0 && (
        <CardContent className="p-0">
          <List
            height={listHeight}
            itemCount={visibleItems.length}
            itemSize={itemHeight}
            width="100%"
            itemData={listData}
            overscanCount={2}
            className="scrollbar-thin scrollbar-thumb-muted scrollbar-track-background"
          >
            {SettingRow}
          </List>
        </CardContent>
      )}
    </Card>
  );
}, (prevProps, nextProps) => {
  // Custom comparison function for better performance
  return (
    prevProps.title === nextProps.title &&
    prevProps.isExpanded === nextProps.isExpanded &&
    prevProps.items === nextProps.items &&
    prevProps.isPowerUser === nextProps.isPowerUser &&
    prevProps.savedNotification === nextProps.savedNotification &&
    prevProps.loadingSettings === nextProps.loadingSettings &&
    prevProps.groupIndex === nextProps.groupIndex
  );
});

VirtualizedSettingsGroup.displayName = 'VirtualizedSettingsGroup';