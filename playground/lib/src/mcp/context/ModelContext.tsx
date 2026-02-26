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
  useEffect,
  useCallback,
  useMemo,
} from "react";
import { ModelId, LLMProvider } from "../providers/ModelId";
import { PROVIDERS } from "../providers/config";
import { useProviderConfig } from "./ProviderConfigContext";
import { ModelAPI } from "../api";
import type { ModelSettings, ModelOption } from "../api/model/types";

export interface ModelSettingsContextValue {
  getModelSettings: (modelId: ModelId) => ModelSettings;
  updateModelSettings: (
    modelId: ModelId,
    settings: Partial<ModelSettings>
  ) => void;
  availableModels: ModelOption[];
}

export interface CurrentModelContextValue {
  currentModel: ModelId;
  setModel: (modelId: ModelId) => void;
  isConfigValid: boolean;
}

type ModelContextValue = CurrentModelContextValue & ModelSettingsContextValue;

const ModelContext = createContext<ModelContextValue | undefined>(undefined);

export interface ModelProviderProps {
  children: React.ReactNode;
}

export function ModelProvider({ children }: ModelProviderProps) {
  const [currentModel, setCurrentModel] = useState<ModelId>(() =>
    ModelAPI.loadCurrentModel()
  );
  const [modelAPI] = useState(() => new ModelAPI());
  const [updateCounter, setUpdateCounter] = useState(0);
  const { getApiKey } = useProviderConfig();

  useEffect(() => {
    const cleanup = modelAPI.addChangeListener(() => {
      setUpdateCounter((prev) => prev + 1);
    });
    return cleanup;
  }, [modelAPI]);

  // Save current model whenever it changes
  useEffect(() => {
    ModelAPI.saveCurrentModel(currentModel);
  }, [currentModel]);

  // Auto-select first valid model when API keys change
  useEffect(() => {
    // If current model is invalid, try to find the first valid one
    if (!modelAPI.validateModel(currentModel, getApiKey).isValid) {
      // Find first provider with API key and use its default model
      const firstValidProvider = Object.entries(PROVIDERS).find(([providerId, provider]) => {
        return !provider.requiresApiKey || getApiKey(providerId as LLMProvider) !== undefined;
      });

      if (firstValidProvider) {
        const [providerId, provider] = firstValidProvider;
        const defaultModel = new ModelId(providerId as LLMProvider, provider.defaultModel);
        setCurrentModel(defaultModel);
      }
    }
  }, [getApiKey, modelAPI, currentModel]);

  const setModel = useCallback(
    (modelId: ModelId) => {
      // Always allow model selection - validation only affects whether it can be used
      setCurrentModel(modelId);
    },
    []
  );

  const isConfigValid = Boolean(
    modelAPI.validateModel(currentModel, getApiKey).isValid
  );

  const getModelSettings = useCallback(
    (modelId: ModelId): ModelSettings => modelAPI.getModelSettings(modelId),
    [modelAPI]
  );

  const updateModelSettings = useCallback(
    (modelId: ModelId, settings: Partial<ModelSettings>) => {
      modelAPI.updateModelSettings(modelId, settings);
    },
    [modelAPI]
  );

  const availableModels = useMemo((): ModelOption[] => {
    return modelAPI.getAvailableModels(getApiKey);
  }, [modelAPI, getApiKey, updateCounter]);

  const value: ModelContextValue = useMemo(
    () => ({
      currentModel,
      setModel,
      isConfigValid,
      getModelSettings,
      updateModelSettings,
      availableModels,
    }),
    [
      currentModel,
      setModel,
      isConfigValid,
      getModelSettings,
      updateModelSettings,
      availableModels,
    ]
  );

  return (
    <ModelContext.Provider value={value}>{children}</ModelContext.Provider>
  );
}

export const useCurrentModel = (): CurrentModelContextValue => {
  const context = useContext(ModelContext);
  if (context === undefined) {
    throw new Error("useCurrentModel must be used within a ModelProvider");
  }
  const { currentModel, setModel, isConfigValid } = context;
  return { currentModel, setModel, isConfigValid };
};

export const useModelSettings = (): ModelSettingsContextValue => {
  const context = useContext(ModelContext);
  if (context === undefined) {
    throw new Error("useModelSettings must be used within a ModelProvider");
  }
  const { getModelSettings, updateModelSettings, availableModels } = context;
  return { getModelSettings, updateModelSettings, availableModels };
};

