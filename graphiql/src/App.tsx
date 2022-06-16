import GraphiQL from 'graphiql';
import { createGraphiQLFetcher } from '@graphiql/toolkit';

import 'graphiql/graphiql.min.css';

const Logo = () => <img src="/logo.svg" className="logo" alt='Claytip'/>;

const fetcher = createGraphiQLFetcher({
  url: (window as any).clayGraphQLEndpoint
});

const App = () => (
  
  <GraphiQL tabs 
    fetcher={fetcher}
    defaultVariableEditorOpen = {true}
    headerEditorEnabled = {true}
  >
     <GraphiQL.Logo><Logo/></GraphiQL.Logo>
  </GraphiQL>
);

export default App;