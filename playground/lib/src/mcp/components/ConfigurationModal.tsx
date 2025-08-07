// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState, useEffect } from "react";
import { Dialog } from "../../util/Dialog";
import { ProviderSelector } from "./ProviderSelector";
import { useChatConfig } from "../context/ChatConfigContext";
import { ChatConfig } from "../providers/types";

interface ConfigurationModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ConfigurationModal({
  open,
  onOpenChange,
}: ConfigurationModalProps) {
  const { config: contextConfig, updateConfig } = useChatConfig();
  const [localConfig, setLocalConfig] = useState<ChatConfig>(contextConfig);

  // When modal opens, initialize local config with context config
  useEffect(() => {
    if (open) setLocalConfig(contextConfig);
  }, [open, contextConfig]);

  const handleConfigChange = (updates: Partial<ChatConfig>) => {
    setLocalConfig((prev) => ({ ...prev, ...updates }));
  };

  const handleSave = () => {
    updateConfig(localConfig);
    onOpenChange(false);
  };

  const closeModal = () => onOpenChange(false);

  return (
    <Dialog
      open={open}
      onOpenChange={closeModal}
      title="AI Provider Configuration"
      size="lg"
    >
      <ProviderSelector config={localConfig} onChange={handleConfigChange} />

      <div className="mt-6 pt-4 border-t border-gray-200 dark:border-gray-700">
        <div className="flex justify-end gap-3">
          <button
            onClick={closeModal}
            className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-500 rounded-lg hover:bg-blue-600 transition-colors"
          >
            Save Configuration
          </button>
        </div>
      </div>
    </Dialog>
  );
}
