// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState } from "react";
import { GraphiQLPlayground } from "./graphql/GraphiQLPlayground";
import { MCPPlayground } from "./mcp/MCPPlayground";
import { AuthContextProvider } from "./auth/AuthContext";
import { ThemeProvider } from "./util/theme";
import { AuthToolbarButton } from "./auth";
import { JWTAuthentication } from "./auth/types";
import { Logo } from "./util/Logo";
import { ThemeToggleButton } from "./util/ThemeToggleButton";
import { PlaygroundTab, PlaygroundTabProps } from "./types";
import { getGraphQLProps, getMCPProps } from "./util/playground-helpers";

export interface PlaygroundProps {
  auth: JWTAuthentication;
  tabs: PlaygroundTabProps[];
}

export function Playground({
  auth,
  tabs,
}: PlaygroundProps) {
  const [activeTab, setActiveTab] = useState<PlaygroundTab>(
    tabs[0]?.tabType || "graphql"
  );

  const graphqlProps = getGraphQLProps(tabs);
  const mcpProps = getMCPProps(tabs);

  return (
    <ThemeProvider>
      <AuthContextProvider oidcUrl={auth.oidcUrl} jwtSecret={auth.jwtSecret}>
        <div className="h-screen flex flex-col overflow-hidden">
          <Navbar
            activeTab={activeTab}
            onTabChange={setActiveTab}
            tabs={tabs}
          />
          <div className="flex-1 min-h-0 overflow-hidden">
            {activeTab === "graphql" ? (
              graphqlProps && <GraphiQLPlayground tab={graphqlProps} auth={auth} />
            ) : (
              mcpProps && <MCPPlayground tab={mcpProps} auth={auth} />
            )}
          </div>
        </div>
      </AuthContextProvider>
    </ThemeProvider>
  );
}

interface NavbarProps {
  activeTab: PlaygroundTab;
  onTabChange: (tab: PlaygroundTab) => void;
  tabs: PlaygroundTabProps[];
}

function Navbar({
  activeTab,
  onTabChange,
  tabs,
}: NavbarProps) {
  const tabLabels: Record<PlaygroundTab, string> = {
    graphql: "GraphQL",
    mcp: "MCP",
  };

  const enabledTabs = tabs.map(tab => tab.tabType);
  const showTabs = tabs.length > 1;

  return (
    <nav className="flex items-center justify-between px-3 border-b bg-white border-gray-200 dark:bg-gray-800 dark:border-gray-700 h-12 dark:text-gray-100">
      <div className="flex items-center h-full">
        <Logo />
        {showTabs && (
          <div className="flex ml-8 h-full">
            {enabledTabs.map((tab) => (
              <button
                key={tab}
                className={`px-4 h-full font-medium transition-colors border-b-2 ${
                  activeTab === tab
                    ? "text-blue-600 border-blue-600 dark:text-blue-400 dark:border-blue-400"
                    : "text-gray-600 border-transparent hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-200"
                }`}
                onClick={() => onTabChange(tab)}
              >
                {tabLabels[tab]}
              </button>
            ))}
          </div>
        )}
      </div>
      <div className="flex items-center gap-2">
        <AuthToolbarButton />
        <ThemeToggleButton />
      </div>
    </nav>
  );
}
