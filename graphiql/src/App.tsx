// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, { useState, useEffect, useRef, useCallback } from "react";
import GraphiQL from "graphiql";
import { createGraphiQLFetcher } from "@graphiql/toolkit";
import { useTheme } from "@graphiql/react";
import { GraphQLSchema } from "graphql";
import { fetchSchema, SchemaError } from "./schema";

import "graphiql/graphiql.min.css";
import { authPlugin } from "./authPlugin";

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

  return theme;
};

function Logo() {
  const graphiqlTheme = useTheme().theme;
  // Fallback to the browser's theme if GraphiQL's theme is set to "System" (which will name `graphiqlTheme` as null)
  // If the user switches theme in the browser, the logo will be updated accordingly
  const browserTheme = useBrowserTheme();

  const effectiveTheme = graphiqlTheme || browserTheme;

  // Currently, switching mode in GraphiQL doesn't update the logo, but this will get fixed
  // when https://github.com/graphql/graphiql/pull/2971 is merged.
  const logo = effectiveTheme === "dark" ? "logo-dark.svg" : "logo-light.svg";

  return (
    <a href="https://exograph.dev" target="_blank" rel="noreferrer">
      <img src={logo} className="logo" alt="Exograph" />
    </a>
  );
}

const fetcher = createGraphiQLFetcher({
  url: (window as any).exoGraphQLEndpoint,
});

const enableSchemaLiveUpdate = (window as any).enableSchemaLiveUpdate;

function App() {
  const [schema, setSchema] = useState<GraphQLSchema | SchemaError | null>(
    null
  );
  const networkErrorCount = useRef(0);

  async function fetchAndSetSchema() {
    const schema = await fetchSchema(fetcher);

    // Ignore network errors for 3 consecutive fetches (to avoid failing when the server is restarting during development or the network is flaky)
    if (networkErrorCount.current >= 3) {
      setSchema("NetworkError");
      return;
    } else if (schema === "NetworkError") {
      // let the old schema stay in place
      networkErrorCount.current += 1;
    } else {
      // Reset the counter when there is no network error
      networkErrorCount.current = 0;
      setSchema(schema);
    }

    if (enableSchemaLiveUpdate) {
      // Schedule another fetch in 2 seconds only if there was no network error while fetching the schema
      setTimeout(fetchAndSetSchema, 2000);
    }
  }

  if (schema === null) {
    fetchAndSetSchema();
  }

  let overlay = null;
  let core = null;

  if (schema === null) {
    overlay = null; // Loading, but let's not show any overlay (we could consider showing with a delay to avoid a flash of the overlay)
    core = <Core schema={null} />;
  } else if (typeof schema == "string") {
    core = <Core schema={null} />;
    if (schema === "EmptySchema") {
      overlay = <EmptySchema />;
    } else if (schema === "InvalidSchema") {
      overlay = <InvalidSchema />;
    } else if (networkErrorCount.current >= 3) {
      overlay = <NetworkError />;
    }
  } else {
    overlay = null;
    core = <Core schema={schema} />;
  }

  return (
    <>
      {overlay && <Overlay>{overlay}</Overlay>}
      {core}
    </>
  );
}

function Core(props: { schema: GraphQLSchema | null }) {
  const [headers, setHeaders] = useState("");
  const [jwtToken, setJwtToken] = useState<string | null>(null);

  const lastGoodJwtToken = useRef<string | null>(null);

  useEffect(() => {
    if (jwtToken) {
      lastGoodJwtToken.current = jwtToken;
    }
  }, [jwtToken]);

  const headersString = computeHeadersString(
    headers,
    jwtToken,
    lastGoodJwtToken.current
  );

  return (
    <GraphiQL
      fetcher={fetcher}
      defaultEditorToolsVisibility={true}
      isHeadersEditorEnabled={true}
      schema={props.schema}
      headers={headersString}
      onEditHeaders={setHeaders}
      plugins={[authPlugin(setJwtToken)]}
    >
      <GraphiQL.Logo>
        <Logo />
      </GraphiQL.Logo>
    </GraphiQL>
  );
}

function computeHeadersString(
  originalHeaders: string,
  token: string | null,
  lastGoodJwtToken: string | null
): string {
  try {
    const headersJson = originalHeaders ? JSON.parse(originalHeaders) : {};
    if (token) {
      headersJson["Authorization"] = `Bearer ${token}`;
    } else if (
      lastGoodJwtToken === headersJson["Authorization"].replace("Bearer ", "")
    ) {
      // If the token is empty and the earlier token is the same one we previously set, we remove the Authorization header
      delete headersJson["Authorization"];
    }
    // If the headersJson is empty, we return an empty string to avoid GraphiQL to display {} in the
    // headers editor
    return Object.entries(headersJson).length
      ? JSON.stringify(headersJson, null, 2)
      : "";
  } catch (e) {
    // If the headers are not valid JSON, we don't process to add the token
    return originalHeaders;
  }
}

function ErrorMessage(props: {
  title: string;
  message?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="error-message">
      <div className="error-title">{props.title}</div>
      {props.message && <div className="error-message">{props.message}</div>}
      {props.children}
    </div>
  );
}

function InvalidSchema() {
  return <ErrorMessage title="Invalid schema" />;
}

function EmptySchema() {
  return (
    <ErrorMessage
      title="Schema contains no queries"
      message="Please check the model to ensure that at least one query is defined."
    />
  );
}

function NetworkError() {
  return (
    <ErrorMessage
      title="Network error"
      message="Please ensure that the server is running."
    >
      <button className="reload-btn" onClick={() => window.location.reload()}>
        Reload
      </button>
    </ErrorMessage>
  );
}

function Overlay(props: { children: React.ReactNode }) {
  return <div className="overlay">{props.children}</div>;
}

export default App;
