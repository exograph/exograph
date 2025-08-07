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
import { ANTHROPIC_PROVIDER, GOOGLE_PROVIDER, LLMProvider, OPENAI_PROVIDER, ModelCreationConfig } from './types';
import { LanguageModel } from 'ai';

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

interface ProviderInfo {
  id: LLMProvider;
  name: string;
  displayName: string;
  models: ModelInfo[];
  defaultModel: string;
  requiresApiKey: boolean;
}


export const PROVIDERS: Record<string, ProviderInfo> = {
  openai: {
    id: OPENAI_PROVIDER,
    name: 'OpenAI',
    displayName: 'OpenAI',
    defaultModel: 'gpt-4o',
    requiresApiKey: true,
    models: [
      { id: 'gpt-4.1', name: 'GPT-4.1', description: 'Latest model with improved coding and instruction following (1M context)' },
      { id: 'gpt-4.1-mini', name: 'GPT-4.1 Mini', description: 'Smaller, efficient version of GPT-4.1 (1M context)' },
      { id: 'gpt-4o', name: 'GPT-4o', description: 'Multimodal model with vision capabilities' },
      { id: 'gpt-4o-mini', name: 'GPT-4o Mini', description: 'Cost-efficient small model with vision' },
      { id: 'gpt-4-turbo', name: 'GPT-4 Turbo', description: 'Optimized for chat with 128K context' },
      { id: 'gpt-3.5-turbo', name: 'GPT-3.5 Turbo', description: 'Fast and cost-effective' },
    ],
  },
  anthropic: {
    id: 'anthropic',
    name: 'Anthropic',
    displayName: 'Anthropic',
    defaultModel: 'claude-sonnet-4-20250514',
    requiresApiKey: true,
    models: [
      { id: 'claude-opus-4-1-20250805', name: 'Claude Opus 4.1', description: 'Most capable model with highest level of intelligence' },
      { id: 'claude-sonnet-4-20250514', name: 'Claude Sonnet 4', description: 'High-performance model with reasoning and efficiency' },
      { id: 'claude-3-5-sonnet-20241022', name: 'Claude 3.5 Sonnet v2', description: 'Upgraded Sonnet 3.5 with enhanced capabilities' },
      { id: 'claude-3-5-haiku-20241022', name: 'Claude 3.5 Haiku', description: 'Fastest model with intelligence' },
    ],
  },
  google: {
    id: 'google',
    name: 'Google',
    displayName: 'Google',
    defaultModel: 'gemini-2.5-flash',
    requiresApiKey: true,
    models: [
      { id: 'gemini-2.5-pro', name: 'Gemini 2.5 Pro', description: 'Most advanced multimodal model' },
      { id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash', description: 'Fast, efficient multimodal model' },
      { id: 'gemini-2.5-flash-lite', name: 'Gemini 2.5 Flash-Lite', description: 'Most cost-efficient high throughput model' },
      { id: 'gemini-2.0-flash-exp', name: 'Gemini 2.0 Flash (Experimental)', description: 'Latest experimental model' },
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

export const getDefaultConfig = (provider: string) => {
  const providerInfo = PROVIDERS[provider];
  if (!providerInfo) {
    throw new Error(`Unknown provider: ${provider}`);
  }

  return {
    provider: providerInfo.id,
    model: providerInfo.defaultModel,
    temperature: 0.7,
    maxTokens: 2048,
  };
};