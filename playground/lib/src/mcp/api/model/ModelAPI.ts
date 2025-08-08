// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import type { LanguageModel } from 'ai';
import { ModelId, LLMProvider } from '../../providers/ModelId';
import { ModelSettings, ModelOption } from '../../context/ModelSettingsContext';
import { PROVIDERS, DEFAULT_PROVIDER, createModel } from '../../providers/config';
import { ModelStorage } from '../storage/ModelStorage';
import { ModelValidator } from './ModelValidator';

const DEFAULT_SETTINGS: ModelSettings = {
  temperature: 0.7,
  maxTokens: 2048,
};

export class ModelAPI {
  private modelSettings: Record<string, ModelSettings> = {};
  private listeners: Set<() => void> = new Set();

  constructor() {
    this.loadFromStorage();
  }

  addChangeListener(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notifyListeners(): void {
    this.listeners.forEach((listener) => listener());
  }

  private loadFromStorage(): void {
    this.modelSettings = ModelStorage.loadModelSettings();
  }

  private saveToStorage(): void {
    ModelStorage.saveModelSettings(this.modelSettings);
  }

  static loadCurrentModel(): ModelId {
    const stored = ModelStorage.loadCurrentModel();
    
    // Create default model
    const defaultProvider = PROVIDERS[DEFAULT_PROVIDER];
    const defaultModel = ModelId.create(DEFAULT_PROVIDER, defaultProvider.defaultModel);
    
    if (!stored) return defaultModel;

    return ModelId.create(
      (stored.provider as LLMProvider) || defaultModel.provider,
      stored.model || defaultModel.model
    );
  }

  static saveCurrentModel(modelId: ModelId): void {
    ModelStorage.saveCurrentModel(modelId);
  }

  getModelSettings(modelId: ModelId): ModelSettings {
    const key = modelId.toString();
    return this.modelSettings[key] || { ...DEFAULT_SETTINGS };
  }

  updateModelSettings(
    modelId: ModelId,
    settings: Partial<ModelSettings>
  ): void {
    const key = modelId.toString();
    const currentSettings = this.getModelSettings(modelId);

    this.modelSettings[key] = {
      ...currentSettings,
      ...settings,
    };


    this.saveToStorage();
    this.notifyListeners();
  }

  getAvailableModels(hasApiKey: (provider: LLMProvider) => boolean): ModelOption[] {
    return Object.values(PROVIDERS)
      .filter((provider) => hasApiKey(provider.id as LLMProvider))
      .flatMap((provider) =>
        provider.models.map((model) => ({
          id: model.id,
          name: model.name,
          description: model.description,
          providerId: provider.id as LLMProvider,
        }))
      );
  }

  createModel(
    modelId: ModelId,
    apiKey: string,
    hasApiKey: (provider: LLMProvider) => boolean
  ): LanguageModel | null {
    // Validate model configuration
    if (!ModelValidator.isModelValid(modelId, hasApiKey)) {
      return null;
    }

    const settings = this.getModelSettings(modelId);

    const modelConfig = {
      apiKey,
      model: modelId.model,
      temperature: settings.temperature,
      maxTokens: settings.maxTokens,
    };

    return createModel({
      provider: modelId.provider,
      config: modelConfig,
    });
  }

  validateModel(
    modelId: ModelId,
    hasApiKey: (provider: LLMProvider) => boolean
  ): { isValid: boolean; error?: string } {
    const isValid = ModelValidator.isModelValid(modelId, hasApiKey);
    const error = isValid
      ? undefined
      : ModelValidator.getValidationError(modelId, hasApiKey) || undefined;

    return { isValid, error };
  }
}