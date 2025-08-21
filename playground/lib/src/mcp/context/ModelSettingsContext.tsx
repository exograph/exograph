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
  useEffect,
} from "react";
import { LLMProvider, ModelId } from "../providers/ModelId";
import { useProviderConfig } from "./ProviderConfigContext";
import { ModelAPI } from "../api";

export interface ModelSettings {
  temperature: number;
  maxTokens: number;
}

export interface ModelOption {
  id: string;
  name: string;
  description?: string;
  providerId: LLMProvider;
}

export interface ModelSettingsContextValue {
  getModelSettings: (modelId: ModelId) => ModelSettings;
  updateModelSettings: (
    modelId: ModelId,
    settings: Partial<ModelSettings>
  ) => void;
  availableModels: ModelOption[];
}

const ModelSettingsContext = createContext<
  ModelSettingsContextValue | undefined
>(undefined);

export interface ModelSettingsProviderProps {
  children: React.ReactNode;
}

export function ModelConfigProvider({ children }: ModelSettingsProviderProps) {
  const { hasApiKey } = useProviderConfig();
  const [modelAPI] = useState(() => new ModelAPI());
  const [updateCounter, setUpdateCounter] = useState(0);

  // Set up change listener on mount
  useEffect(() => {
    const cleanup = modelAPI.addChangeListener(() => {
      setUpdateCounter(prev => prev + 1);
    });
    return cleanup;
  }, [modelAPI]);

  const getModelSettings = useCallback(
    (modelId: ModelId): ModelSettings => {
      return modelAPI.getModelSettings(modelId);
    },
    [modelAPI]
  );

  const updateModelSettings = useCallback(
    (
      modelId: ModelId,
      settings: Partial<ModelSettings>
    ) => {
      modelAPI.updateModelSettings(modelId, settings);
    },
    [modelAPI]
  );

  // Get all available models from providers with API keys
  const availableModels = useMemo((): ModelOption[] => {
    return modelAPI.getAvailableModels(hasApiKey);
  }, [modelAPI, hasApiKey, updateCounter]);

  const value: ModelSettingsContextValue = {
    getModelSettings,
    updateModelSettings,
    availableModels,
  };

  return (
    <ModelSettingsContext.Provider value={value}>
      {children}
    </ModelSettingsContext.Provider>
  );
}

export const useModelSettings = (): ModelSettingsContextValue => {
  const context = useContext(ModelSettingsContext);
  if (context === undefined) {
    throw new Error(
      "useModelSettings must be used within a ModelSettingsProvider"
    );
  }
  return context;
};
