import { useEffect, useState } from 'react';
import { native } from '../api';
import type { Action, Assessment } from '../types';
import KaliBridgePanel from './KaliBridgePanel';
import { ScopeAccess, useAssessmentScope } from './ScopeAccess';

export default function ProjectKaliTools({assessment,action,busy}:{assessment:Assessment;action:Action;busy:boolean}) {
  const scope = useAssessmentScope(native && ['code','files'].includes(assessment.kind) ? assessment : undefined);
  const [visibleFor,setVisibleFor] = useState('');
  useEffect(()=>{
    if(scope.status)setVisibleFor(scope.status.revision?.body.methods.includes('kali.rg.signals.v1')?assessment.id:'');
  },[scope.status,assessment.id]);
  if (visibleFor!==assessment.id) return null;
  return <ScopeAccess.Provider value={scope}><KaliBridgePanel assessment={assessment} action={action} busy={busy} scanOnly/></ScopeAccess.Provider>;
}
