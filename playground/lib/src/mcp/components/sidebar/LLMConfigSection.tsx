// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState } from "react";
import { Settings } from "lucide-react";
import { useCurrentModel } from "../../context/ModelContext";
import { ModelId } from "../../providers/ModelId";
import { useModelSettings } from "../../context/ModelContext";
import { ProviderConfigurationModal } from "../ProviderConfigurationModal";

export function LLMConfigSection() {
  const { availableModels } = useModelSettings();
  const { currentModel: config, setModel } = useCurrentModel();
  const [showConfigModal, setShowConfigModal] = useState(false);

  const currentModelIndex = availableModels.findIndex(
    (model) => model.providerId === config.provider && model.id === config.model
  );

  const handleModelChange = (event: React.ChangeEvent<HTMLSelectElement>) => {
    const selectedIndex = parseInt(event.target.value, 10);
    const selectedModel = availableModels[selectedIndex];
    if (selectedModel) {
      setModel(ModelId.create(selectedModel.providerId, selectedModel.id));
    }
  };

  return (
    <>
      <div className="text-xs text-gray-500 dark:text-gray-400">
        <div className="font-medium mb-1 flex items-center justify-between">
          <span>LLM Configuration</span>
          <button
            onClick={() => setShowConfigModal(true)}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            <Settings className="w-4 h-4" />
          </button>
        </div>

        {availableModels.length === 0 ? (
          <div className="text-gray-400 dark:text-gray-500 mb-2">
            No providers have their API keys configured
          </div>
        ) : (
          <select
            value={currentModelIndex >= 0 ? currentModelIndex.toString() : ""}
            onChange={handleModelChange}
            className="w-full text-xs bg-gray-50 dark:bg-gray-800 hover:bg-gray-100 dark:hover:bg-gray-700 p-2 rounded-md border border-gray-200 dark:border-gray-600 text-gray-700 dark:text-gray-300 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          >
            {availableModels.map((modelOption, index) => (
              <option key={index} value={index.toString()}>
                {modelOption.name}
              </option>
            ))}
          </select>
        )}
      </div>

      <ProviderConfigurationModal
        open={showConfigModal}
        onOpenChange={setShowConfigModal}
      />
    </>
  );
}
