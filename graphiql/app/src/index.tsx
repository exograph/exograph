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
  GraphiQLPlayground,
  Fetcher,
  createGraphiQLFetcher,
} from "exograph-playground-lib";

const urlFetcher: Fetcher = createGraphiQLFetcher({
  url: (window as any).exoGraphQLEndpoint,
});

const container = document.getElementById("root");
const root = createRoot(container as HTMLElement);
root.render(
  <GraphiQLPlayground
    fetcher={urlFetcher}
    oidcUrl={(window as any).exoOidcUrl as string | undefined}
    upstreamGraphQLEndpoint={
      (window as any).exoUpstreamGraphQLEndpoint as string | undefined
    }
    enableSchemaLiveUpdate={
      ((window as any).enableSchemaLiveUpdate as boolean) || false
    }
  />
);
