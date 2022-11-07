import GraphiQL from "graphiql";
import { createGraphiQLFetcher } from "@graphiql/toolkit";

import "graphiql/graphiql.min.css";

const Logo = () => (
  <a href="https://claytip.com" target="_blank" rel="noreferrer">
    <img src="logo.svg" className="logo" alt="Claytip" />
  </a>
);

const fetcher = createGraphiQLFetcher({
  url: (window as any).clayGraphQLEndpoint,
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
