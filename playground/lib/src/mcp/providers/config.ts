// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { createOpenAI } from '@ai-sdk/openai';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { ANTHROPIC_PROVIDER, GOOGLE_PROVIDER, LLMProvider, OPENAI_PROVIDER } from './ModelId';
import { LanguageModel } from 'ai';

interface ModelCreationConfig {
  apiKey: string;
  model: string;
  temperature?: number;
  maxTokens?: number;
}

interface ModelInfo {
  id: string;
  name: string;
  description?: string;
}

interface CreateModelOptions {
  provider: LLMProvider;
  config: ModelCreationConfig;
}

type ModelFactory = (options: CreateModelOptions) => LanguageModel;

export interface ProviderInfo {
  id: LLMProvider;
  displayName: string;
  models: ModelInfo[];
  defaultModel: string;
  requiresApiKey: boolean;
}


export const PROVIDERS: Record<LLMProvider, ProviderInfo> = {
  anthropic: {
    id: ANTHROPIC_PROVIDER,
    displayName: 'Anthropic',
    defaultModel: 'claude-opus-4-6',
    requiresApiKey: true,
    models: [
      { id: 'claude-opus-4-6', name: 'Claude Opus 4.6', description: 'Most intelligent model for agents and coding' },
      { id: 'claude-sonnet-4-6', name: 'Claude Sonnet 4.6', description: 'Best combination of speed and intelligence' },
      { id: 'claude-haiku-4-5-20251001', name: 'Claude Haiku 4.5', description: 'Fastest model with near-frontier intelligence' },
    ],
  },
  openai: {
    id: OPENAI_PROVIDER,
    displayName: 'OpenAI',
    defaultModel: 'gpt-5.2',
    requiresApiKey: true,
    models: [
      { id: 'gpt-5.2', name: 'GPT-5.2', description: 'Flagship model for coding and agentic tasks' },
      { id: 'gpt-5-mini', name: 'GPT-5 Mini', description: 'Fast and cost-efficient for well-defined tasks' },
      { id: 'gpt-5-nano', name: 'GPT-5 Nano', description: 'Fastest and cheapest for summarization and classification' },
      { id: 'o3', name: 'o3', description: 'Advanced reasoning for math, science, and coding (256K context)' },
      { id: 'o4-mini', name: 'o4-mini', description: 'Fast reasoning optimized for coding and visual tasks (256K context)' },
    ],
  },
  google: {
    id: GOOGLE_PROVIDER,
    displayName: 'Google',
    defaultModel: 'gemini-3.1-pro-preview',
    requiresApiKey: true,
    models: [
      { id: 'gemini-3.1-pro-preview', name: 'Gemini 3.1 Pro', description: 'Advanced intelligence with powerful agentic and coding capabilities' },
      { id: 'gemini-3-flash-preview', name: 'Gemini 3 Flash', description: 'Frontier-class performance at a fraction of the cost' },
    ],
  },
};

// For now let's use the first provider as the default
export const DEFAULT_PROVIDER = Object.keys(PROVIDERS)[0] as LLMProvider;

export const createModel: ModelFactory = ({ provider, config }) => {
  switch (provider) {
    case OPENAI_PROVIDER: {
      const openai = createOpenAI({
        apiKey: config.apiKey,
      });
      return openai(config.model);
    }
    case ANTHROPIC_PROVIDER: {
      const anthropic = createAnthropic({
        apiKey: config.apiKey,
      });
      return anthropic(config.model);
    }
    case GOOGLE_PROVIDER: {
      const google = createGoogleGenerativeAI({
        apiKey: config.apiKey,
      });
      return google(config.model);
    }
    default:
      throw new Error(`Unsupported provider: ${provider}`);
  }
};