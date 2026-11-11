export function parsePorts(value:string):{ports:number[];error:string|null} {
  const trimmed=value.trim();
  if(!trimmed)return {ports:[],error:'Informe pelo menos uma porta TCP.'};
  const tokens=trimmed.split(/[\s,]+/);
  if(tokens.length>16)return {ports:[],error:'Informe no máximo 16 portas TCP.'};
  if(tokens.some(token=>!/^\d+$/.test(token)))return {ports:[],error:'Use apenas números de portas separados por vírgula ou espaço, sem intervalos.'};
  const ports=tokens.map(Number);
  if(ports.some(port=>!Number.isInteger(port)||port<1||port>65535))return {ports:[],error:'Cada porta deve estar entre 1 e 65535.'};
  if(new Set(ports).size!==ports.length)return {ports:[],error:'Informe portas únicas, sem repetir valores.'};
  return {ports,error:null};
}
