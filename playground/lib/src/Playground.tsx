// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { GraphiQLPlayground } from "./GraphiQLPlayground";
import { Navbar } from "./Navbar";
import { AuthContextProvider } from "./AuthContext";
import { ThemeProvider } from "./theme";
import { Fetcher } from "@graphiql/toolkit";
import { JwtSecret } from "./auth/secret";

export interface PlaygroundProps {
  fetcher: Fetcher;
  oidcUrl?: string;
  jwtSecret?: JwtSecret;
  upstreamGraphQLEndpoint?: string;
  enableSchemaLiveUpdate: boolean;
  schemaId?: number;
  jwtSourceHeader?: string;
  jwtSourceCookie?: string;
  initialQuery?: string;
  storageKey?: string;
}

export function Playground({
  oidcUrl,
  jwtSecret,
  ...playgroundProps
}: PlaygroundProps) {
  return (
    <ThemeProvider>
      <AuthContextProvider oidcUrl={oidcUrl} jwtSecret={jwtSecret}>
        <div className="h-screen flex flex-col overflow-hidden">
          <Navbar />
          <div className="flex-1 min-h-0 overflow-hidden">
            <GraphiQLPlayground {...playgroundProps} />
          </div>
        </div>
      </AuthContextProvider>
    </ThemeProvider>
  );
}
