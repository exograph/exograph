// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { ModelId, LLMProvider } from '../../providers/ModelId';
import { PROVIDERS } from '../../providers/config';

export class ModelValidator {
  static isModelValid(modelId: ModelId, hasApiKey: (provider: LLMProvider) => boolean): boolean {
    const provider = PROVIDERS[modelId.provider];
    if (!provider) return false;

    // Check if model exists in provider
    const modelExists = provider.models.some((m) => m.id === modelId.model);
    if (!modelExists) return false;

    // Check API key requirement
    if (provider.requiresApiKey && !hasApiKey(modelId.provider)) {
      return false;
    }

    return true;
  }

  static isProviderValid(provider: LLMProvider): boolean {
    return Boolean(PROVIDERS[provider]);
  }

  static getValidationError(
    modelId: ModelId,
    hasApiKey: (provider: LLMProvider) => boolean
  ): string | null {
    const provider = PROVIDERS[modelId.provider];
    if (!provider) {
      return `Unknown provider: ${modelId.provider}`;
    }

    const modelExists = provider.models.some((m) => m.id === modelId.model);
    if (!modelExists) {
      return `Model '${modelId.model}' not found for provider '${provider.displayName}'`;
    }

    if (provider.requiresApiKey && !hasApiKey(modelId.provider)) {
      return `API key required for ${provider.displayName}`;
    }

    return null;
  }
}