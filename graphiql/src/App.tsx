// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, { useState, useRef, useContext, useMemo } from "react";
import GraphiQL from "graphiql";
import { Fetcher, createGraphiQLFetcher } from "@graphiql/toolkit";
import { GraphQLSchema } from "graphql";
import { fetchSchema, SchemaError } from "./schema";
import "graphiql/graphiql.min.css";
import { AuthContext, AuthContextProvider } from "./AuthContext";
import { Logo } from "./Logo";
import { AuthToolbarButton } from "./auth";

const enableSchemaLiveUpdate = (window as any).enableSchemaLiveUpdate;

export function AppWithAuth() {
  return (
    <AuthContextProvider>
      <App />
    </AuthContextProvider>
  );
}

function App() {
  const [schema, setSchema] = useState<GraphQLSchema | SchemaError | null>(
    null
  );
  const networkErrorCount = useRef(0);
  const { getTokenFn } = useContext(AuthContext);

  const fetcher = useMemo(() => {
    const authHeaderFetch = async (
      input: RequestInfo | URL,
      init?: RequestInit | undefined
    ) => {
      if (getTokenFn) {
        let authToken = await getTokenFn();

        const headers = {
          ...init?.headers,
          Authorization: `Bearer ${authToken}`,
        };
        const withHeader = { ...init, headers };
        return await window.fetch(input, withHeader);
      } else {
        return await window.fetch(input, init);
      }
    };

    return createGraphiQLFetcher({
      url: (window as any).exoGraphQLEndpoint,
      fetch: authHeaderFetch,
    });
  }, [getTokenFn]);

  const schemaFetcher = useMemo(() => {
    const schemaFetch = async (
      input: RequestInfo | URL,
      init?: RequestInit | undefined
    ) => {
      const headers = {
        ...init?.headers,
        // This is a special header that tells the server that the current operation is a schema query
        // This is used in playground mode to resolve such request locally (and not forward it to the upstream server)
        _exo_operation_kind: "schema_query",
      };
      const withHeader = { ...init, headers };
      return await window.fetch(input, withHeader);
    };

    return createGraphiQLFetcher({
      url: (window as any).exoGraphQLEndpoint,
      fetch: schemaFetch,
    });
  }, []);

  async function fetchAndSetSchema() {
    const schema = await fetchSchema(schemaFetcher);

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
    core = <Core schema={null} fetcher={fetcher} />;
  } else if (typeof schema == "string") {
    core = <Core schema={null} fetcher={fetcher} />;
    if (schema === "EmptySchema") {
      overlay = <EmptySchema />;
    } else if (schema === "InvalidSchema") {
      overlay = <InvalidSchema />;
    } else if (networkErrorCount.current >= 3) {
      overlay = <NetworkError />;
    }
  } else {
    overlay = null;
    core = <Core schema={schema} fetcher={fetcher} />;
  }

  return (
    <>
      {overlay && <Overlay>{overlay}</Overlay>}
      {core}
    </>
  );
}

function Core(props: { schema: GraphQLSchema | null; fetcher: Fetcher }) {
  return (
    <GraphiQL
      fetcher={props.fetcher}
      defaultEditorToolsVisibility={true}
      isHeadersEditorEnabled={true}
      schema={props.schema}
      toolbar={{ additionalContent: <AuthToolbarButton /> }}
    >
      <GraphiQL.Logo>
        <Logo />
      </GraphiQL.Logo>
    </GraphiQL>
  );
}

function ErrorMessage(props: {
  title: string;
  message?: string;
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
