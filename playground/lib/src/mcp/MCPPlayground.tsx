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
import { ConversationSidebar } from "./components/sidebar/ConversationSidebar";
import { Thread } from "./components/chat/Thread";
import { ModelProvider } from "./context/ModelContext";
import { ProviderConfigProvider } from "./context/ProviderConfigContext";
import { MCPClientProvider } from "./context/MCPClientContext";
import { ExographRuntimeProvider } from "./context/AssistantRuntimeContext";

export interface MCPPlaygroundProps
  extends BasePlaygroundComponentProps<PlaygroundMCPProps> {}

// Inner component that uses the context
function MCPPlaygroundInner({ mcp }: { mcp: PlaygroundMCPProps }) {
  return (
    <div className="flex h-full bg-gray-100 dark:bg-gray-900">
      <ConversationSidebar mcpEndpoint={mcp.mcpHttpPath} />

      <div className="flex-1 bg-white dark:bg-gray-800 min-h-0 overflow-hidden">
        <Thread />
      </div>
    </div>
  );
}

// Main component wrapped with providers
export function MCPPlayground({ tab: mcp, auth }: MCPPlaygroundProps) {
  return (
    <ProviderConfigProvider>
      <ModelProvider>
        <MCPClientProvider endpoint={mcp.mcpHttpPath} auth={auth}>
          <ExographRuntimeProvider>
            <MCPPlaygroundInner mcp={mcp} />
          </ExographRuntimeProvider>
        </MCPClientProvider>
      </ModelProvider>
    </ProviderConfigProvider>
  );
}
