import { memo, useLayoutEffect, useRef, type ReactNode } from 'react';
import { springAnimate, springs } from './motion/spring';

const fence = /^\s{0,3}(`{3,}|~{3,})([^\s`~]*)\s*$/;
const heading = /^\s{0,3}#{1,6}\s+(.+)$/;
const bullet = /^\s{0,3}[-+*]\s+(.+)$/;
const numbered = /^\s{0,3}(\d{1,6})[.)]\s+(.+)$/;

function RevealWord({text}:{text:string}) {
  const span=useRef<HTMLSpanElement>(null);
  useLayoutEffect(()=>{
    if(!span.current)return;
    const animation=springAnimate(span.current,value=>({opacity:Math.max(0,Math.min(1,value)),transform:`translateY(${(1-value)*6}px)`,filter:`blur(${Math.max(0,(1-value)*4)}px)`}),springs.word);
    return()=>animation?.cancel();
  },[]);
  return <span className="stream-word" ref={span}>{text}</span>;
}

function reveal(text:string,start:number,from:number) {
  if(!Number.isFinite(from)||start+text.length<=from)return text;
  // Only newly received words are animated. No artificial typing clock or delay.
  const prefix=Math.max(0,from-start), tail=text.slice(prefix);
  return <>{text.slice(0,prefix)}{tail.split(/(\s+)/).map((word,index)=>/\S/.test(word)&&index<160?<RevealWord key={start+prefix+':'+index} text={word}/>:word)}</>;
}

function inline(text: string, start=0, from=Infinity) {
  let offset=start;
  return text.split(/(`[^`\n]+`|\*\*[^*\n]+\*\*)/g).map((part, index) => {
    const position=offset;offset+=part.length;
    if (part.length > 2 && part.startsWith('`') && part.endsWith('`')) return <code key={index}>{reveal(part.slice(1, -1),position+1,from)}</code>;
    if (part.length > 4 && part.startsWith('**') && part.endsWith('**')) return <strong key={index}>{reveal(part.slice(2, -2),position+2,from)}</strong>;
    return <span key={index}>{reveal(part,position,from)}</span>;
  });
}

// Render text nodes only. Model output cannot load remote images, navigate the
// WebView, or introduce HTML. Unsupported Markdown stays readable as plain text.
export const ConversationText = memo(function ConversationText({ text, streaming=false }: { text: string; streaming?:boolean }) {
  const previous=useRef('');
  const normalized=text.replace(/\r\n?/g, '\n');
  // Bound the motion work even when a provider delivers a large buffered chunk.
  // Earlier characters remain visible immediately; none are queued or dropped.
  const revealFrom=streaming?Math.max(normalized.length-1600,normalized.startsWith(previous.current)?previous.current.length:0):Infinity;
  useLayoutEffect(()=>{previous.current=normalized;},[normalized]);
  const lines = normalized.split('\n');
  const offsets:number[]=[];let length=0;
  for(const line of lines){offsets.push(length);length+=line.length+1;}
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
      blocks.push(<pre key={start}><code>{reveal(content.join('\n'),offsets[start+1]??0,revealFrom)}</code></pre>);
      continue;
    }
    const title = line.match(heading);
    if (title) {
      blocks.push(<h4 key={start}>{inline(title[1],offsets[start]+line.indexOf(title[1]),revealFrom)}</h4>);
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
      const children = items.map((item, index) => <li key={index}>{inline(item,offsets[start+index]+lines[start+index].indexOf(item),revealFrom)}</li>);
      blocks.push(order ? <ol key={start} start={Number(order[1])}>{children}</ol> : <ul key={start}>{children}</ul>);
      continue;
    }
    const paragraph = [line];
    while (i < lines.length && lines[i].trim() && ![fence, heading, bullet, numbered].some(pattern => pattern.test(lines[i]))) {
      paragraph.push(lines[i++]);
    }
    blocks.push(<p key={start}>{inline(paragraph.join('\n'),offsets[start],revealFrom)}</p>);
  }
  return <div className="conversation-copy">{blocks}</div>;
});
