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
  const browserTheme = useBrowserTheme();

  // Currently, switching mode in GraphiQL doesn't update the logo, but this will get fixed
  // when https://github.com/graphql/graphiql/pull/2971 is merged.
  const logo =
    graphiqlTheme === "dark" || browserTheme === "dark"
      ? "logo-dark.svg"
      : "logo-light.svg";

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

  async function fetchAndSetSchema() {
    let schema = await fetchSchema(fetcher);
    setSchema(schema);
    if (enableSchemaLiveUpdate && schema !== "NetworkError") {
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
    } else {
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
  return (
    <GraphiQL
      fetcher={fetcher}
      defaultEditorToolsVisibility={true}
      isHeadersEditorEnabled={true}
      schema={props.schema}
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
