import { request } from './api';

export type ProjectAnalysisOptions = {
  advancedEnabled:boolean;
  additionalContextEnabled:boolean;
  context:[string,string,string];
  maxCostUsd:number|null;
  minutes:number|null;
  title:string;
};
export const defaultProjectOptions=():ProjectAnalysisOptions=>({advancedEnabled:false,additionalContextEnabled:false,context:['','',''],maxCostUsd:null,minutes:null,title:''});
export const analysisContextLabels=['Vetores de ataque','Áreas de foco','Contexto adicional de segurança'];
export function readProjectOptions(value:unknown):ProjectAnalysisOptions {
  const options=(value as {options?:ProjectAnalysisOptions}|null)?.options;
  if(!options||typeof options.advancedEnabled!=='boolean'||typeof options.additionalContextEnabled!=='boolean'||
    !Array.isArray(options.context)||options.context.length!==3||!options.context.every(item=>typeof item==='string')||
    typeof options.title!=='string'||!(options.maxCostUsd===null||typeof options.maxCostUsd==='number'&&Number.isFinite(options.maxCostUsd)&&options.maxCostUsd>=.01&&options.maxCostUsd<=100)||
    !(options.minutes===null||typeof options.minutes==='number'&&Number.isInteger(options.minutes)&&options.minutes>=1&&options.minutes<=1440))throw new Error('Não foi possível confirmar as opções de análise deste projeto.');
  return options;
}
export const loadProjectOptions=async(projectId:string)=>readProjectOptions(await request('project.options.get',{projectId}));
export function effectiveScanLimit(options:ProjectAnalysisOptions) {
  return options.advancedEnabled&&options.maxCostUsd!==null?options.maxCostUsd:5;
}
