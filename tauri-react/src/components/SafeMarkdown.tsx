import { useState, useId } from 'react';
import { ClipboardDocumentIcon, CheckIcon } from '@heroicons/react/20/solid';
import { parseMarkdownBlocks } from '../utils/markdownParser';

export {
  escapeHtml,
  sanitizeUrl,
  formatInlineMarkdown,
  parseMarkdownBlocks,
  parseNonCodeMarkdown,
} from '../utils/markdownParser';

export function CodeBlockView({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = useState(false);
  const lines = code.split('\n');

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback
    }
  };

  return (
    <div className="code-block-modern">
      <div className="code-block-header">
        <span className="code-lang-badge">{language || 'code'}</span>
        <button
          type="button"
          className="code-copy-btn"
          onClick={onCopy}
          aria-label={copied ? 'Code copied' : 'Copy code to clipboard'}
          title="Copy code"
        >
          {copied ? (
            <>
              <CheckIcon width={13} height={13} aria-hidden="true" />
              <span>Copied</span>
            </>
          ) : (
            <>
              <ClipboardDocumentIcon width={13} height={13} aria-hidden="true" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      <div className="code-content-wrap">
        <div className="code-line-numbers" aria-hidden="true">
          {lines.map((_, idx) => (
            <span key={idx}>{idx + 1}</span>
          ))}
        </div>
        <pre className="code-pre">
          <code>{code}</code>
        </pre>
      </div>
    </div>
  );
}

export function SafeMarkdown({ content }: { content: string }) {
  const idPrefix = useId();
  const blocks = parseMarkdownBlocks(content);

  return (
    <div className="markdown-render-flow">
      {blocks.map((block, idx) => {
        if (block.type === 'code') {
          return (
            <CodeBlockView
              key={`${idPrefix}-code-${idx}`}
              code={block.content}
              language={block.language || 'text'}
            />
          );
        }
        return (
          <div
            key={`${idPrefix}-html-${idx}`}
            className="markdown-block-html"
            dangerouslySetInnerHTML={{ __html: block.content }}
          />
        );
      })}
    </div>
  );
}
