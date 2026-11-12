import { memo, type ReactNode } from 'react';

const fence = /^\s{0,3}(`{3,}|~{3,})([^\s`~]*)\s*$/;
const heading = /^\s{0,3}#{1,6}\s+(.+)$/;
const bullet = /^\s{0,3}[-+*]\s+(.+)$/;
const numbered = /^\s{0,3}(\d{1,6})[.)]\s+(.+)$/;

function inline(text: string) {
  return text.split(/(`[^`\n]+`|\*\*[^*\n]+\*\*)/g).map((part, index) => {
    if (part.length > 2 && part.startsWith('`') && part.endsWith('`')) return <code key={index}>{part.slice(1, -1)}</code>;
    if (part.length > 4 && part.startsWith('**') && part.endsWith('**')) return <strong key={index}>{part.slice(2, -2)}</strong>;
    return part;
  });
}

// Render text nodes only. Model output cannot load remote images, navigate the
// WebView, or introduce HTML. Unsupported Markdown stays readable as plain text.
export const ConversationText = memo(function ConversationText({ text }: { text: string }) {
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  const blocks: ReactNode[] = [];
  let i = 0;
  while (i < lines.length) {
    const start = i;
    const line = lines[i++];
    if (!line.trim()) continue;
    const marker = line.match(fence);
    if (marker) {
      const end = new RegExp('^\\s{0,3}' + marker[1][0] + '{' + marker[1].length + ',}\\s*$');
      const content = [];
      while (i < lines.length && !end.test(lines[i])) content.push(lines[i++]);
      if (i < lines.length) i++;
      blocks.push(<pre key={start}><code>{content.join('\n')}</code></pre>);
      continue;
    }
    const title = line.match(heading);
    if (title) {
      blocks.push(<h4 key={start}>{inline(title[1])}</h4>);
      continue;
    }
    const order = line.match(numbered);
    const first = line.match(bullet) ?? order;
    if (first) {
      const pattern = order ? numbered : bullet;
      const items = [first[first.length - 1]];
      while (i < lines.length) {
        const item = lines[i].match(pattern);
        if (!item) break;
        items.push(item[item.length - 1]);
        i++;
      }
      const children = items.map((item, index) => <li key={index}>{inline(item)}</li>);
      blocks.push(order ? <ol key={start} start={Number(order[1])}>{children}</ol> : <ul key={start}>{children}</ul>);
      continue;
    }
    const paragraph = [line];
    while (i < lines.length && lines[i].trim() && ![fence, heading, bullet, numbered].some(pattern => pattern.test(lines[i]))) {
      paragraph.push(lines[i++]);
    }
    blocks.push(<p key={start}>{inline(paragraph.join('\n'))}</p>);
  }
  return <div className="conversation-copy">{blocks}</div>;
});
