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

export interface MCPPlaygroundProps
  extends BasePlaygroundComponentProps<PlaygroundMCPProps> {}

export function MCPPlayground({ tab: mcp, auth: _auth }: MCPPlaygroundProps) {
  return (
    <div className="flex items-center justify-center h-full bg-white dark:bg-gray-800">
      <div className="text-2xl text-gray-600 dark:text-gray-300">
        MCP Playground Coming soon
        <br />
        {mcp.mcpHttpPath}
      </div>
    </div>
  );
}
