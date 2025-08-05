// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, { useState, useRef, useContext, useEffect, useCallback, useMemo } from "react";
import { GraphiQL, HISTORY_PLUGIN } from "graphiql";
import { Fetcher, FetcherOpts, FetcherParams, createLocalStorage } from "@graphiql/toolkit";
import { GraphQLSchema } from "graphql";
import { fetchSchema, SchemaError } from "./schema";
import { AuthContext, AuthContextProvider } from "./AuthContext";
import { Logo } from "./Logo";
import { AuthToolbarButton } from "./auth";
import { explorerPlugin } from "@graphiql/plugin-explorer";

import "./index.css";
import "graphiql/style.css";
import "@graphiql/plugin-explorer/style.css";
import { Theme, useTheme } from "./theme";
import { JwtSecret } from "./auth/secret";

interface GraphiQLPlaygroundProps extends _GraphiQLPlaygroundProps {
  oidcUrl?: string;
  jwtSecret?: JwtSecret;
}

interface _GraphiQLPlaygroundProps extends GraphiQLPassThroughProps {
  upstreamGraphQLEndpoint?: string;
  enableSchemaLiveUpdate: boolean;
  schemaId?: number;
  jwtSourceHeader?: string;
  jwtSourceCookie?: string;
}

export function GraphiQLPlayground({
  fetcher,
  oidcUrl,
  jwtSecret,
  upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate,
  schemaId,
  initialQuery,
  theme,
  storageKey,
  jwtSourceHeader,
  jwtSourceCookie,
}: GraphiQLPlaygroundProps) {
  return (
    <AuthContextProvider oidcUrl={oidcUrl} jwtSecret={jwtSecret}>
      <_GraphiQLPlayground
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        enableSchemaLiveUpdate={enableSchemaLiveUpdate}
        jwtSourceHeader={jwtSourceHeader}
        jwtSourceCookie={jwtSourceCookie}
        schemaId={schemaId}
        initialQuery={initialQuery}
        theme={theme}
        storageKey={storageKey}
      />
    </AuthContextProvider>
  );
}

interface GraphiQLPassThroughProps {
  fetcher: Fetcher;
  initialQuery?: string;
  theme?: Theme;
  storageKey?: string;
}

function _GraphiQLPlayground({
  fetcher,
  upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate,
  jwtSourceHeader,
  jwtSourceCookie,
  schemaId,
  initialQuery,
  theme,
  storageKey,
}: _GraphiQLPlaygroundProps) {
  const { getTokenFn } = useContext(AuthContext);

  const dataFetcher: Fetcher = async (graphQLParams: FetcherParams, opts?: FetcherOpts) => {
    // Add a special header (`_exo_playground`) to the request to indicate that it's coming from the playground
    let additionalHeaders: Record<string, any> = {
      _exo_playground: "true",
    };

    if (getTokenFn) {
      let authToken = await getTokenFn();

      if (jwtSourceCookie) {
        document.cookie = `${jwtSourceCookie}=${authToken}`;
      } else {
        let authHeader = jwtSourceHeader || "Authorization";

        additionalHeaders = {
          ...additionalHeaders,
          [authHeader]: `Bearer ${authToken}`,
        };
      }
    }

    return fetcher(graphQLParams, {
      ...opts,
      headers: { ...opts?.headers, ...additionalHeaders },
    });
  };

  const schemaFetcher: Fetcher = (graphQLParams: FetcherParams, opts?: FetcherOpts) => {
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
      schemaId={schemaId}
      initialQuery={initialQuery}
      theme={theme}
      storageKey={storageKey}
    />
  );
}

