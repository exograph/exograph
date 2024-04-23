// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, {
  useState,
  useRef,
  useContext,
  useEffect,
  useCallback,
} from "react";
import GraphiQL from "graphiql";
import { Fetcher, FetcherOpts, FetcherParams } from "@graphiql/toolkit";
import { GraphQLSchema } from "graphql";
import { fetchSchema, SchemaError } from "./schema";
import { AuthContext, AuthContextProvider } from "./AuthContext";
import { Logo } from "./Logo";
import { AuthToolbarButton } from "./auth";
import { explorerPlugin } from "@graphiql/plugin-explorer";

import "./index.css";
import "graphiql/graphiql.css";
import "@graphiql/plugin-explorer/dist/style.css";
import { useTheme as useGraphiqlTheme } from "@graphiql/react";
import { Theme, useTheme } from "./theme";

interface GraphiQLPlaygroundProps extends _GraphiQLPlaygroundProps {
  oidcUrl?: string;
}

interface _GraphiQLPlaygroundProps {
  fetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
  enableSchemaLiveUpdate: boolean;
  theme?: Theme;
}

export function GraphiQLPlayground({
  fetcher,
  oidcUrl,
  upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate,
  theme,
}: GraphiQLPlaygroundProps) {
  return (
    <AuthContextProvider oidcUrl={oidcUrl}>
      <_GraphiQLPlayground
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        enableSchemaLiveUpdate={enableSchemaLiveUpdate}
        theme={theme}
      />
    </AuthContextProvider>
  );
}

function _GraphiQLPlayground({
  fetcher,
  upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate,
  theme,
}: _GraphiQLPlaygroundProps) {
  const { getTokenFn } = useContext(AuthContext);

  const dataFetcher: Fetcher = async (
    graphQLParams: FetcherParams,
    opts?: FetcherOpts
  ) => {
    // Add a special header (`_exo_playground`) to the request to indicate that it's coming from the playground
    let additionalHeaders: Record<string, any> = {
      _exo_playground: "true",
    };

    if (getTokenFn) {
      let authToken = await getTokenFn();

      additionalHeaders = {
        ...additionalHeaders,
        Authorization: `Bearer ${authToken}`,
      };
    }
    return fetcher(graphQLParams, {
      ...opts,
      headers: { ...opts?.headers, ...additionalHeaders },
    });
  };

  const schemaFetcher: Fetcher = (
    graphQLParams: FetcherParams,
    opts?: FetcherOpts
  ) => {
    const additionalHeaders: Record<string, any> = {
      _exo_operation_kind: "schema_query",
    };

    return fetcher(graphQLParams, {
      ...opts,
      headers: { ...opts?.headers, ...additionalHeaders },
    });
  };

  return (
    <SchemaFetchingCore
      fetcher={dataFetcher}
      schemaFetcher={schemaFetcher}
      upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
      enableSchemaLiveUpdate={enableSchemaLiveUpdate}
      theme={theme}
    />
  );
}

