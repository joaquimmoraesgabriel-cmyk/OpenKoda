import openaiLogo from './assets/providers/openai.png';
import openrouterLogo from './assets/providers/openrouter.png';
import groqLogo from './assets/providers/groq.png';
import anthropicLogo from './assets/providers/anthropic.svg';
import geminiLogo from './assets/providers/gemini.svg';
import deepseekLogo from './assets/providers/deepseek.svg';
import xaiLogo from './assets/providers/xai.svg';

// Provider-published artwork bundled locally. No requests to brand sites at runtime.
export const providers = {
  openai: {name: 'OpenAI', endpoint: 'https://api.openai.com/v1', logo: openaiLogo},
  openrouter: {name: 'OpenRouter', endpoint: 'https://openrouter.ai/api/v1', logo: openrouterLogo},
  groq: {name: 'Groq', endpoint: 'https://api.groq.com/openai/v1', logo: groqLogo},
  anthropic: {name: 'Anthropic', endpoint: 'https://api.anthropic.com/v1', logo: anthropicLogo},
  gemini: {name: 'Google Gemini', endpoint: 'https://generativelanguage.googleapis.com/v1beta/openai', logo: geminiLogo},
  deepseek: {name: 'DeepSeek', endpoint: 'https://api.deepseek.com', logo: deepseekLogo},
  xai: {name: 'xAI', endpoint: 'https://api.x.ai/v1', logo: xaiLogo},
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