function SchemaFetchingCore({
  schemaFetcher,
  fetcher,
  upstreamGraphQLEndpoint,
  enableSchemaLiveUpdate,
  schemaId,
  initialQuery,
  theme,
  storageKey,
}: {
  schemaFetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
  enableSchemaLiveUpdate: boolean;
  schemaId?: number;
} & GraphiQLPassThroughProps) {
  const [schema, setSchema] = useState<GraphQLSchema | SchemaError | null>(null);
  const networkErrorCount = useRef(0);

  const fetchAndSetSchema = useCallback(async () => {
    const fetchedSchema = await fetchSchema(schemaFetcher);

    if (enableSchemaLiveUpdate) {
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
    } else {
      setSchema(fetchedSchema);
    }
  }, [schema, setSchema, schemaFetcher, enableSchemaLiveUpdate]);

  const unrecoverableNetworkError =
    schema === "NetworkError" && (networkErrorCount.current >= 3 || !enableSchemaLiveUpdate);

  // Reset the schema when the schemaId changes (another effect will re-fetch the schema)
  useEffect(() => {
    setSchema(null);
  }, [schemaId]);

  // Reset the network error count when the schema is loaded
  useEffect(() => {
    if (schema !== null && typeof schema !== "string") {
      networkErrorCount.current = 0;
    }
  }, [schema]);

  // Fetch schema only if needed (when the schema is not loaded or is invalid)
  useEffect(() => {
    (async () => {
      if (!(schema !== null && typeof schema !== "string")) {
        await fetchAndSetSchema();
      }
    })();
  }, [fetchAndSetSchema, schema]);

  useEffect(() => {
    if (!enableSchemaLiveUpdate || unrecoverableNetworkError) {
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
        initialQuery={initialQuery}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        theme={theme}
        storageKey={storageKey}
      />
    );
  } else if (typeof schema == "string") {
    core = (
      <Core
        schema={null}
        initialQuery={initialQuery}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        theme={theme}
        storageKey={storageKey}
      />
    );
    if (schema === "EmptySchema") {
      errorMessage = <EmptySchema />;
    } else if (schema === "InvalidSchema") {
      errorMessage = <InvalidSchema />;
    } else if (unrecoverableNetworkError) {
      errorMessage = (
        <NetworkError
          onRetry={() => {
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
        initialQuery={initialQuery}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
        theme={theme}
        storageKey={storageKey}
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

const plugins = [HISTORY_PLUGIN, explorerPlugin()];

function Core({
  schema,
  initialQuery,
  fetcher,
  upstreamGraphQLEndpoint,
  theme,
  storageKey,
}: {
  upstreamGraphQLEndpoint?: string;
  schema: GraphQLSchema | null;
} & GraphiQLPassThroughProps) {
  const storedTheme = useTheme();
  const effectiveTheme = theme || storedTheme;

  const storage = useMemo(() => {
    return createLocalStorage({
      namespace: `exograph-playground:${storageKey || ""}`,
    });
  }, [storageKey]);

  return (
    <GraphiQL
      fetcher={fetcher}
      defaultQuery={initialQuery}
      plugins={plugins}
      defaultEditorToolsVisibility={true}
      isHeadersEditorEnabled={true}
      schema={schema}
      showPersistHeadersSettings={true}
      storage={storage}
    >
      <GraphiQL.Logo>
        <Logo theme={effectiveTheme} />
      </GraphiQL.Logo>
      <GraphiQL.Toolbar>
        {({ merge, prettify, copy }) => (
          <>
            {prettify}
            {merge}
            {copy}
            <AuthToolbarButton />
          </>
        )}
      </GraphiQL.Toolbar>
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
      {props.message && <div className="error-description">{props.message}</div>}
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

function NetworkError({ onRetry }: { onRetry: () => void }) {
  return (
    <ErrorMessage title="Network error" message="Please ensure that the server is running.">
      <button className="graphiql-button reload-btn" onClick={() => onRetry()}>
        Retry
      </button>
    </ErrorMessage>
  );
}

function Overlay(props: { children: React.ReactNode }) {
  return <div className="overlay graphiql-dialog-overlay">{props.children}</div>;
}
