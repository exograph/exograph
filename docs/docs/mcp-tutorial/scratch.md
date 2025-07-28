
# Starting from an existing database

We'll redo the [same tutorial](../mcp-tutorial), but this time starting from an existing database to show how you can do the same with your own database.

## Setting up a new database

Let's use `ddl.sql` and `seed.sql` from the `financial-advisor-mcp` directory to create a new database (which is the same as the one we used in the [existing Exograph model](../mcp-tutorial) tutorial).

```sh
# shell-command-next-line
createdb financial-advisor-mcp
# shell-command-next-line
psql -U postgres -d financial-advisor-mcp -f ddl.sql
# shell-command-next-line
psql -U postgres -d financial-advisor-mcp -f seed.sql
```

## Creating a new Exograph model from the database

Use the `exo new` command to create a new Exograph model from the database:

```sh
# shell-command-next-line
DATABASE_URL=postgresql://localhost:5432/financial-advisor-mcp exo new financial-advisor-mcp-scratch --query-access true
```

This creates a new Exograph model in the `financial-advisor-mcp-scratch` directory. The `--query-access true` flag sets up access control rules so that anyone can query any data (but no one can mutate it).

Open the `financial-advisor-mcp-scratch/index.exo` file. You should see the Exograph model for the financial advisor domain (and all access control annotations set to `@access(query=true, mutate=false)`).

## Starting the MCP server

Before starting the MCP server, create a `.env.dev.local` file in the `financial-advisor-mcp-scratch` directory:

```sh
# shell-command-next-line
echo "DATABASE_URL=postgresql://localhost:5432/financial-advisor-mcp" > financial-advisor-mcp-scratch/.env.dev.local
```

Use the `exo dev` command to start the MCP server:

```sh
# shell-command-next-line
exo dev
```

Update the Claude Desktop configuration file to use the MCP server:

```json
{
  "mcpServers": {
    "finance-advisory-local": {
      "command": "<your home directory>/.exograph/bin/exo-mcp-bridge",
      "args": [
        "--endpoint",
        "http://localhost:9876/mcp"
      ]
    }
  }
}
```

Since the model in the current version doesn't require authentication, we don't need to pass the `Authorization` header.

Now start Claude Desktop, open a new chat window, and try the same prompts as before.

## Applying access control rules

First, add a `context` element to the `index.exo` file to extract the current user's ID and role:

```exo
context AuthContext {
  @jwt("sub") id: Uuid
  @jwt role: String
}
```

Next, use the `AuthContext` in the `index.exo` file to tighten the access control rules. We want to restrict the `Account` type so it can be viewed only by:
- The associated customer (`self.customer.id == AuthContext.id`)
- The customer's financial advisor (`self.customer.financialAdvisor.id == AuthContext.id`)
- An admin (`AuthContext.role == "admin"`)

So the access control rule would be:
```exo
AuthContext.id == self.customer.id || 
AuthContext.id == self.customer.financialAdvisor.id ||
AuthContext.role == "admin"
```

Set this access control rule on the `Account` type as follows:

```exo
@access(query=AuthContext.id == self.customer.id || 
              AuthContext.id == self.customer.financialAdvisor.id ||
              AuthContext.role == "admin")
type Account {
  ...
}
```

Since Exograph's default access control is `false`, we don't need to specify `@access(mutate=false)` on the `Account` type.

Similarly, for the `Customer` type, we want to ensure it can be viewed only by:
- The customer themselves (`self.id == AuthContext.id`)
- The associated financial advisor (`self.financialAdvisor.id == AuthContext.id`)
- An admin (`AuthContext.role == "admin"`)

So the access control rule would be:
```exo
AuthContext.id == self.id || AuthContext.id == self.financialAdvisor.id || AuthContext.role == "admin"
```

Set this access control rule on the `Customer` type as follows:

```exo
@access(query=AuthContext.id == self.id || 
              AuthContext.id == self.financialAdvisor.id || 
              AuthContext.role == "admin")
type Customer {
  ...
}
```

You can now update the rest of the types in a similar manner. Study [access control rules](/postgres/access-control.md) for more details or take a peek at the [code](https://github.com/exograph/examples/blob/main/financial-advisor-mcp/src/index.exo) from the [existing Exograph model](../mcp-tutorial) tutorial.

## Using the MCP server

Using the MCP server is the same as in the [existing Exograph model](../mcp-tutorial) tutorial. Make sure to pass the `Authorization` header, since with access control in place, you'll get errors without it (except for types that have access control set to `@access(query=true, mutate=false)`).
