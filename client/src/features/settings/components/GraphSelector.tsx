import React from 'react';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../design-system/components/Select';
import { Label } from '../../design-system/components/Label';

interface GraphSelectorProps {
  currentGraph: 'logseq' | 'visionflow';
  onGraphChange: (graph: 'logseq' | 'visionflow') => void;
}

export const GraphSelector: React.FC<GraphSelectorProps> = ({ 
  currentGraph, 
  onGraphChange 
}) => {
  return (
    <div className="flex items-center gap-2">
      <Label htmlFor="graph-selector">Active Graph:</Label>
      <Select value={currentGraph} onValueChange={onGraphChange}>
        <SelectTrigger id="graph-selector" className="w-40">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="logseq">
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 rounded-full bg-blue-500" />
              <span>Logseq</span>
            </div>
          </SelectItem>
          <SelectItem value="visionflow">
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 rounded-full bg-green-500" />
              <span>VisionFlow</span>
            </div>
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
};