// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { GraphiQLPlayground } from "./graphql/GraphiQLPlayground";
import { AuthContextProvider } from "./auth/AuthContext";
import { ThemeProvider } from "./util/theme";
import { Fetcher } from "@graphiql/toolkit";
import { JwtSecret } from "./auth/secret";
import { AuthToolbarButton } from "./auth";
import { Logo } from "./util/Logo";
import { ThemeToggleButton } from "./util/ThemeToggleButton";

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

function Navbar() {
  return (
    <nav className="flex items-center justify-between px-3 py-2 border-b bg-white border-gray-200 dark:bg-gray-800 dark:border-gray-700 h-12 dark:text-gray-100">
      <Logo />
      <div className="flex items-center gap-2">
        <AuthToolbarButton />
        <ThemeToggleButton />
      </div>
    </nav>
  );
}
