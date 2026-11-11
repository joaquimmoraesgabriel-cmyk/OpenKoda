export const sensitivityLabels = {
  unclassified:'Não classificado',
  public:'Público',
  internal:'Interno',
  sensitive:'Sensível',
} as const;
export type DataSensitivity = keyof typeof sensitivityLabels;

export const dataCategoryLabels = {
  human_text:'Texto informado pela pessoa',
  target_metadata:'Identificação de fontes e alvos',
  execution_summary:'Resumo de execuções',
  evidence_excerpt:'Trechos de evidências',
  source_excerpt:'Trechos de código e arquivos',
  finding_text:'Descrição e validação de achados',
  activity_metadata:'Registro de atividades',
} as const;

export const dataDestinationLabels = {
  remote_ai:'Provedor de IA',
  local_kali:'Kali local',
  report_export:'Relatório local',
  recovery_backup:'Backup de recuperação',
  local_raw_view:'Revisão local do conteúdo armazenado',
} as const;

export function dataLabel(value:string,labels:Record<string,string>){return labels[value]??value;}
