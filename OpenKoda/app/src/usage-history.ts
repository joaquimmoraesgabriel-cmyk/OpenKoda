import { knownProvider, providers } from './ai-provider';

export interface UsageRow {
  day: string; provider: string | null; model: string; assessmentId: string | null;
  requests: number; input: number | null; output: number | null; partial: number;
}

export function localDay(date = new Date()) {
  const pad = (value: number) => String(value).padStart(2, '0');
  return date.getFullYear() + '-' + pad(date.getMonth() + 1) + '-' + pad(date.getDate());
}

export function offsetDay(day: string, distance: number) {
  const date = new Date(day + 'T12:00:00');
  date.setDate(date.getDate() + distance);
  return localDay(date);
}

export function periodRows(rows: UsageRow[], range: string, today: string) {
  const first = range === '7d' ? offsetDay(today, -6) : range === '30d' ? offsetDay(today, -29) : undefined;
  return rows.filter(row => {
    if (row.day > today) return false;
    if (first) return row.day >= first;
    if (range === 'all') return true;
    return row.day === (range === 'today' ? today : range);
  });
}

export const providerName = (id: string | null) => id && knownProvider(id) ? providers[id].name : 'Outro provedor';
export const modelKey = (row: UsageRow) => JSON.stringify([row.provider, row.model]);
export type UsageTotals = {input:number; output:number; requests:number; partial:number; measured:boolean; volume:number};
export const emptyTotals = (): UsageTotals => ({input:0, output:0, requests:0, partial:0, measured:false, volume:0});

export function groupUsage(rows: UsageRow[], key: (row: UsageRow) => string) {
  const groups = new Map<string, UsageTotals>();
  for (const row of rows) {
    const id = key(row);
    const item = groups.get(id) ?? emptyTotals();
    item.input += row.input ?? 0;
    item.output += row.output ?? 0;
    item.volume = item.input + item.output;
    item.requests += row.requests;
    item.partial += row.partial;
    item.measured ||= row.input !== null || row.output !== null;
    groups.set(id, item);
  }
  return groups;
}

export function streaks(activeDays: string[], today: string) {
  const sorted = [...new Set(activeDays)].filter(day => day <= today).sort();
  let run = 0, longest = 0, previous = '';
  for (const day of sorted) {
    run = previous && offsetDay(previous,1) === day ? run + 1 : 1;
    longest = Math.max(longest, run);
    previous = day;
  }
  return {current: previous >= offsetDay(today,-1) ? run : 0, longest};
}
