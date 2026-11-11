import { useEffect, useRef, useState } from 'react';
import { request } from '../api';
import { errorMessage, shortDate } from '../types';
import { KeyValue, Notice } from './shared';

const guidance = {
  architecture: 'A imagem Kali desta edição é para Windows x64. Use um host x64 para o ambiente gerido.',
  windows_update: 'O fluxo de preparação usa Windows 10 build 19041 ou posterior, ou Windows 11. Consulte o Windows Update e volte a verificar.',
  virtualization: 'A virtualização está desabilitada no firmware. Consulte o administrador ou o fabricante para habilitar o recurso e depois verifique novamente.',
  prepare_wsl: 'Peça a um administrador para preparar os componentes do WSL em um PowerShell elevado. O comando a seguir instala o WSL sem escolher uma distribuição. Reinicie quando o Windows solicitar.',
  update_wsl: 'O WSL respondeu, mas sua versão não confirmou a instalação por nome e destino. Revise a atualização do WSL com o administrador. A atualização é compartilhada pelas distribuições deste Windows.',
  verify_wsl: 'Confira wsl --status no PowerShell. Se a Plataforma de Máquina Virtual estiver desabilitada, peça ao administrador para habilitá-la em Recursos do Windows. Conclua o reinício solicitado e volte a verificar.',
  inspect: 'Alguns requisitos não puderam ser verificados. Confira as permissões de diagnóstico do Windows com o administrador e tente novamente.',
  review_kali: 'O WSL respondeu e oferece instalação dedicada por nome e destino. Consulte abaixo a identidade e o diagnóstico do Kali. No primeiro uso, selecione Revisar preparação do Kali.',
} as const;

interface Readiness {
  step: keyof typeof guidance; startedAt: string; checkedAt: string; rebootObserved: boolean | null;
  windowsBuild: number | null; architecture: string | null; hypervisor: boolean | null;
  virtualizationFirmware: boolean | null; virtualMachinePlatform: boolean | null; freeBytes: number | null;
  distributionCount: number | null; wslClient: boolean | null; wslVersion: string | null;
}
function flag(value: boolean | null) {
  return value === true ? 'Sim' : value === false ? 'Não' : 'Não verificado';
}

export default function KaliHostReadiness({ disabled }: { disabled: boolean }) {
  const [result, setResult] = useState<Readiness | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const generation = useRef(0);
  async function check(action: 'environment.host.resume' | 'environment.host.check') {
    const id = ++generation.current;
    setPending(true); setError('');
    try {
      const view = await request<Readiness | null>(action);
      if (id === generation.current) setResult(view);
    } catch (reason) {
      if (id === generation.current) { setResult(null); setError(errorMessage(reason)); }
    } finally {
      if (id === generation.current) setPending(false);
    }
  }
  useEffect(() => {
    void check('environment.host.resume');
    return () => { generation.current++; };
  }, []);
  return <section className="content-section" aria-labelledby="wsl-preparation-heading">
    <h2 id="wsl-preparation-heading">Preparar o Windows para o WSL</h2>
    <p>Verifique os requisitos antes de preparar o Kali. Ao iniciar, o aplicativo salva um ponto de retomada. Ao voltar a esta aba, o diagnóstico iniciado consulta novamente o Windows.</p>
    <button className="button" disabled={disabled || pending} onClick={() => void check('environment.host.check')}>
      {pending ? 'Verificando requisitos…' : result ? 'Verificar novamente os requisitos' : 'Verificar requisitos do Windows'}
    </button>
    {error && <div role="alert"><Notice tone="warning">{error}</Notice></div>}
    {result && <>
      <p role="status">{guidance[result.step] ?? guidance.inspect}</p>
      {result.step === 'prepare_wsl' && <pre aria-label="Comando manual para o administrador">wsl --install --no-distribution</pre>}
      {result.step === 'update_wsl' && <pre aria-label="Comando manual para atualizar o WSL">wsl --update</pre>}
      <KeyValue label="Windows">{result.windowsBuild == null ? 'Build não verificada' : 'Build '+result.windowsBuild} · {result.architecture ?? 'Arquitetura não verificada'}</KeyValue>
      <KeyValue label="Versão do WSL">{result.wslVersion ?? (result.wslClient === false ? 'Cliente ausente' : 'Não verificada')}</KeyValue>
      <KeyValue label="Plataforma de Máquina Virtual habilitada">{flag(result.virtualMachinePlatform)}</KeyValue>
      <KeyValue label="Hipervisor detectado">{flag(result.hypervisor)}</KeyValue>
      <KeyValue label="Virtualização informada pelo firmware">{flag(result.virtualizationFirmware)}</KeyValue>
      <KeyValue label="Distribuições registradas neste usuário">{result.distributionCount ?? 'Não verificado'}</KeyValue>
      <KeyValue label="Espaço livre no destino">{result.freeBytes == null ? 'Não verificado' : (result.freeBytes / 2**30).toLocaleString('pt-BR', { maximumFractionDigits: 1 })+' GiB'}</KeyValue>
      <KeyValue label="Reinício observado desde o início">{flag(result.rebootObserved)}</KeyValue>
      <KeyValue label="Verificação atual">{shortDate(result.checkedAt)}</KeyValue>
    </>}
  </section>;
}
