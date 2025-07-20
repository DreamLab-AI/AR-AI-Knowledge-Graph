// client/src/types/lucide-react.d.ts
declare module 'lucide-react' {
  import { FC, SVGProps } from 'react';

  export interface IconProps extends SVGProps<SVGSVGElement> {
    size?: string | number;
    color?: string;
    strokeWidth?: string | number;
  }

  export type Icon = FC<IconProps>;

  // This generic export will cover all icons from the library
  const icons: { [key: string]: Icon };
  export default icons;

  // You can still export specific icons if you need explicit names for clarity
  export const X: Icon;
  export const Settings: Icon;
  export const Eye: Icon;
  export const EyeOff: Icon;
  export const ChevronDown: Icon;
  export const ChevronUp: Icon;
  export const ChevronLeft: Icon;
  export const ChevronRight: Icon;
  export const SkipForward: Icon;
  export const Check: Icon;
  export const CheckCircle: Icon;
  export const Send: Icon;
  export const Download: Icon;
  export const Anchor: Icon;
  export const Search: Icon;
  export const Keyboard: Icon;
  export const User: Icon;
  export const Undo: Icon;
  export const Redo: Icon;
  export const History: Icon;
  export const HelpCircle: Icon;
  export const ExternalLink: Icon;
  export const Info: Icon;
  export const Minimize: Icon;
  export const Maximize: Icon;
  export const Terminal: Icon;
  export const Smartphone: Icon;
  export const Loader2: Icon;
  export const AlertTriangle: Icon;
  export const Circle: Icon;
}