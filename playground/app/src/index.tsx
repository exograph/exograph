// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { createRoot } from "react-dom/client";
import "./index.css";

import {
  Playground,
  Fetcher,
  createGraphiQLFetcher,
} from "exograph-playground-lib";
import { PlaygroundConfig } from "./config";
import { PlaygroundTabProps } from "exograph-playground-lib/dist/types";

let playgroundConfig = (window as any).exoConfig as PlaygroundConfig;

const urlFetcher: Fetcher = createGraphiQLFetcher({
  url: playgroundConfig.graphqlHttpPath,
});

const container = document.getElementById("root");
const root = createRoot(container as HTMLElement);

const authProps = {
  oidcUrl: playgroundConfig.oidcUrl,
  jwtSourceHeader: playgroundConfig.jwtSourceHeader,
  jwtSourceCookie: playgroundConfig.jwtSourceCookie,
};

const graphqlProps = {
  tabType: "graphql" as const,
  fetcher: urlFetcher,
  upstreamGraphQLEndpoint: playgroundConfig.upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate: playgroundConfig.enableSchemaLiveUpdate,
};

const mcpProps = playgroundConfig.mcpHttpPath && {
  tabType: "mcp" as const,
  mcpHttpPath: playgroundConfig.mcpHttpPath,
};

const tabs = [graphqlProps, mcpProps].filter(
  (prop) => prop !== undefined
) as Array<PlaygroundTabProps>;

root.render(<Playground auth={authProps} tabs={tabs} />);
