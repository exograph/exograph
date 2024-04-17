// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, { useState, useRef, useContext } from "react";
import GraphiQL from "graphiql";
import {
  Fetcher,
  FetcherOpts,
  FetcherParams,
  createGraphiQLFetcher,
} from "@graphiql/toolkit";
import { GraphQLSchema } from "graphql";
import { fetchSchema, SchemaError } from "./schema";
import { AuthContext, AuthContextProvider } from "./AuthContext";
import { Logo } from "./Logo";
import { AuthToolbarButton } from "./auth";
import { explorerPlugin } from "@graphiql/plugin-explorer";

import "./index.css";
import "graphiql/graphiql.css";
import "@graphiql/plugin-explorer/dist/style.css";

const enableSchemaLiveUpdate = (window as any).enableSchemaLiveUpdate;

const urlFetcher: Fetcher = createGraphiQLFetcher({
  url: (window as any).exoGraphQLEndpoint,
});

export function AppWithAuth({ fetcher = urlFetcher }: { fetcher?: Fetcher }) {
  return (
    <AuthContextProvider>
      <App fetcher={fetcher} />
    </AuthContextProvider>
  );
}

function App({ fetcher }: { fetcher: Fetcher }) {
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

  const upstreamGraphQLEndpoint = (window as any).exoUpstreamGraphQLEndpoint;

  return (
    <SchemaFetchingCore
      fetcher={dataFetcher}
      schemaFetcher={schemaFetcher}
      upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
    />
  );
}

function SchemaFetchingCore({
  schemaFetcher,
  fetcher,
  upstreamGraphQLEndpoint,
}: {
  schemaFetcher: Fetcher;
  fetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
}) {
  const [schema, setSchema] = useState<GraphQLSchema | SchemaError | null>(
    null
  );
  const networkErrorCount = useRef(0);

  async function fetchAndSetSchema() {
    console.log("Fetching schema...");
    const schema = await fetchSchema(schemaFetcher);
    console.log("Schema fetched ", schema);

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
    core = (
      <Core
        schema={null}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
      />
    );
  } else if (typeof schema == "string") {
    core = (
      <Core
        schema={null}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
      />
    );
    if (schema === "EmptySchema") {
      overlay = <EmptySchema />;
    } else if (schema === "InvalidSchema") {
      overlay = <InvalidSchema />;
    } else if (networkErrorCount.current >= 3) {
      overlay = <NetworkError />;
    }
  } else {
    overlay = null;
    core = (
      <Core
        schema={schema}
        fetcher={fetcher}
        upstreamGraphQLEndpoint={upstreamGraphQLEndpoint}
      />
    );
  }

  return (
    <>
      {overlay && <Overlay>{overlay}</Overlay>}
      {core}
    </>
  );
}

function Core({
  schema,
  fetcher,
  upstreamGraphQLEndpoint,
}: {
  schema: GraphQLSchema | null;
  fetcher: Fetcher;
  upstreamGraphQLEndpoint?: string;
}) {
  // GraphiQL loses the persisted headers when the schema is updated (or the playground is manually
  // reloaded) So, use the current value of the setting in local storage as the initial value
  const shouldPersistHeaders =
    localStorage.getItem("graphiql:shouldPersistHeaders") === "true";

  const explorer = explorerPlugin({ showAttribution: false });

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
          <Logo />
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

function NetworkError() {
  return (
    <ErrorMessage
      title="Network error"
      message="Please ensure that the server is running."
    >
      <button
        className="graphiql-button reload-btn"
        onClick={() => window.location.reload()}
      >
        Reload
      </button>
    </ErrorMessage>
  );
}

function Overlay(props: { children: React.ReactNode }) {
  return (
    <div className="overlay graphiql-dialog-overlay">{props.children}</div>
  );
}
