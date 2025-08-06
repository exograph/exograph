// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { Fetcher } from "@graphiql/toolkit";
import { PlaygroundTabProps } from "../types";

export interface PlaygroundGraphQLProps extends PlaygroundTabProps {
  readonly tabType: "graphql";
  fetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
  enableSchemaLiveUpdate: boolean;
  schemaId?: number;
  initialQuery?: string;
  storageKey?: string;
}