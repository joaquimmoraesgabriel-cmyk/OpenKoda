const queued = new Map<string,string>();
export const openingAnalysisEvent = 'openkoda:opening-analysis';
export function stageOpeningAnalysis(assessmentId:string, prompt:string) {
  queued.set(assessmentId,prompt);
  window.dispatchEvent(new CustomEvent(openingAnalysisEvent,{detail:assessmentId}));
}
export function takeOpeningAnalysis(assessmentId:string) {
  const prompt = queued.get(assessmentId);
  queued.delete(assessmentId);
  return prompt;
}

