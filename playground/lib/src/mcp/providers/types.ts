// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.


export * from './ChatMessage';
import type { ChatMessage } from './ChatMessage';
import type { LanguageModel } from 'ai';

export interface ChatConversation {
  id: string;
  title: string;
  messages: ChatMessage[];
  updatedAt: Date;
  provider: LLMProvider;
  model: string;
  isScratchPad?: boolean;
}

export const OPENAI_PROVIDER = 'openai' as const;
export const ANTHROPIC_PROVIDER = 'anthropic' as const;
export const GOOGLE_PROVIDER = 'google' as const;

export type LLMProvider = typeof OPENAI_PROVIDER | typeof ANTHROPIC_PROVIDER | typeof GOOGLE_PROVIDER;

export interface ChatConfig {
  provider: LLMProvider;
  model: string;
  temperature: number;
  maxTokens: number;
}

// Model-related interfaces
export interface ModelCreationConfig {
  apiKey: string;
  model: string;
  temperature?: number;
  maxTokens?: number;
}

// Type validation functions
export function isLanguageModel(obj: unknown): obj is LanguageModel {
  return (
    obj !== null &&
    typeof obj === 'object' &&
    'doGenerate' in obj &&
    'doStream' in obj &&
    typeof (obj as any).doGenerate === 'function' &&
    typeof (obj as any).doStream === 'function'
  );
}

export function validateModelConfig(config: unknown): config is ModelCreationConfig {
  if (!config || typeof config !== 'object') return false;
  
  const c = config as any;
  return (
    typeof c.apiKey === 'string' &&
    c.apiKey.trim().length > 0 &&
    typeof c.model === 'string' &&
    c.model.trim().length > 0 &&
    (c.temperature === undefined || (typeof c.temperature === 'number' && c.temperature >= 0 && c.temperature <= 2)) &&
    (c.maxTokens === undefined || (typeof c.maxTokens === 'number' && c.maxTokens > 0))
  );
}

