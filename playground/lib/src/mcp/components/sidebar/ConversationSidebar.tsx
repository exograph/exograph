// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState } from "react";
import { ConversationSection } from "./ConversationSection";
import { MCPServerSection } from "./MCPServerSection";
import { LLMConfigSection } from "./LLMConfigSection";
import { ThreadListPrimitive } from "@assistant-ui/react";
import { Plus } from "lucide-react";

interface ConversationSidebarProps {
  mcpEndpoint: string;
}

export function ConversationSidebar({ mcpEndpoint }: ConversationSidebarProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (collapsed) {
    return (
      <div
        className={`w-12 bg-white dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700`}
      >
        <div className="p-2 space-y-2">
          <button
            onClick={() => setCollapsed(false)}
            className="w-8 h-8 rounded-lg bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 flex items-center justify-center text-gray-600 dark:text-gray-400"
            title="Expand sidebar"
          >
            →
          </button>
          <ThreadListPrimitive.New
            className="w-8 h-8 rounded-lg bg-blue-500 hover:bg-blue-600 text-white flex items-center justify-center"
            title="New conversation"
          >
            <Plus className="w-4 h-4" />
          </ThreadListPrimitive.New>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`w-80 h-full bg-white dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700 flex flex-col shrink-0`}
    >
      <div className="flex items-center justify-end p-4 border-b border-gray-200 dark:border-gray-700">
        <button
          onClick={() => setCollapsed(true)}
          className="text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300"
          title="Collapse sidebar"
        >
          ←
        </button>
      </div>

      <ConversationSection />

      <div className="p-4 border-t border-gray-200 dark:border-gray-700 space-y-3">
        <MCPServerSection mcpEndpoint={mcpEndpoint} />

        <hr className="border-gray-200 dark:border-gray-700" />

        <LLMConfigSection />
      </div>
    </div>
  );
}
