// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { PlaygroundTabProps } from "../types";
import { PlaygroundGraphQLProps } from "../graphql/types";
import { PlaygroundMCPProps } from "../mcp/types";

export function findTabProps<T extends PlaygroundTabProps>(
  tabs: PlaygroundTabProps[],
  tabType: T['tabType']
): T | undefined {
  return tabs.find(tab => tab.tabType === tabType) as T | undefined;
}

export function getGraphQLProps(tabs: PlaygroundTabProps[]): PlaygroundGraphQLProps | undefined {
  return findTabProps<PlaygroundGraphQLProps>(tabs, "graphql");
}

export function getMCPProps(tabs: PlaygroundTabProps[]): PlaygroundMCPProps | undefined {
  return findTabProps<PlaygroundMCPProps>(tabs, "mcp");
}