function SchemaFetchingCore({
  schemaFetcher,
  fetcher,
  upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate,
  theme,
}: {
  schemaFetcher: Fetcher;
  fetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
  enableSchemaLiveUpdate: boolean;
  theme?: Theme;
}) {
  const [schema, setSchema] = useState<GraphQLSchema | SchemaError | null>(
    null
  );
  const networkErrorCount = useRef(0);

  const fetchAndSetSchema = useCallback(async () => {
    const fetchedSchema = await fetchSchema(schemaFetcher);

    // Ignore network errors for 3 consecutive fetches (to avoid failing when the server is restarting during development or the network is flaky)
    if (networkErrorCount.current >= 3) {
      setSchema("NetworkError");
    } else if (fetchedSchema === "NetworkError") {
      // let the old schema stay in place
      networkErrorCount.current += 1;
    } else {
      // Reset the counter when there is no network error
      setSchema(fetchedSchema);
    }
  }, [setSchema, schemaFetcher]);

  const isSchemaValid: boolean = schema !== null && typeof schema !== "string";

  useEffect(() => {
    if (isSchemaValid) {
      networkErrorCount.current = 0;
    }
  }, [isSchemaValid]);

  // Fetch schema only if needed (when the schema is not loaded or is invalid)
  useEffect(() => {
    (async () => {
      if (!isSchemaValid) {
        await fetchAndSetSchema();
      }
    })();
  }, [fetchAndSetSchema, isSchemaValid]);

  useEffect(() => {
    if (
      !enableSchemaLiveUpdate ||
      (schema === "NetworkError" && networkErrorCount.current >= 3)
    ) {
      return;
    }
    const intervalId = setInterval(fetchAndSetSchema, 2000);
    return () => clearInterval(intervalId);
  }, [enableSchemaLiveUpdate, schema, fetchAndSetSchema]);

  let errorMessage = null;
  let core = null;

  if (schema === null) {
    errorMessage = null; // Loading, but let's not show any error (we could consider showing with a delay to avoid a flash of the overlay)
    core = (
      <Core
        schema={null}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        theme={theme}
      />
    );
  } else if (typeof schema == "string") {
    core = (
      <Core
        schema={null}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        theme={theme}
      />
    );
    if (schema === "EmptySchema") {
      errorMessage = <EmptySchema />;
    } else if (schema === "InvalidSchema") {
      errorMessage = <InvalidSchema />;
    } else if (networkErrorCount.current >= 3) {
      errorMessage = (
        <NetworkError
          onReload={() => {
            networkErrorCount.current = 0;
            setSchema(null);
          }}
        />
      );
    }
  } else {
    core = (
      <Core
        schema={schema}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        theme={theme}
      />
    );
  }

  return (
    <div style={{ position: "relative", height: "100%" }}>
      {errorMessage && <Overlay>{errorMessage}</Overlay>}
      {core}
    </div>
  );
}

function Core({
  schema,
  fetcher,
  upstreamGraphQLEndpoint,
  theme,
}: {
  schema: GraphQLSchema | null;
  fetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
  theme?: Theme;
}) {
  // GraphiQL loses the persisted headers when the schema is updated (or the playground is manually
  // reloaded) So, use the current value of the setting in local storage as the initial value
  const shouldPersistHeaders =
    localStorage.getItem("graphiql:shouldPersistHeaders") === "true";

  const explorer = explorerPlugin({ showAttribution: false });

  const { setTheme: setGraphiqlTheme } = useGraphiqlTheme();
  const storedTheme = useTheme();
  const effectiveTheme = theme || storedTheme;

  useEffect(() => {
    setGraphiqlTheme(effectiveTheme);
  }, [setGraphiqlTheme, effectiveTheme]);

  return (
    <>
      <GraphiQL
        fetcher={fetcher}
        plugins={[explorer]}
        defaultEditorToolsVisibility={true}
        isHeadersEditorEnabled={true}
        schema={schema}
        toolbar={{ additionalContent: <AuthToolbarButton /> }}
        shouldPersistHeaders={shouldPersistHeaders}
        showPersistHeadersSettings={true}
      >
        <GraphiQL.Logo>
          <Logo theme={effectiveTheme} />
        </GraphiQL.Logo>
        {upstreamGraphQLEndpoint && (
          <GraphiQL.Footer>
            <div style={{ paddingTop: "5px" }}>
              <b>Endpoint URL</b>:{" "}
              <span
                style={{
                  color: "hsl(var(--color-primary))",
                }}
              >
                {upstreamGraphQLEndpoint}
              </span>
            </div>
          </GraphiQL.Footer>
        )}
      </GraphiQL>
    </>
  );
}

function ErrorMessage(props: {
  title: string;
  message?: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <div className="error-message">
      <div className="error-title">{props.title}</div>
      {props.message && (
        <div className="error-description">{props.message}</div>
      )}
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
      message={
        <div>
          <p>
            <a href="https://spec.graphql.org/June2018/#sec-Root-Operation-Types">
              GraphQL specification
            </a>{" "}
            requires at least one query in the schema.
          </p>
          <p>Please ensure that the model defines a query.</p>
        </div>
      }
    />
  );
}

function NetworkError({ onReload: onRetry }: { onReload: () => void }) {
  return (
    <ErrorMessage
      title="Network error"
      message="Please ensure that the server is running."
    >
      <button className="graphiql-button reload-btn" onClick={() => onRetry()}>
        Retry
      </button>
    </ErrorMessage>
  );
}

function Overlay(props: { children: React.ReactNode }) {
  return (
    <div className="overlay graphiql-dialog-overlay">{props.children}</div>
  );
}
