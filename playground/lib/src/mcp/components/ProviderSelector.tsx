// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// React import removed as it's not needed
import { ChatConfig, LLMProvider } from '../providers/types';
import { PROVIDERS } from '../providers/config';
import { useApiKeys } from '../context/ApiKeyContext';
import { Tooltip } from '../../util/Tooltip';

interface ProviderSelectorProps {
  config: ChatConfig;
  onChange: (updates: Partial<ChatConfig>) => void;
  className?: string;
}

export function ProviderSelector({ 
  config, 
  onChange, 
  className = '' 
}: ProviderSelectorProps) {
  const currentProvider = PROVIDERS[config.provider];
  const { getApiKey, setApiKey, hasApiKey, isStoringInLocalStorage } = useApiKeys();
  
  const setProvider = (provider: LLMProvider) => {
    const providerInfo = PROVIDERS[provider];
    if (providerInfo) {
      onChange({
        provider,
        model: providerInfo.defaultModel,
      });
    }
  };

  const setModel = (model: string) => {
    onChange({ model });
  };


  const setTemperature = (temperature: number) => {
    onChange({ temperature });
  };

  const setMaxTokens = (maxTokens: number) => {
    onChange({ maxTokens });
  };

  const isConfigValid = Boolean(
    config.provider &&
    config.model &&
    (PROVIDERS[config.provider]?.requiresApiKey ? hasApiKey(config.provider) : true)
  );

  return (
    <div className={`space-y-4 ${className}`}>

      <div className="grid grid-cols-3 gap-2">
        {Object.values(PROVIDERS).map((provider) => (
          <button
            key={provider.id}
            onClick={() => setProvider(provider.id)}
            className={`
              p-3 rounded-lg border-2 text-sm font-medium transition-all duration-200
              ${
                config.provider === provider.id
                  ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300'
                  : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'
              }
            `}
          >
            <div className="font-medium">{provider.displayName}</div>
            <div className="text-xs opacity-75 mt-1">
              {provider.models.length} models
            </div>
          </button>
        ))}
      </div>

      <div className="space-y-4 p-4 bg-gray-50 dark:bg-gray-900 rounded-lg">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Model
            </label>
            <select
              value={config.model}
              onChange={(e) => setModel(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            >
              {currentProvider?.models.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name} {model.description && `- ${model.description}`}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              API Key for {currentProvider?.displayName}
            </label>
            <input
              type="password"
              value={getApiKey(config.provider)}
              onChange={(e) => setApiKey(config.provider, e.target.value, isStoringInLocalStorage(config.provider))}
              placeholder={`Enter your ${currentProvider?.displayName} API key`}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
            <div className="flex items-start gap-3 mt-3">
              <input
                type="checkbox"
                id="store-api-keys"
                checked={isStoringInLocalStorage(config.provider)}
                onChange={(e) => setApiKey(config.provider, getApiKey(config.provider), e.target.checked)}
                disabled={!getApiKey(config.provider)}
                className="mt-0.5 h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
              />
              <div>
                <div className="flex items-center gap-1">
                  <label htmlFor="store-api-keys" className="text-sm font-medium text-gray-700 dark:text-gray-300 cursor-pointer">
                    Remember API key in browser storage
                  </label>
                  <Tooltip
                    content={isStoringInLocalStorage(config.provider)
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

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                Temperature ({config.temperature})
              </label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={config.temperature}
                onChange={(e) => setTemperature(parseFloat(e.target.value))}
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
                min="1"
                max="8192"
                value={config.maxTokens}
                onChange={(e) => setMaxTokens(parseInt(e.target.value))}
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
          </div>

          <div className={`p-3 rounded-md ${isConfigValid ? 'bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200' : 'bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200'}`}>
            <div className="flex items-center gap-2">
              <div className={`w-2 h-2 rounded-full ${isConfigValid ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className="text-sm font-medium">
                {isConfigValid ? 'Configuration Valid' : 'Configuration Required'}
              </span>
            </div>
            {!isConfigValid && (
              <div className="text-xs mt-1">
                Please provide an API key for {currentProvider?.displayName}
              </div>
            )}
          </div>
        </div>
    </div>
  );
}