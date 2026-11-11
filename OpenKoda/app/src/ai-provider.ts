import openaiLogo from './assets/providers/openai.png';
import openrouterLogo from './assets/providers/openrouter.png';
import groqLogo from './assets/providers/groq.png';

// Provider-published artwork bundled locally. No requests to brand sites at runtime.
export const providers = {
  openai: {name: 'OpenAI', endpoint: 'https://api.openai.com/v1', logo: openaiLogo},
  openrouter: {name: 'OpenRouter', endpoint: 'https://openrouter.ai/api/v1', logo: openrouterLogo},
  groq: {name: 'Groq', endpoint: 'https://api.groq.com/openai/v1', logo: groqLogo},
} as const;

export type ProviderId = keyof typeof providers;
export type AiSelection = {provider: ProviderId; model: string};
export type ProviderStatus = {
  provider?: ProviderId; hasApiKey: boolean; verified: boolean;
  checkedAt?: string; model: string; endpoint: string;
};

export function knownProvider(value: string): value is ProviderId {
  return Object.hasOwn(providers, value);
}

export function selectionLabel(value: {provider: string; model: string}) {
  if (!value.model) return 'Nenhum modelo configurado';
  return knownProvider(value.provider) ? providers[value.provider].name + ' · ' + value.model : value.model;
}
