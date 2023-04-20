// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState, useEffect, useRef, useCallback } from 'react'
import GraphiQL from "graphiql";
import { createGraphiQLFetcher } from "@graphiql/toolkit";
import { useTheme } from "@graphiql/react";

import "graphiql/graphiql.min.css";

export const useBrowserTheme = () => {
  const mql = useRef(window.matchMedia("(prefers-color-scheme: dark)")).current;

  const currentTheme = useCallback(() => {
    return mql.matches ? "dark" : "light";
  }, [mql]);

  const [theme, setTheme] = useState(currentTheme());

  useEffect(() => {
    const setCurrentTheme = () => {
      setTheme(currentTheme());
    };
    mql.addEventListener("change", setCurrentTheme);
    return () => mql.removeEventListener("change", setCurrentTheme);
  }, [currentTheme, mql]);

  return theme
}

function Logo() {
  const graphiqlTheme = useTheme().theme;
  const browserTheme = useBrowserTheme();

  // Currently, switching mode in GraphiQL doesn't update the logo, but this will get fixed
  // when https://github.com/graphql/graphiql/pull/2971 is merged.
  const logo = graphiqlTheme === "dark" || browserTheme === "dark" ? "logo-dark.svg" : "logo-light.svg";

  return (
    <a href="https://exograph.dev" target="_blank" rel="noreferrer">
      <img src={logo} className="logo" alt="Exograph" />
    </a>
  );
};

const fetcher = createGraphiQLFetcher({
  url: (window as any).exoGraphQLEndpoint,
});

const App = () => (
  <GraphiQL
    fetcher={fetcher}
    defaultEditorToolsVisibility={true}
    isHeadersEditorEnabled={true}
  >
    <GraphiQL.Logo>
      <Logo />
    </GraphiQL.Logo>
  </GraphiQL>
);

export default App;
