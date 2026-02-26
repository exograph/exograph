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
  useMemo,
  useEffect,
} from "react";
import { LLMProvider } from "../providers/ModelId";
import { ProviderAPI } from "../api";

export interface ProviderConfigContextValue {
  getApiKey: (provider: LLMProvider) => string | undefined;
  setApiKey: (
    provider: LLMProvider,
    apiKey: string | undefined,
    storeInLocalStorage: boolean
  ) => void;
  isStoringInLocalStorage: (provider: LLMProvider) => boolean;
}

const ProviderConfigContext = createContext<
  ProviderConfigContextValue | undefined
>(undefined);

export interface ProviderConfigProviderProps {
  children: React.ReactNode;
}

export function ProviderConfigProvider({
  children,
}: ProviderConfigProviderProps) {
  const [providerAPI] = useState(() => new ProviderAPI());
  const [updateCounter, setUpdateCounter] = useState(0);

  // Set up change listener on mount
  useEffect(() => {
    const cleanup = providerAPI.addChangeListener(() => {
      setUpdateCounter((prev) => prev + 1);
    });
    return cleanup;
  }, [providerAPI]);

  // Memoize the context value to avoid unnecessary re-renders
  const value = useMemo<ProviderConfigContextValue>(
    () => ({
      getApiKey: (provider) => providerAPI.getApiKey(provider),
      setApiKey: (provider, apiKey, storeInLocalStorage) => {
        providerAPI.setApiKey(provider, apiKey, storeInLocalStorage);
      },
      isStoringInLocalStorage: (provider) =>
        providerAPI.isStoringInLocalStorage(provider),
    }),
    [providerAPI, updateCounter]
  );

  return (
    <ProviderConfigContext.Provider value={value}>
      {children}
    </ProviderConfigContext.Provider>
  );
}

export const useProviderConfig = (): ProviderConfigContextValue => {
  const context = useContext(ProviderConfigContext);
  if (!context) {
    throw new Error(
      "useProviderConfig must be used within a ProviderConfigProvider"
    );
  }
  return context;
};
