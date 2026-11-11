import {describe,expect,it} from 'vitest';
import {groupUsage,modelKey,offsetDay,periodRows,streaks,type UsageRow} from './usage-history';

const row:UsageRow={day:'2026-03-01',provider:'groq',model:'shared',assessmentId:null,input:null,output:null,partial:1,requests:1};
describe('histórico de uso do perfil',()=>{
  it('mantém ausências, zeros e modelos homônimos separados',()=>{
    const groups=groupUsage([row,{...row,provider:'openai',input:0}],modelKey);
    expect(groups.size).toBe(2);
    expect(groups.get(modelKey(row))).toMatchObject({measured:false,volume:0,partial:1});
    expect(groups.get(modelKey({...row,provider:'openai'}))).toMatchObject({measured:true,volume:0});
  });
  it('filtra os dias do calendário local e preserva o histórico integral',()=>{
    const days=['2026-02-01','2026-03-01','2026-03-02','2026-03-08','2026-03-09'];
    const records=days.map(day=>({...row,day}));
    expect(periodRows(records,'7d','2026-03-08').map(r=>r.day)).toEqual(['2026-03-02','2026-03-08']);
    expect(periodRows(records,'today','2026-03-08')).toEqual([records[3]]);
    expect(periodRows(records,'2026-02-01','2026-03-08')).toEqual([records[0]]);
    expect(periodRows(records,'all','2026-03-08')).toHaveLength(4);
    expect(offsetDay('2024-03-01',-1)).toBe('2024-02-29');
    expect(streaks(['2026-03-03','2026-03-04','2026-03-07'],'2026-03-08')).toEqual({current:1,longest:2});
  });
});
