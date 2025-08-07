// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, {
  createContext,
  useContext,
  useCallback,
  useMemo,
  useRef,
} from "react";
import { createModel } from "../providers/config";
import {
  ChatConfig,
  isLanguageModel,
  validateModelConfig,
} from "../providers/types";
import type { LanguageModel } from "ai";

// Simple cache entry for single model (we don't expect users to switch models frequently)
interface CachedModelEntry {
  model: LanguageModel;
  config: ChatConfig;
  apiKey: string;
}

export interface ModelContextValue {
  getCachedModel: (config: ChatConfig, apiKey: string) => LanguageModel;
  clearCache: () => void;
}

const ModelContext = createContext<ModelContextValue | undefined>(undefined);

// Simple single-model cache
class SingleModelCache {
  private cachedEntry: CachedModelEntry | null = null;

  get(config: ChatConfig, apiKey: string): LanguageModel | null {
    if (!this.cachedEntry) {
      return null;
    }

    // Check if the cached model matches current config
    const cached = this.cachedEntry;
    if (
      cached.config.provider === config.provider &&
      cached.config.model === config.model &&
      cached.config.temperature === config.temperature &&
      cached.config.maxTokens === config.maxTokens &&
      cached.apiKey === apiKey
    ) {
      return cached.model;
    }

    return null;
  }

  set(config: ChatConfig, apiKey: string, model: LanguageModel): void {
    this.cachedEntry = {
      model,
      config: { ...config },
      apiKey,
    };
  }

  clear(): void {
    this.cachedEntry = null;
  }
}

export interface ModelProviderProps {
  children: React.ReactNode;
}

export function ModelProvider({ children }: ModelProviderProps) {
  const cacheRef = useRef(new SingleModelCache());

  const getCachedModel = useCallback((config: ChatConfig, apiKey: string) => {
    const cache = cacheRef.current;

    // Validate inputs
    if (!apiKey || apiKey.trim().length === 0) {
      throw new Error(`API key is required for provider: ${config.provider}`);
    }

    if (!config.model || config.model.trim().length === 0) {
      throw new Error(`Model is required for provider: ${config.provider}`);
    }

    // Try to get from cache first
    const cachedModel = cache.get(config, apiKey);
    if (cachedModel) {
      return cachedModel;
    }

    // Validate model creation config
    const modelConfig = {
      apiKey,
      model: config.model,
      temperature: config.temperature,
      maxTokens: config.maxTokens,
    };

    if (!validateModelConfig(modelConfig)) {
      throw new Error(
        `Invalid model configuration: ${JSON.stringify(modelConfig)}`
      );
    }

    // Create new model if not in cache
    const model = createModel({
      provider: config.provider,
      config: modelConfig,
    });

    // Validate the created model
    if (!isLanguageModel(model)) {
      throw new Error(
        `Created model is not a valid LanguageModel for provider: ${config.provider}`
      );
    }

    // Cache the model (replaces any existing cached model)
    cache.set(config, apiKey, model);
    return model;
  }, []);

  const clearCache = useCallback(() => {
    cacheRef.current.clear();
  }, []);

  const value = useMemo<ModelContextValue>(
    () => ({
      getCachedModel,
      clearCache,
    }),
    [getCachedModel, clearCache]
  );

  return (
    <ModelContext.Provider value={value}>{children}</ModelContext.Provider>
  );
}

export const useModel = (): ModelContextValue => {
  const context = useContext(ModelContext);
  if (!context) {
    throw new Error("useModel must be used within a ModelProvider");
  }
  return context;
};
