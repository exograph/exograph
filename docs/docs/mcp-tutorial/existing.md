---
slug: /mcp-tutorial
sidebar_position: 0
---

# Using an existing Exograph model

In the [getting started](/getting-started/local.md#working-with-llms-using-mcp) guide, we worked with the "todo" domain to show how to use MCP with Exograph. However, that domain is too simple to demonstrate the real power of MCP. In this tutorial, we will use a more complex domain—a boutique financial advisory firm—to interact with data using natural language. This domain includes concepts like accounts, transactions, advisors, customers, and more. 

In this part, we will start with an existing Exograph model. Later, we will start [from an existing database](scratch.md).

## Clone the examples repository

The code is available in the [exograph-examples](https://github.com/exograph/exograph-examples) repository. Clone the repository and navigate to the `financial-advisor` directory.

```sh
# shell-command-next-line
git clone https://github.com/exograph/exograph-examples.git
# shell-command-next-line
cd exograph-examples/financial-advisor
```

Open the current folder in your favorite IDE (VS Code, Cursor, etc.) with the [Exograph extension](https://marketplace.visualstudio.com/items?itemName=exograph.exograph) installed. Open the `index.exo` file. You should see the Exograph model for the financial advisor domain with access control rules such as:

- A branch may be viewed by any user.
- A customer can only view their own information.
- A financial advisor can only view customers assigned to them.
- An admin user has full access to all data.

## Start the MCP server

Let's start the MCP server. Assuming you have either Postgres or Docker installed, you can start the MCP server with the following command:

```sh
# shell-command-next-line
exo yolo --seed seed.sql
```

This command creates a new temporary database and seeds it with sample data. When you stop the server, the database will be deleted. Alternatively, you can use dev mode with an existing database. See [here](/cli-reference/development/dev) for more details. Either way, the console will display the MCP server URL:

```sh
Started server on localhost:9876 in 11.55 ms
- GraphQL endpoint hosted at:
        http://localhost:9876/graphql
# highlight-start        
- MCP endpoint hosted at:
        http://localhost:9876/mcp
# highlight-end
- Playground hosted at:
        http://localhost:9876/playground
```

We need to configure an MCP client to use the MCP endpoint.

## Using the MCP Server

We'll use Claude Desktop for this tutorial (but you can adapt it to other MCP clients). To simplify setup, we've created a script that creates the configuration file and sets up authentication. Run the following command:

```sh
# shell-command-next-line
./scripts/switch-user.sh <user-id or "admin">
```

The script accepts either a user ID (from our seed data: EMP001-004 for financial advisors or CUST000001-000015 for customers) or "admin" for the admin user. It creates a configuration file at `~/Library/Application Support/Claude/claude_desktop_config.json` (for Mac) with the appropriate authentication headers.

:::note Authentication
In a real application, you would use OAuth support, which will come to Exograph soon. For agentic workflows, you have more options. For example, if the agent is presented as a web application, you would pass along the authentication token provided by the web application.
:::

Once you run the script, the Claude configuration file looks like this:

```json
{
  "mcpServers": {
    "finance-advisory-local": {
      "command": "<your home directory>/.exograph/bin/exo-mcp-bridge",
      "args": [
        "--endpoint",
        "http://localhost:9876/mcp",
        "--header",
        "Authorization=Bearer ey..."
      ]
    }
  }
}
```

Let's try with a few users to see how Exograph's access control works.

### With the admin user

Let's switch to the admin user. The tutorial code is configured to give the admin user full access to all data.

```sh
# shell-command-next-line
./scripts/switch-user.sh admin
```

The "admin" argument sets up configuration for the admin user (by passing the `Authorization` header), which has full access to all data. Later, we'll see how to configure access for a specific customer or financial advisor.

Now, start Claude Desktop and open a new chat window. If you click on the "Search and tools" button, you should see the "finance-advisory-local" server in the list.

You can now ask questions like "List all the branches" or "List all the customers". Before running each query, Claude Desktop will ask for your permission. You may allow individual queries or allow all queries.

Try different prompts such as "How are financial advisors doing?" or "Could we better assign customers to financial advisors?"

### With a financial advisor

Let's switch to a financial advisor:

```sh
# shell-command-next-line
./scripts/switch-user.sh EMP001
```

Restart Claude Desktop (required whenever you switch users).

Now ask questions like "List all the customers". You'll see that the financial advisor can only see customers assigned to them. Try switching to a different financial advisor and asking the same question.

### With a customer

Let's switch to a customer. Our model allows customers to see their own information:

```sh
# shell-command-next-line
./scripts/switch-user.sh CUST000001
```

Restart Claude Desktop.

Now ask questions like "List all the branches" (which is allowed since there's no restriction on branches). However, if you query for all customers, you'll only see the current customer's information.
