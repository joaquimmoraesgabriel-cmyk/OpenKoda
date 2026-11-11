import {describe, expect, it} from 'vitest';
import {defaultAppearance, parseAppearance} from './appearance';
import {composerSubmitKey} from './composer-preference';

describe('preferências do dispositivo',()=>{
  it('preserva escolhas anteriores e repõe campos desconhecidos sem ativar recursos',()=>{
    const current=parseAppearance(JSON.stringify({interfaceScale:125,motion:'reduced',density:'compact'}));
    expect(current).toEqual({...defaultAppearance,interfaceScale:125,motion:'reduced',density:'compact'});
    expect(parseAppearance(JSON.stringify({uiFont:'url(https://example.test/font)',codeFontSize:500,wallpaper:'custom',wallpaperImage:'data:image/svg+xml,<svg/>',wallpaperOpacity:30}))).toEqual(defaultAppearance);
  });
  it('mantém as duas famílias e tamanhos independentes',()=>{
    expect(parseAppearance(JSON.stringify({uiFont:'Segoe UI',uiFontSize:20,codeFont:'JetBrains Mono',codeFontSize:10}))).toMatchObject({uiFont:'Segoe UI',uiFontSize:20,codeFont:'JetBrains Mono',codeFontSize:10});
  });
  it('respeita composição, repetição, modificadores e quebras de linha em ambos os atalhos',()=>{
    const enter={key:'Enter',ctrlKey:false,metaKey:false,altKey:false,shiftKey:false,repeat:false,isComposing:false,keyCode:13};
    expect(composerSubmitKey(enter,'modifier-enter')).toBe(false);
    expect(composerSubmitKey(enter,'enter')).toBe(true);
    for (const modifier of [{ctrlKey:true},{metaKey:true}]) expect(composerSubmitKey({...enter,...modifier},'modifier-enter')).toBe(true);
    for (const protectedKey of [{shiftKey:true},{repeat:true},{isComposing:true},{keyCode:229},{altKey:true}]) {
      expect(composerSubmitKey({...enter,...protectedKey},'enter')).toBe(false);
      expect(composerSubmitKey({...enter,ctrlKey:true,...protectedKey},'modifier-enter')).toBe(false);
    }
  });
});
