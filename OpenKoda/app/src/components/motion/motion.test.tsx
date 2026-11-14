import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { springSamples, springs } from './spring';
import { normalizeOtp, OtpCode } from './OtpCode';
import { CardFieldPreview } from './CardField';
import { ConversationText } from '../ConversationText';

describe('motion controls',()=>{
  it('uses finite bounded samples and settles exactly for each supplied spring',()=>{
    for(const preset of Object.values(springs)) {
      const values=springSamples(preset[0],preset[1]);
      expect(values[0]).toBe(0);expect(values.at(-1)).toBe(1);
      expect(values.length).toBeLessThanOrEqual(121);
      expect(values.every(value=>Number.isFinite(value)&&value>=0&&value<1.5)).toBe(true);
    }
  });
  it('keeps every streamed character available and escapes provider HTML',()=>{
    const markup=renderToStaticMarkup(<ConversationText streaming text={'Olá <img src=x onerror=alert(1)>\r\n\r\n```js\r\nconst a = 1;\r\n```'}/>);
    expect(markup).not.toContain('<img');
    expect(markup).toContain('&lt;img');
    expect(markup).toContain('const');expect(markup).toContain('1;');
  });
  it('does not verify an OTP merely because six digits have been entered',()=>{
    expect(normalizeOtp('12a34 5678')).toBe('123456');
    const markup=renderToStaticMarkup(<OtpCode value="123456" onChange={()=>{}}/>);
    expect(markup).not.toContain('Código confirmado');
    expect(markup).toContain('one-time-code');
    expect(markup).toContain('data-status="idle"');
  });
  it('keeps payment preparation separate from actual payment entry',()=>{
    const markup=renderToStaticMarkup(<CardFieldPreview/>);
    expect(markup).toContain('Integração de pagamento pendente');
    expect(markup).not.toContain('<input');
  });
});
