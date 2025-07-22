// src/features/design-system/patterns/MarkdownRenderer.tsx
import React, { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { cn } from '@/utils/classNameUtils';
import { Button } from '@/features/design-system/components/Button';
import { Download, Check, Settings, Terminal, Anchor } from 'lucide-react';

// Interactive code block component
const InteractiveCodeBlock = ({ language, code, className }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative group my-4">
      <SyntaxHighlighter
        style={vscDarkPlus}
        language={language}
        className={cn("!bg-muted/50 rounded-md", className)}
        showLineNumbers
      >
        {code}
      </SyntaxHighlighter>
      <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={handleCopy}
          aria-label="Copy code"
        >
          {copied ? <Check className="h-4 w-4 text-success" /> : <Download className="h-4 w-4" />}
        </Button>
      </div>
    </div>
  );
};

// Interactive link component
const InteractiveLink = ({ href, children, ...props }: { href?: string, children: React.ReactNode }) => {
  const isExternal = href?.startsWith('http');
  return (
    <a
      href={href}
      target={isExternal ? "_blank" : undefined}
      rel={isExternal ? "noopener noreferrer" : undefined}
      className="text-primary hover:underline inline-flex items-center gap-1"
      {...props}
    >
      {children}
      {isExternal && <Anchor className="h-3 w-3" />}
    </a>
  );
};

const MarkdownRenderer = ({ content, className }) => {
  return (
    <div className={cn("prose prose-sm prose-invert max-w-none", className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          code({ node, className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || '');
            const codeContent = String(children).replace(/\n$/, '');
            return match ? (
              <InteractiveCodeBlock language={match[1]} code={codeContent} className={className} />
            ) : (
              <code className="bg-muted/80 text-foreground px-1.5 py-1 rounded-md font-mono" {...props}>
                {children}
              </code>
            );
          },
          a: ({node, ...props}) => <InteractiveLink href={props.href}>{props.children}</InteractiveLink>,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
};

export default MarkdownRenderer;