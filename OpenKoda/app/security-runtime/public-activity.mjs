// Only public assistant messages and content-free tool metadata cross the UI boundary.
// SDK reasoning activities and raw command lines are deliberately excluded.
function characterPrefix(text,limit) {
  let result='',count=0;
  for(const character of text.toWellFormed()){if(count++===limit)break;result+=character;}
  return result;
}
export function safeActivityText(text,key='') {
  if(typeof text!=='string')return '';
  return characterPrefix(text.split(key||'\0').join(key?'[redacted]':'\0')
    .replace(/\bsk-(?:proj-|or-v1-)?[A-Za-z0-9_-]{16,}/g,'[redacted]')
    .replace(/[\u0000-\u001f\u007f]/g,' ').trim(),480);
}
export function publicSessionUpdate(session,key) {
  const event=session?.event,payload=event?.payload;
  let text;
  if(event?.type==='event_msg'&&payload?.type==='agent_message')text=payload.message;
  else if(event?.type==='response_item'&&payload?.type==='message'&&payload.role==='assistant'&&
    ['commentary','final'].includes(payload.channel)&&Array.isArray(payload.content)) {
    text=payload.content.filter(item=>item?.type==='output_text'&&typeof item.text==='string').map(item=>item.text).join('\n');
  }
  const message=safeActivityText(text,key);
  return message?{type:'progress',stage:'message',message}:null;
}
export function publicToolUpdate(activity,key) {
  if(!['command','tool'].includes(activity?.kind)||!['running','completed','failed'].includes(activity.status))return null;
  const paths=(Array.isArray(activity.paths)?activity.paths:[]).filter(path=>typeof path==='string'&&
    !/^(?:[/\\]|[a-z]:)/i.test(path)&&!path.split(/[/\\]/).some(part=>part==='..')&&!path.includes('\0'))
    .slice(0,3).map(path=>characterPrefix(safeActivityText(path,key),100)).filter(Boolean);
  const label={running:'Consultando o código',completed:'Consulta de código concluída',failed:'Uma consulta de código não foi concluída'}[activity.status];
  return {type:'progress',stage:'tool',message:label+(paths.length?' · '+paths.join(', '):'.')};
}
export function progressMessage(event,label) {
  const reviews=event.reviewItems;
  const completed=reviews?.completed??event.filesCompleted,total=reviews?.total??event.filesTotal;
  return Number.isSafeInteger(completed)&&Number.isSafeInteger(total)&&completed>=0&&total>0&&completed<=total&&total<=1e6
    ?`${label} ${completed} de ${total} ${reviews?'itens de revisão':'arquivos'} informados pelo motor.`:label;
}
