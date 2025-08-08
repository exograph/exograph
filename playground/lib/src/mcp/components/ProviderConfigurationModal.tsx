// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState } from "react";
import { Dialog } from "../../util/Dialog";
import { LLMProvider, ModelId } from "../providers/ModelId";
import { PROVIDERS } from "../providers/config";
import { useProviderConfig } from "../context/ProviderConfigContext";
import { useModelSettings } from "../context/ModelSettingsContext";
import { Tooltip } from "../../util/Tooltip";

// Constants for model settings limits
const TEMPERATURE_MIN = 0;
const TEMPERATURE_MAX = 2;
const TEMPERATURE_STEP = 0.1;
const MAX_TOKENS_MIN = 1;
const MAX_TOKENS_MAX = 8192;

interface ProviderConfigurationModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface ModelSettingsProps {
  modelId: ModelId;
}

function ModelSettings({ modelId }: ModelSettingsProps) {
  const { getModelSettings, updateModelSettings } = useModelSettings();
  const settings = getModelSettings(modelId);

  const handleTemperatureChange = (temperature: number) => {
    updateModelSettings(modelId, { temperature });
  };

  const handleMaxTokensChange = (maxTokens: number) => {
    updateModelSettings(modelId, { maxTokens });
  };

  return (
    <div className="space-y-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
      <div className="text-sm font-medium text-blue-700 dark:text-blue-300">
        Settings for {modelId.model}
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Temperature ({settings.temperature})
          </label>
          <input
            type="range"
            min={TEMPERATURE_MIN}
            max={TEMPERATURE_MAX}
            step={TEMPERATURE_STEP}
            value={settings.temperature}
            onChange={(e) =>
              handleTemperatureChange(parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
            <span>Focused</span>
            <span>Creative</span>
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Max Tokens
          </label>
          <input
            type="number"
            min={MAX_TOKENS_MIN}
            max={MAX_TOKENS_MAX}
            value={settings.maxTokens}
            onChange={(e) => handleMaxTokensChange(parseInt(e.target.value))}
            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
        </div>
      </div>
    </div>
  );
}

export function ProviderConfigurationModal({
  open,
  onOpenChange,
}: ProviderConfigurationModalProps) {
  const [selectedProvider, setSelectedProvider] = useState<LLMProvider>("openai");
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  
  const currentProvider = PROVIDERS[selectedProvider];
  const { getApiKey, setApiKey, hasApiKey, isStoringInLocalStorage } = useProviderConfig();

  const handleProviderChange = (provider: LLMProvider) => {
    setSelectedProvider(provider);
    setSelectedModel(null); // Reset model selection when switching providers
  };

  const isConfigValid = Boolean(
    selectedProvider &&
      (PROVIDERS[selectedProvider]?.requiresApiKey
        ? hasApiKey(selectedProvider)
        : true)
  );

  const closeModal = () => onOpenChange(false);

  return (
    <Dialog
      open={open}
      onOpenChange={closeModal}
      title="Settings"
      size="lg"
    >
      <div className="space-y-4">
        <div className="text-sm text-gray-600 dark:text-gray-400 mb-4">
          Configure API keys for each provider and adjust settings for individual
          models.
        </div>

        {/* Provider Selection */}
        <div className="grid grid-cols-3 gap-2">
          {Object.values(PROVIDERS).map((provider) => (
            <button
              key={provider.id}
              onClick={() => handleProviderChange(provider.id)}
              className={`
                p-3 rounded-lg border-2 text-sm font-medium transition-all duration-200 relative
                ${
                  selectedProvider === provider.id
                    ? "border-blue-500 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300"
                    : "border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600"
                }
              `}
            >
              <div className="font-medium">{provider.displayName}</div>
              <div className="text-xs opacity-75 mt-1">
                {provider.models.length} models
              </div>
              {hasApiKey(provider.id) && (
                <div className="absolute top-1 right-1 w-2 h-2 bg-green-500 rounded-full"></div>
              )}
            </button>
          ))}
        </div>

        {/* API Key Configuration */}
        <div className="space-y-4 p-4 bg-gray-50 dark:bg-gray-900 rounded-lg">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              API Key for {currentProvider?.displayName}
            </label>
            <input
              type="password"
              value={getApiKey(selectedProvider)}
              onChange={(e) =>
                setApiKey(
                  selectedProvider,
                  e.target.value,
                  isStoringInLocalStorage(selectedProvider)
                )
              }
              placeholder={`Enter your ${currentProvider?.displayName} API key`}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
            <div className="flex items-start gap-3 mt-3">
              <input
                type="checkbox"
                id="store-api-keys"
                checked={isStoringInLocalStorage(selectedProvider)}
                onChange={(e) =>
                  setApiKey(
                    selectedProvider,
                    getApiKey(selectedProvider),
                    e.target.checked
                  )
                }
                disabled={!getApiKey(selectedProvider)}
                className="mt-0.5 h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
              />
              <div>
                <div className="flex items-center gap-1">
                  <label
                    htmlFor="store-api-keys"
                    className="text-sm font-medium text-gray-700 dark:text-gray-300 cursor-pointer"
                  >
                    Remember API key in browser storage
                  </label>
                  <Tooltip
                    content={
                      isStoringInLocalStorage(selectedProvider)
                        ? "API key will be saved locally in your browser. Uncheck to keep it only for this session."
                        : "API key will only be kept for this session. Check to save it in your browser."
                    }
                    position="bottom"
                    size="lg"
                  >
                    <button
                      type="button"
                      className="w-4 h-4 rounded-full border border-gray-400 dark:border-gray-500 text-gray-500 dark:text-gray-400 flex items-center justify-center text-xs font-medium cursor-pointer hover:border-gray-600 dark:hover:border-gray-300 transition-colors"
                    >
                      ?
                    </button>
                  </Tooltip>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Model-specific Settings */}
        {isConfigValid && (
          <div className="space-y-4 p-4 bg-gray-50 dark:bg-gray-900 rounded-lg">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                Model Settings
              </label>
              <select
                value={selectedModel || ""}
                onChange={(e) => setSelectedModel(e.target.value || null)}
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              >
                <option value="">Select a model to configure...</option>
                {currentProvider?.models.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name}
                  </option>
                ))}
              </select>
            </div>

            {selectedModel && (
              <ModelSettings modelId={ModelId.create(selectedProvider, selectedModel)} />
            )}
          </div>
        )}
      </div>

      <div className="mt-6 pt-4 border-t border-gray-200 dark:border-gray-700">
        <div className="flex justify-end">
          <button
            onClick={closeModal}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-500 rounded-lg hover:bg-blue-600 transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </Dialog>
  );
}
