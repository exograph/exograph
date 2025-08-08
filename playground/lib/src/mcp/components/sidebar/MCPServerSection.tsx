// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useMCPClient } from "../../context/MCPClientContext";
import { RefreshCw } from "lucide-react";

interface MCPServerSectionProps {
  mcpEndpoint: string;
}

export function MCPServerSection({ mcpEndpoint }: MCPServerSectionProps) {
  const state = useMCPClient();

  const getStatusColor = () => {
    switch (state.type) {
      case 'connected':
        return "bg-green-500";
      case 'connecting':
        return "bg-yellow-500";
      case 'failed':
        return "bg-red-500";
      default:
        return "bg-gray-400";
    }
  };

  const getStatusText = () => {
    switch (state.type) {
      case 'connected':
        return `Connected (${Object.keys(state.tools).length} tools)`;
      case 'connecting':
        return "Connecting...";
      case 'failed':
        return "Connection failed";
      default:
        return "Not connected";
    }
  };

  return (
    <div className="text-xs text-gray-500 dark:text-gray-400">
      <div className="font-medium mb-1">MCP Server</div>
      <div className="break-all mb-2">{mcpEndpoint}</div>
      <div className="flex items-center gap-1">
        <div className={`w-2 h-2 rounded-full ${getStatusColor()}`} />
        <span className="flex-1">{getStatusText()}</span>
        {state.type === 'failed' && (
          <button
            onClick={state.retry}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 ml-1"
            title="Retry connection"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  );
}
