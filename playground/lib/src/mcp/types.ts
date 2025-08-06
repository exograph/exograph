// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { PlaygroundTabProps } from "../types";

export interface PlaygroundMCPProps extends PlaygroundTabProps {
  readonly tabType: "mcp";
  // TODO: Add MCP-specific properties when MCP functionality is implemented
}