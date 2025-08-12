// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { LLMProvider } from '../../providers/ModelId';
import { PROVIDERS } from '../../providers/config';
import { ApiKeyStorageMap, ApiKeyStorageUtils } from '../../context/ApiKeyStorage';
import { ProviderStorage } from '../storage/ProviderStorage';

export class ProviderAPI {
  private apiKeys: ApiKeyStorageMap;
  private listeners: Set<() => void> = new Set();

  constructor() {
    this.apiKeys = this.loadApiKeys();
  }

  addChangeListener(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notifyListeners(): void {
    this.listeners.forEach((listener) => listener());
  }

  private loadApiKeys(): ApiKeyStorageMap {
    const stored = ProviderStorage.loadApiKeys();

    return Object.fromEntries(
      Object.keys(PROVIDERS).map((provider) => [
        provider,
        stored[provider as LLMProvider]
          ? ApiKeyStorageUtils.localStorage(stored[provider as LLMProvider])
          : ApiKeyStorageUtils.empty(),
      ])
    ) as ApiKeyStorageMap;
  }

  private saveStorageToLocalStorage(): void {
    const keysToStore = this.getKeysForPersistence();
    ProviderStorage.saveApiKeys(keysToStore as Record<LLMProvider, string>);
  }

  private getKeysForPersistence(): Record<string, string> {
    return Object.fromEntries(
      Object.entries(this.apiKeys)
        .filter(([, value]) => ApiKeyStorageUtils.isStoredInLocalStorage(value))
        .map(([provider, value]) => [provider, ApiKeyStorageUtils.getApiKey(value)])
    );
  }

  getApiKey(provider: LLMProvider): string {
    return ApiKeyStorageUtils.getApiKey(this.apiKeys[provider] || ApiKeyStorageUtils.empty());
  }

  setApiKey(provider: LLMProvider, apiKey: string, storeInLocalStorage: boolean): void {
    this.apiKeys = {
      ...this.apiKeys,
      [provider]: ApiKeyStorageUtils.setApiKey(apiKey, storeInLocalStorage),
    };
    this.saveStorageToLocalStorage();
    this.notifyListeners();
  }

  hasApiKey(provider: LLMProvider): boolean {
    return ApiKeyStorageUtils.hasApiKey(this.apiKeys[provider] || ApiKeyStorageUtils.empty());
  }

  isStoringInLocalStorage(provider: LLMProvider): boolean {
    return ApiKeyStorageUtils.isStoredInLocalStorage(
      this.apiKeys[provider] || ApiKeyStorageUtils.empty()
    );
  }
}