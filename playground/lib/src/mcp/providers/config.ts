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
    defaultModel: 'claude-opus-4-1-20250805',
    requiresApiKey: true,
    models: [
      { id: 'claude-opus-4-1-20250805', name: 'Claude Opus 4.1', description: 'Most capable model with highest level of intelligence' },
      { id: 'claude-sonnet-4-20250514', name: 'Claude Sonnet 4', description: 'High-performance model with reasoning and efficiency' },
      { id: 'claude-3-7-sonnet-20250219', name: 'Claude Sonnet 3.7', description: 'High-performance model with extended thinking capabilities' },
    ],
  },
  openai: {
    id: OPENAI_PROVIDER,
    displayName: 'OpenAI',
    defaultModel: 'gpt-4.1',
    requiresApiKey: true,
    models: [
      { id: 'gpt-4.1', name: 'GPT-4.1', description: 'Latest flagship model with improved coding and instruction following (1M context)' },
      { id: 'gpt-4.1-mini', name: 'GPT-4.1 Mini', description: 'High-performance efficient model, beats GPT-4o (1M context)' },
      { id: 'gpt-4.1-nano', name: 'GPT-4.1 Nano', description: 'Fastest and most cost-effective model (1M context)' },
      { id: 'gpt-4-turbo', name: 'GPT-4 Turbo', description: 'Optimized for chat with 128K context' },
    ],
  },
  google: {
    id: GOOGLE_PROVIDER,
    displayName: 'Google',
    defaultModel: 'gemini-2.5-pro',
    requiresApiKey: true,
    models: [
      { id: 'gemini-2.5-pro', name: 'Gemini 2.5 Pro', description: 'Most powerful thinking model with maximum response accuracy' },
      { id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash', description: 'Best price-performance model with well-rounded capabilities' },
      { id: 'gemini-2.5-flash-lite', name: 'Gemini 2.5 Flash-Lite', description: 'Optimized for cost efficiency and high throughput' },
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