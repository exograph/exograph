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
} from "react";
import { ModelId } from "../providers/ModelId";
import { PROVIDERS } from "../providers/config";
import { useProviderConfig } from "./ProviderConfigContext";
import { ModelAPI } from "../api";

export interface CurrentModelContextValue {
  currentModel: ModelId;
  setModel: (modelId: ModelId) => void;
  isConfigValid: boolean;
}

const CurrentModelContext = createContext<CurrentModelContextValue | undefined>(
  undefined
);

export interface CurrentModelProviderProps {
  children: React.ReactNode;
}

export function CurrentModelProvider({ children }: CurrentModelProviderProps) {
  const [currentModel, setCurrentModel] = useState<ModelId>(() => ModelAPI.loadCurrentModel());
  const [modelAPI] = useState(() => new ModelAPI());
  const { hasApiKey } = useProviderConfig();

  // Save current model whenever it changes
  useEffect(() => {
    ModelAPI.saveCurrentModel(currentModel);
  }, [currentModel]);

  // Auto-select first valid model when API keys change
  useEffect(() => {
    // If current model is invalid, try to find the first valid one
    if (!modelAPI.validateModel(currentModel, hasApiKey).isValid) {
      // Find first provider with API key and use its default model
      const firstValidProvider = Object.entries(PROVIDERS).find(([providerId, provider]) => {
        return !provider.requiresApiKey || hasApiKey(providerId as any);
      });

      if (firstValidProvider) {
        const [providerId, provider] = firstValidProvider;
        const defaultModel = new ModelId(providerId as any, provider.defaultModel);
        setCurrentModel(defaultModel);
      }
    }
  }, [hasApiKey, modelAPI, currentModel]);

  const setModel = useCallback(
    (modelId: ModelId) => {
      // Always allow model selection - validation only affects whether it can be used
      setCurrentModel(modelId);
    },
    []
  );

  const isConfigValid = Boolean(
    modelAPI.validateModel(currentModel, hasApiKey).isValid
  );

  const value: CurrentModelContextValue = {
    currentModel,
    setModel,
    isConfigValid,
  };

  return (
    <CurrentModelContext.Provider value={value}>
      {children}
    </CurrentModelContext.Provider>
  );
}

export const useCurrentModel = (): CurrentModelContextValue => {
  const context = useContext(CurrentModelContext);
  if (context === undefined) {
    throw new Error("useCurrentModel must be used within a CurrentModelProvider");
  }
  return context;
};

