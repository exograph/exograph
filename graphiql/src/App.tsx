import GraphiQL from 'graphiql';
import 'graphiql/graphiql.min.css';

const Logo = () => <img src="/logo.svg" className="logo" alt='Claytip'/>;

const App = () => (
  <GraphiQL tabs 
    fetcher={async graphQLParams => {
      const data = await fetch(
        window.location.origin,
        {
          method: 'POST',
          headers: {
            Accept: 'application/json',
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(graphQLParams),
          credentials: 'same-origin',
        },
      );
      return data.json().catch(() => data.text());
    }}
    defaultVariableEditorOpen = {true}
    headerEditorEnabled = {true}
  >
     <GraphiQL.Logo><Logo/></GraphiQL.Logo>
  </GraphiQL>
);

export default App;