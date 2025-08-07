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
import { ChatConfig, LLMProvider } from "../providers/types";
import {
  PROVIDERS,
  getDefaultConfig,
  DEFAULT_PROVIDER,
} from "../providers/config";

export interface ChatConfigContextValue {
  config: ChatConfig;
  updateConfig: (updates: Partial<ChatConfig> | ChatConfig) => void;
  setProvider: (provider: LLMProvider) => void;
  setModel: (model: string) => void;
  setTemperature: (temperature: number) => void;
  setMaxTokens: (maxTokens: number) => void;
  availableModels: string[];
  loading: boolean;
}

const ChatConfigContext = createContext<ChatConfigContextValue | undefined>(
  undefined
);

const STORAGE_KEY = "exograph-mcp-playground-chat-config";

const loadChatConfig = (): ChatConfig => {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return getDefaultConfig(DEFAULT_PROVIDER) as ChatConfig;

    const parsed = JSON.parse(stored);
    const defaultConfig = getDefaultConfig(DEFAULT_PROVIDER) as ChatConfig;
    return {
      provider: parsed.provider || defaultConfig.provider,
      model: parsed.model || defaultConfig.model,
      temperature: parsed.temperature ?? defaultConfig.temperature,
      maxTokens: parsed.maxTokens ?? defaultConfig.maxTokens,
    };
  } catch (error) {
    console.error("Error loading chat config:", error);
    return getDefaultConfig(DEFAULT_PROVIDER) as ChatConfig;
  }
};

const saveChatConfig = (config: ChatConfig): void => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  } catch (error) {
    console.error("Error saving chat config:", error);
  }
};

export interface ChatConfigProviderProps {
  children: React.ReactNode;
}

export function ChatConfigProvider({ children }: ChatConfigProviderProps) {
  const [config, setConfig] = useState<ChatConfig>(() => loadChatConfig());
  const loading = false; // No loading needed since we load synchronously

  // Save config whenever it changes
  useEffect(() => {
    saveChatConfig(config);
  }, [config]);

  const updateConfig = useCallback(
    (updates: Partial<ChatConfig> | ChatConfig) => {
      const newConfig = { ...config, ...updates };
      setConfig(newConfig);
    },
    [config]
  );

  const setProvider = useCallback(
    (provider: LLMProvider) => {
      const providerInfo = PROVIDERS[provider];
      if (providerInfo) {
        updateConfig({
          provider,
          model: providerInfo.defaultModel,
        });
      }
    },
    [updateConfig]
  );

  const setModel = useCallback(
    (model: string) => {
      updateConfig({ model });
    },
    [updateConfig]
  );

  const setTemperature = useCallback(
    (temperature: number) => {
      updateConfig({ temperature });
    },
    [updateConfig]
  );

  const setMaxTokens = useCallback(
    (maxTokens: number) => {
      updateConfig({ maxTokens });
    },
    [updateConfig]
  );

  const availableModels =
    PROVIDERS[config.provider]?.models.map((m) => m.id) || [];

  const value: ChatConfigContextValue = {
    config,
    updateConfig,
    setProvider,
    setModel,
    setTemperature,
    setMaxTokens,
    availableModels,
    loading,
  };

  return (
    <ChatConfigContext.Provider value={value}>
      {children}
    </ChatConfigContext.Provider>
  );
}

export const useChatConfig = (): ChatConfigContextValue => {
  const context = useContext(ChatConfigContext);
  if (context === undefined) {
    throw new Error("useChatConfig must be used within a ChatConfigProvider");
  }
  return context;
};
