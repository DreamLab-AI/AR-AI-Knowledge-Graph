declare module 'react-syntax-highlighter' {
  import { ComponentType, ReactNode } from 'react';

  export interface SyntaxHighlighterProps {
    language?: string;
    style?: any;
    children?: string;
    className?: string;
    showLineNumbers?: boolean;
    [key: string]: any;
  }

  export const Prism: ComponentType<SyntaxHighlighterProps>;
  export const Light: ComponentType<SyntaxHighlighterProps>;
}

declare module 'react-syntax-highlighter/dist/esm/styles/prism' {
  const vscDarkPlus: any;
  const dracula: any;
  const atomDark: any;
  const materialDark: any;
  const materialLight: any;
  const nord: any;
  const okaidia: any;
  const solarizedlight: any;
  const tomorrow: any;
  const vs: any;
  const xonokai: any;
  
  export {
    vscDarkPlus,
    dracula,
    atomDark,
    materialDark,
    materialLight,
    nord,
    okaidia,
    solarizedlight,
    tomorrow,
    vs,
    xonokai
  };
}
