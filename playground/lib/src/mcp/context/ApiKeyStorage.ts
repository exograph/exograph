// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { LLMProvider } from "../providers/types";

export type ApiKeyStorage =
  | { type: 'empty' }
  | { type: 'session'; apiKey: string }
  | { type: 'localStorage'; apiKey: string };

export type ApiKeyStorageMap = Record<LLMProvider, ApiKeyStorage>;

export const ApiKeyStorageUtils = {
  empty(): ApiKeyStorage {
    return { type: 'empty' };
  },

  session(apiKey: string): ApiKeyStorage {
    return { type: 'session', apiKey };
  },

  localStorage(apiKey: string): ApiKeyStorage {
    return { type: 'localStorage', apiKey };
  },

  getApiKey(storage: ApiKeyStorage): string {
    return storage.type === 'empty' ? '' : storage.apiKey;
  },

  hasApiKey(storage: ApiKeyStorage): boolean {
    return storage.type !== 'empty';
  },

  isStoredInLocalStorage(storage: ApiKeyStorage): boolean {
    return storage.type === 'localStorage';
  },

  setApiKey(apiKey: string, storeInLocalStorage: boolean): ApiKeyStorage {
    if (!apiKey.trim()) {
      return ApiKeyStorageUtils.empty();
    }

    return storeInLocalStorage
      ? ApiKeyStorageUtils.localStorage(apiKey)
      : ApiKeyStorageUtils.session(apiKey);
  }
};