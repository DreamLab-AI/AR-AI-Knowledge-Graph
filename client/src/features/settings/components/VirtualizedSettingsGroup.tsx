import React, { useMemo, useCallback, memo } from 'react';
import { FixedSizeList as List } from 'react-window';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/features/design-system/components/Card';
import { ChevronDown, Check } from 'lucide-react';
import { SettingControlComponent } from './SettingControlComponent';
import { useSettingsStore } from '@/store/settingsStore';
import { cn } from '@/utils/classNameUtils';
import { LoadingSpinner } from '@/features/design-system/components/LoadingSpinner';
import { UISettingDefinition } from '../config/settingsUIDefinition';
import { useSettingsPerformance } from '../hooks/useSettingsPerformance';

interface SettingItem {
  key: string;
  path: string;
  definition: UISettingDefinition;
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

// Pre-calculate item data to avoid recalculation
interface ItemData {
  items: SettingItem[];
  isPowerUser: boolean;
  savedNotification: string | null;
  loadingSettings: Set<string>;
  onSettingChange: (path: string, value: any) => void;
  getSettingValue: (path: string) => any;
  itemCache: Map<string, any>;
}

// Memoized row component with aggressive optimization
const SettingRow = memo(({
  index,
  style,
  data
}: {
  index: number;
  style: React.CSSProperties;
  data: ItemData;
}) => {
  const { items, isPowerUser, savedNotification, loadingSettings, onSettingChange, getSettingValue, itemCache } = data;
  const item = items[index];

  // Skip rendering for power-user items if not authorized
  if (item.isPowerUser && !isPowerUser) return null;

  // Use cached value if available
  const cacheKey = `${item.path}-${index}`;
  const cachedValue = itemCache.get(cacheKey);
  const value = cachedValue !== undefined ? cachedValue : getSettingValue(item.path);

  // Update cache if value changed
  if (cachedValue !== value) {
    itemCache.set(cacheKey, value);
  }

  const isLoading = loadingSettings.has(item.path);
  const isSaved = savedNotification === item.path;

  return (
    <div style={style} className="px-4">
      <div className="relative">
        <div className="relative">
          {isLoading && (
            <div className="absolute inset-0 flex items-center justify-center bg-background/50 z-10">
              <LoadingSpinner size="sm" />
            </div>
          )}
          <SettingControlComponent
            path={item.path}
            settingDef={item.definition}
            value={value}
            onChange={onSettingChange}
          />
        </div>
        {isSaved && !isLoading && (
          <div className="absolute -top-1 -right-1 flex items-center gap-1 text-xs text-green-600 bg-green-50 px-2 py-1 rounded z-20 animate-fade-in">
            <Check className="h-3 w-3" />
            Saved
          </div>
        )}
      </div>
    </div>
  );
}, (prevProps, nextProps) => {
  // Optimized comparison function
  if (prevProps.index !== nextProps.index) return false;
  
  const prevItem = prevProps.data.items[prevProps.index];
  const nextItem = nextProps.data.items[nextProps.index];
  
  if (prevItem.path !== nextItem.path) return false;
  
  // Use cached values for comparison
  const prevCacheKey = `${prevItem.path}-${prevProps.index}`;
  const nextCacheKey = `${nextItem.path}-${nextProps.index}`;
  
  const prevValue = prevProps.data.itemCache.get(prevCacheKey) ?? prevProps.data.getSettingValue(prevItem.path);
  const nextValue = nextProps.data.itemCache.get(nextCacheKey) ?? nextProps.data.getSettingValue(nextItem.path);
  
  return (
    prevValue === nextValue &&
    prevProps.data.loadingSettings.has(prevItem.path) === nextProps.data.loadingSettings.has(nextItem.path) &&
    (prevProps.data.savedNotification === prevItem.path) === (nextProps.data.savedNotification === nextItem.path) &&
    prevProps.data.isPowerUser === nextProps.data.isPowerUser
  );
});

SettingRow.displayName = 'SettingRow';

export const VirtualizedSettingsGroup = memo(({
  title,
  description,
  items,
  isPowerUser = false,
  isExpanded,
  onToggle,
  savedNotification,
  loadingSettings,
  onSettingChange,
  groupIndex
}: VirtualizedSettingsGroupProps) => {
  // Performance monitoring
  const { measureSearch, getMetrics } = useSettingsPerformance(`VirtualizedGroup-${title}`, {
    enableLogging: process.env.NODE_ENV === 'development',
    sampleRate: 50,
  });

  // Stable getter with caching
  const itemCache = useMemo(() => new Map<string, any>(), []);
  
  const getSettingValue = useCallback((path: string) => {
    // Check cache first
    const cached = itemCache.get(path);
    if (cached !== undefined) return cached;
    
    // Get from store and cache
    const value = useSettingsStore.getState().get(path);
    itemCache.set(path, value);
    return value;
  }, [itemCache]);

  // Filter and memoize visible items
  const visibleItems = useMemo(() => {
    const filtered = items.filter(item => !item.isPowerUser || isPowerUser);
    // Pre-cache values for visible items
    filtered.forEach(item => {
      const cacheKey = `${item.path}-${items.indexOf(item)}`;
      if (!itemCache.has(cacheKey)) {
        itemCache.set(cacheKey, getSettingValue(item.path));
      }
    });
    return filtered;
  }, [items, isPowerUser, itemCache, getSettingValue]);

  // Optimized setting change handler
  const handleSettingChange = useCallback((path: string, value: any) => {
    // Update cache immediately for responsive UI
    const itemIndex = items.findIndex(item => item.path === path);
    if (itemIndex >= 0) {
      itemCache.set(`${path}-${itemIndex}`, value);
    }
    onSettingChange(path, value);
  }, [items, itemCache, onSettingChange]);

  // Memoized data object with minimal dependencies
  const listData = useMemo<ItemData>(() => ({
    items: visibleItems,
    isPowerUser,
    savedNotification,
    loadingSettings,
    onSettingChange: handleSettingChange,
    getSettingValue,
    itemCache,
  }), [visibleItems, isPowerUser, savedNotification, loadingSettings, handleSettingChange, getSettingValue, itemCache]);

  // Skip rendering if not power user for power user groups
  if (isPowerUser !== undefined && !isPowerUser && items.every(item => item.isPowerUser)) {
    return null;
  }

  // Optimized dimensions
  const itemHeight = 72; // Reduced height for better density
  const maxVisibleItems = 10; // Show more items
  const listHeight = Math.min(visibleItems.length * itemHeight, maxVisibleItems * itemHeight);

  return (
    <Card className="mb-3 overflow-hidden border-border/50 shadow-sm">
      <CardHeader
        className="cursor-pointer py-2.5 px-4 hover:bg-muted/30 transition-colors duration-150"
        onClick={onToggle}
      >
        <div className="flex items-center justify-between">
          <div className="flex-1">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              {title}
              {items.some(item => item.isPowerUser) && isPowerUser && (
                <span className="text-xs px-1.5 py-0.5 bg-primary/10 text-primary rounded animate-fade-in">
                  Pro
                </span>
              )}
            </CardTitle>
            {description && (
              <CardDescription className="text-xs mt-0.5">
                {description}
              </CardDescription>
            )}
          </div>
          <ChevronDown
            className={cn(
              "h-4 w-4 transition-transform duration-200",
              isExpanded ? "rotate-0" : "-rotate-90"
            )}
          />
        </div>
      </CardHeader>

      {isExpanded && visibleItems.length > 0 && (
        <CardContent className="p-0 border-t border-border/30">
          <List
            height={listHeight}
            itemCount={visibleItems.length}
            itemSize={itemHeight}
            width="100%"
            itemData={listData}
            overscanCount={3}
            className="scrollbar-thin scrollbar-thumb-muted scrollbar-track-transparent"
            style={{ scrollbarWidth: 'thin' }}
          >
            {SettingRow}
          </List>
        </CardContent>
      )}
    </Card>
  );
}, (prevProps, nextProps) => {
  // Aggressive memoization - only re-render on significant changes
  return (
    prevProps.title === nextProps.title &&
    prevProps.isExpanded === nextProps.isExpanded &&
    prevProps.items === nextProps.items &&
    prevProps.isPowerUser === nextProps.isPowerUser &&
    prevProps.savedNotification === nextProps.savedNotification &&
    prevProps.loadingSettings === nextProps.loadingSettings &&
    prevProps.groupIndex === nextProps.groupIndex &&
    prevProps.onToggle === nextProps.onToggle &&
    prevProps.onSettingChange === nextProps.onSettingChange
  );
});

VirtualizedSettingsGroup.displayName = 'VirtualizedSettingsGroup';