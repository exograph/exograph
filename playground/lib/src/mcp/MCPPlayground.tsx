// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { PlaygroundMCPProps } from "./types";
import { BasePlaygroundComponentProps } from "../util/component-types";
import { ConversationSidebar } from "./components/ConversationSidebar";
import { ChatInterface } from "./components/ChatInterface";
import { useConversations } from "./context/ConversationContext";
import { ChatConfigProvider, useChatConfig } from "./context/ChatConfigContext";
import { ApiKeyProvider } from "./context/ApiKeyContext";
import { ConversationProvider } from "./context/ConversationContext";
import { ModelProvider } from "./context/ModelContext";

export interface MCPPlaygroundProps
  extends BasePlaygroundComponentProps<PlaygroundMCPProps> {
  // This interface intentionally extends the base props without additional members
}

// Inner component that uses the context
function MCPPlaygroundInner({ mcp }: { mcp: PlaygroundMCPProps }) {
  const { loading: configLoading } = useChatConfig();
  const { loading: conversationsLoading } = useConversations();

  if (conversationsLoading || configLoading) {
    return (
      <div className="flex items-center justify-center h-full bg-white dark:bg-gray-800">
        <div className="text-gray-600 dark:text-gray-300">
          Loading conversations...
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full bg-gray-100 dark:bg-gray-900">
      <ConversationSidebar mcpEndpoint={mcp.mcpHttpPath} />

      <div className="flex-1 bg-white dark:bg-gray-800 min-h-0 overflow-hidden">
        <ChatInterface />
      </div>
    </div>
  );
}

// Main component wrapped with providers
export function MCPPlayground({ tab: mcp, auth: _auth }: MCPPlaygroundProps) {
  return (
    <ApiKeyProvider>
      <ChatConfigProvider>
        <ModelProvider>
          <ConversationProvider>
            <MCPPlaygroundInner mcp={mcp} />
          </ConversationProvider>
        </ModelProvider>
      </ChatConfigProvider>
    </ApiKeyProvider>
  );
}
