import { describe, expect, it } from 'vitest';
import { httpReason, validHttpRationale } from './http-validation';

describe('revisão humana HTTP',()=>{
  it('conta bytes UTF-8 da justificativa sem truncar',()=>{
    expect(validHttpRationale('  Revisão da resposta registrada.  ')).toBe(true);
    expect(validHttpRationale('é'.repeat(1000))).toBe(true);
    expect(validHttpRationale('é'.repeat(1001))).toBe(false);
    expect(validHttpRationale('😀'.repeat(500))).toBe(true);
    expect(validHttpRationale('😀'.repeat(501))).toBe(false);
  });
  it('recusa nota vazia e controles, preservando LF e tabulação',()=>{
    expect(validHttpRationale(' \n\t ')).toBe(false);
    expect(validHttpRationale('Observação\n\tlimitada')).toBe(true);
    for(const c of ['\u0000','\r','\u007f','\u0085','\u009f'])expect(validHttpRationale(`Fato${c}observado`)).toBe(false);
  });
  it('explica limitações conhecidas e conserva razões futuras sem inventar significado',()=>{
    expect(httpReason('changed_peer')).toContain('difere');
    expect(httpReason('capture_incomplete')).toContain('não foi concluída');
    expect(httpReason('future_reason')).toBe('Motivo informado pelo núcleo: future_reason');
  });
});
