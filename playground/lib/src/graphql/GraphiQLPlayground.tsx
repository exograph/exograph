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
  useMemo,
} from "react";
import { GraphiQL, HISTORY_PLUGIN } from "graphiql";
import {
  Fetcher,
  FetcherOpts,
  FetcherParams,
  Storage,
  createLocalStorage,
} from "@graphiql/toolkit";
import { GraphQLSchema } from "graphql";
import { fetchSchema, SchemaError } from "./schema";
import { AuthContext } from "../auth/AuthContext";
import { AuthConfigContext } from "../auth/secret/AuthConfigProvider";
import { explorerPlugin } from "@graphiql/plugin-explorer";
import { PlaygroundGraphQLProps as GraphiQLProps } from "./types";
import { BasePlaygroundComponentProps } from "../util/component-types";

import "./index.css";
import "graphiql/style.css";
import "@graphiql/plugin-explorer/style.css";
import { useTheme } from "../util/theme";

export interface GraphiQLPlaygroundProps extends BasePlaygroundComponentProps<GraphiQLProps> {}

export function GraphiQLPlayground({ tab: graphql, auth }: GraphiQLPlaygroundProps) {
  const { fetcher } = graphql;
  const { jwtSourceHeader, jwtSourceCookie } = auth;
  const { getTokenFn } = useContext(AuthContext);
  const authConfigContext = useContext(AuthConfigContext);
  const customHeaders = authConfigContext?.config?.headers || {};

  const dataFetcher: Fetcher = async (
    graphQLParams: FetcherParams,
    opts?: FetcherOpts
  ) => {
    // Add a special header (`_exo_playground`) to the request to indicate that it's coming from the playground
    let additionalHeaders: Record<string, any> = {
      _exo_playground: "true",
      ...customHeaders, // Include custom headers from config
    };

    if (getTokenFn) {
      let authToken = await getTokenFn();

      if (authToken) {
        if (jwtSourceCookie) {
          document.cookie = `${jwtSourceCookie}=${authToken}`;
        } else {
          const authHeader = jwtSourceHeader || "Authorization";
          additionalHeaders = {
            ...additionalHeaders,
            [authHeader]: `Bearer ${authToken}`,
          };
        }
      }
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
      schemaFetcher={schemaFetcher}
      {...graphql}
      fetcher={dataFetcher}
    />
  );
}

function SchemaFetchingCore(
  props: {
    schemaFetcher: Fetcher;
  } & GraphiQLProps
) {
  const { schemaFetcher, enableSchemaLiveUpdate, schemaId } = props;
  const [schema, setSchema] = useState<GraphQLSchema | SchemaError | null>(
    null
  );
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
    schema === "NetworkError" &&
    (networkErrorCount.current >= 3 || !enableSchemaLiveUpdate);

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

  const coreProps = {
    initialQuery: props.initialQuery,
    fetcher: props.fetcher,
    upstreamGraphQLEndpoint: props.upstreamGraphQLEndpoint,
    storageKey: props.storageKey,
  };

  let core = null;

  if (schema === null) {
    errorMessage = null; // Loading, but let's not show any error (we could consider showing with a delay to avoid a flash of the overlay)
    core = <Core {...coreProps} schema={null} />;
  } else if (typeof schema == "string") {
    core = <Core {...coreProps} schema={null} />;
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
    core = <Core {...coreProps} schema={schema} />;
  }

  return (
    <div className="relative h-full flex flex-col">
      {errorMessage && <Overlay>{errorMessage}</Overlay>}
      <div className="flex-1">{core}</div>
    </div>
  );
}

const plugins = [HISTORY_PLUGIN, explorerPlugin()];

function Core({
  schema,
  initialQuery,
  fetcher,
  upstreamGraphQLEndpoint,
  storageKey,
}: {
  schema: GraphQLSchema | null;
} & Pick<
  GraphiQLProps,
  "fetcher" | "initialQuery" | "storageKey" | "upstreamGraphQLEndpoint"
>) {
  const theme = useTheme();

  const storage = useMemo<Storage>(() => {
    const baseStorage = createLocalStorage({
      namespace: `exograph-playground:${storageKey || ""}`,
    });

    // Avoid persisting the hidden state for the variables/headers panel so it always re-opens.
    const isHiddenSecondaryEditor = (key: string, value: string | null) =>
      key === "secondaryEditorFlex" && value === "hide-second";

    const getFallbackValue = (key: string, value: string | null) => {
      if (key === "variables" && (value === null || value.trim() === "")) {
        return "{}";
      }
      return value;
    };

    return {
      getItem(key) {
        const storedValue = baseStorage.getItem(key);
        if (isHiddenSecondaryEditor(key, storedValue)) {
          baseStorage.removeItem(key);
          return null;
        }
        return getFallbackValue(key, storedValue);
      },
      setItem(key, value) {
        if (isHiddenSecondaryEditor(key, value)) {
          baseStorage.removeItem(key);
          return;
        }
        baseStorage.setItem(key, value);
      },
      removeItem(key) {
        baseStorage.removeItem(key);
      },
      clear() {
        baseStorage.clear();
      },
      get length() {
        return baseStorage.length;
      },
    };
  }, [storageKey]);

  return (
    <GraphiQL
      fetcher={fetcher}
      defaultQuery={initialQuery}
      plugins={plugins}
      defaultEditorToolsVisibility="variables"
      isHeadersEditorEnabled={true}
      schema={schema}
      showPersistHeadersSettings={true}
      storage={storage}
      forcedTheme={theme}
    >
      <GraphiQL.Logo>{null}</GraphiQL.Logo>
      <GraphiQL.Toolbar>
        {({ merge, prettify, copy }) => (
          <>
            {prettify}
            {merge}
            {copy}
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

function NetworkError({ onRetry }: { onRetry: () => void }) {
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
