export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

export function sanitizeUrl(rawUrl: string): string | null {
  const trimmed = rawUrl.trim();
  if (!trimmed) return null;
  if (/^(https?:\/\/|mailto:)/i.test(trimmed)) {
    return trimmed;
  }
  if (trimmed.startsWith('#') || trimmed.startsWith('/')) {
    return trimmed;
  }
  return null;
}

export function sanitizeImageUrl(rawUrl: string): string | null {
  const trimmed = rawUrl.trim();
  if (!trimmed) return null;
  if (/^(https?:\/\/)/i.test(trimmed) || trimmed.startsWith('/')) return trimmed;
  if (/^data:image\/(png|jpeg|webp|gif|svg\+xml);base64,[a-z0-9+/=]+$/i.test(trimmed)) return trimmed;
  return null;
}

export function formatInlineMarkdown(text: string): string {
  let result = escapeHtml(text);

  // Render only trusted image URL schemes; all other content remains escaped.
  result = result.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_match, alt, url) => {
    const cleanUrl = sanitizeImageUrl(url);
    return cleanUrl ? `<img src=\"${escapeHtml(cleanUrl)}\" alt=\"${escapeHtml(alt)}\" class=\"markdown-image\" loading=\"lazy\" />` : '';
  });

  // Bold + Italic: ***text*** or ___text___
  result = result.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');

  // Bold: **text** or __text__
  result = result.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  result = result.replace(/__(.+?)__/g, '<strong>$1</strong>');

  // Italic: *text* or _text_
  result = result.replace(/\*([^*\n]+?)\*/g, '<em>$1</em>');
  result = result.replace(/_([^_\n]+?)_/g, '<em>$1</em>');

  // Inline code: `code`
  result = result.replace(/`([^`\n]+?)`/g, '<code class="inline-code">$1</code>');

  // Links: [label](url)
  result = result.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_match, label, url) => {
    const cleanUrl = sanitizeUrl(url);
    if (!cleanUrl) {
      return label;
    }
    return `<a href="${escapeHtml(cleanUrl)}" target="_blank" rel="noopener noreferrer" class="markdown-link">${label}</a>`;
  });

  return result;
}

export function parseMarkdownBlocks(text: string): Array<{ type: 'code' | 'html'; language?: string; content: string }> {
  const parts = text.split(/(```[\s\S]*?```)/g);
  const blocks: Array<{ type: 'code' | 'html'; language?: string; content: string }> = [];

  for (const part of parts) {
    if (!part) continue;
    if (part.startsWith('```') && part.endsWith('```')) {
      const inside = part.slice(3, -3);
      const firstNewline = inside.indexOf('\n');
      let language = 'text';
      let code = inside;
      if (firstNewline !== -1) {
        language = inside.slice(0, firstNewline).trim() || 'text';
        code = inside.slice(firstNewline + 1);
      }
      if (code.endsWith('\n')) code = code.slice(0, -1);
      blocks.push({ type: 'code', language, content: code });
    } else {
      const html = parseNonCodeMarkdown(part);
      if (html.trim()) {
        blocks.push({ type: 'html', content: html });
      }
    }
  }

  return blocks;
}

export function parseNonCodeMarkdown(text: string): string {
  const lines = text.split('\n');
  const output: string[] = [];
  let inUl = false;
  let inOl = false;
  let inBlockquote = false;
  let inTable = false;
  let tableHeaderCols: string[] = [];
  let tableBodyRows: string[][] = [];

  const closeList = () => {
    if (inUl) { output.push('</ul>'); inUl = false; }
    if (inOl) { output.push('</ol>'); inOl = false; }
  };

  const closeBlockquote = () => {
    if (inBlockquote) { output.push('</blockquote>'); inBlockquote = false; }
  };

  const flushTable = () => {
    if (!inTable) return;
    let html = '<div class="table-wrap"><table>';
    if (tableHeaderCols.length) {
      html += '<thead><tr>' + tableHeaderCols.map(c => `<th>${formatInlineMarkdown(c)}</th>`).join('') + '</tr></thead>';
    }
    if (tableBodyRows.length) {
      html += '<tbody>';
      for (const row of tableBodyRows) {
        html += '<tr>' + row.map(c => `<td>${formatInlineMarkdown(c)}</td>`).join('') + '</tr>';
      }
      html += '</tbody>';
    }
    html += '</table></div>';
    output.push(html);
    inTable = false;
    tableHeaderCols = [];
    tableBodyRows = [];
  };

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const line = rawLine.trim();

    // Table detection: line starting and ending with '|' or having '|'
    if (line.includes('|') && (line.startsWith('|') || line.endsWith('|'))) {
      closeList();
      closeBlockquote();
      const cells = line.split('|').map(c => c.trim()).filter((_c, idx, arr) => idx !== 0 && idx !== arr.length - 1);
      const nextLine = lines[i + 1]?.trim() ?? '';
      if (!inTable && /^\|?(\s*:?-+:?\s*\|)+\s*:?-+:?\s*\|?$/.test(nextLine)) {
        inTable = true;
        tableHeaderCols = cells;
        i++; // Skip delimiter line
        continue;
      } else if (inTable) {
        tableBodyRows.push(cells);
        continue;
      }
    } else {
      flushTable();
    }

    // Horizontal rule
    if (/^(---|___|\*\*\*)$/.test(line)) {
      closeList();
      closeBlockquote();
      output.push('<hr class="markdown-hr" />');
      continue;
    }

    // Headings
    if (line.startsWith('#')) {
      closeList();
      closeBlockquote();
      const match = line.match(/^(#{1,4})\s+(.+)$/);
      if (match) {
        const level = match[1].length;
        const headingText = formatInlineMarkdown(match[2]);
        output.push(`<h${level} class="markdown-h${level}">${headingText}</h${level}>`);
        continue;
      }
    }

    // Blockquote
    if (line.startsWith('>')) {
      closeList();
      if (!inBlockquote) {
        output.push('<blockquote class="markdown-blockquote">');
        inBlockquote = true;
      }
      const bqText = formatInlineMarkdown(line.replace(/^>\s?/, ''));
      output.push(`<p>${bqText}</p>`);
      continue;
    } else {
      closeBlockquote();
    }

    // Unordered list
    if (/^[-*]\s+/.test(line)) {
      if (inOl) { output.push('</ol>'); inOl = false; }
      if (!inUl) { output.push('<ul class="markdown-ul">'); inUl = true; }
      const itemText = formatInlineMarkdown(line.replace(/^[-*]\s+/, ''));
      output.push(`<li>${itemText}</li>`);
      continue;
    }

    // Ordered list
    if (/^\d+\.\s+/.test(line)) {
      if (inUl) { output.push('</ul>'); inUl = false; }
      if (!inOl) { output.push('<ol class="markdown-ol">'); inOl = true; }
      const itemText = formatInlineMarkdown(line.replace(/^\d+\.\s+/, ''));
      output.push(`<li>${itemText}</li>`);
      continue;
    }

    closeList();

    // Empty line
    if (!line) {
      continue;
    }

    // Normal paragraph
    const pText = formatInlineMarkdown(rawLine);
    output.push(`<p class="markdown-p">${pText}</p>`);
  }

  closeList();
  closeBlockquote();
  flushTable();

  return output.join('\n');
}
