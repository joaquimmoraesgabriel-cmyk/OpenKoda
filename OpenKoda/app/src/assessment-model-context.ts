import { createContext } from 'react';
import type { Action, Settings } from './types';

export const AssessmentModelContext = createContext<{settings: Settings; action: Action} | null>(null);
