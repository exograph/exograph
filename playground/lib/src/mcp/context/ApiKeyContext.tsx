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
  useState,
  useCallback,
  useMemo,
} from "react";
import { LLMProvider } from "../providers/types";
import type { ApiKeyStorageMap } from "./ApiKeyStorage";
import { ApiKeyStorageUtils } from "./ApiKeyStorage";
import { PROVIDERS } from "../providers/config";

export interface ApiKeyContextValue {
  getApiKey: (provider: LLMProvider) => string;
  setApiKey: (
    provider: LLMProvider,
    apiKey: string,
    storeInLocalStorage: boolean
  ) => void;
  hasApiKey: (provider: LLMProvider) => boolean;
  isStoringInLocalStorage: (provider: LLMProvider) => boolean;
  clearApiKeys: () => void;
}

const ApiKeyContext = createContext<ApiKeyContextValue | undefined>(undefined);

const STORAGE_KEY = "exograph-mcp-playground-api-keys";

// Helper to create empty storage map
const createEmptyStorage = (): ApiKeyStorageMap =>
  Object.fromEntries(
    Object.keys(PROVIDERS).map((p) => [p, ApiKeyStorageUtils.empty()])
  ) as ApiKeyStorageMap;

// Helper to load storage from localStorage
const loadFromLocalStorage = (): ApiKeyStorageMap => {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return createEmptyStorage();

    const parsed = JSON.parse(stored);
    return Object.fromEntries(
      Object.keys(PROVIDERS).map((provider) => [
        provider,
        parsed[provider]
          ? ApiKeyStorageUtils.localStorage(parsed[provider])
          : ApiKeyStorageUtils.empty(),
      ])
    ) as ApiKeyStorageMap;
  } catch (error) {
    console.error("Error loading API keys:", error);
    return createEmptyStorage();
  }
};

// Helper to save to localStorage
const persistToLocalStorage = (storage: ApiKeyStorageMap): void => {
  try {
    const keysToStore: Record<string, string> = {};
    
    Object.entries(storage).forEach(([provider, value]) => {
      if (ApiKeyStorageUtils.isStoredInLocalStorage(value)) {
        keysToStore[provider] = ApiKeyStorageUtils.getApiKey(value);
      }
    });

    if (Object.keys(keysToStore).length > 0) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(keysToStore));
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch (error) {
    console.error("Error saving API keys:", error);
  }
};

export interface ApiKeyProviderProps {
  children: React.ReactNode;
}

export function ApiKeyProvider({ children }: ApiKeyProviderProps) {
  const [apiKeyStorage, setApiKeyStorage] = useState<ApiKeyStorageMap>(() =>
    loadFromLocalStorage()
  );

  const setApiKey = useCallback(
    (provider: LLMProvider, apiKey: string, storeInLocalStorage: boolean) => {
      setApiKeyStorage((current) => {
        const updated = {
          ...current,
          [provider]: ApiKeyStorageUtils.setApiKey(apiKey, storeInLocalStorage),
        };
        persistToLocalStorage(updated);
        return updated;
      });
    },
    []
  );

  const clearApiKeys = useCallback(() => {
    const empty = createEmptyStorage();
    setApiKeyStorage(empty);
    localStorage.removeItem(STORAGE_KEY);
  }, []);

  // Memoize the context value to avoid unnecessary re-renders
  const value = useMemo<ApiKeyContextValue>(
    () => ({
      getApiKey: (provider) => ApiKeyStorageUtils.getApiKey(apiKeyStorage[provider]),
      setApiKey,
      hasApiKey: (provider) => ApiKeyStorageUtils.hasApiKey(apiKeyStorage[provider]),
      isStoringInLocalStorage: (provider) =>
        ApiKeyStorageUtils.isStoredInLocalStorage(apiKeyStorage[provider]),
      clearApiKeys,
    }),
    [apiKeyStorage, setApiKey, clearApiKeys]
  );

  return (
    <ApiKeyContext.Provider value={value}>{children}</ApiKeyContext.Provider>
  );
}

export const useApiKeys = (): ApiKeyContextValue => {
  const context = useContext(ApiKeyContext);
  if (!context) {
    throw new Error("useApiKeys must be used within an ApiKeyProvider");
  }
  return context;
};