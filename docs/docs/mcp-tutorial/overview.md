---
slug: /mcp-tutorial
sidebar_position: 0
---

# Model Context Protocol

In the [getting started](/getting-started#working-with-llms-using-mcp) guide, we created a simple Exograph application and set up an MCP client (Claude Desktop). However, the todo application was too simple to show the real power of MCP. In this tutorial, we will use a Financial Advisory domain to interact with data using natural language.

## Clone the examples repository

We will use the Financial Advisor domain from the [exograph-examples](https://github.com/exograph/exograph-examples) repository. Let's clone the repository.

```sh
# shell-command-next-line
git clone https://github.com/exograph/exograph-examples.git
```

Navigate to the `financial-advisor` directory.

```sh
# shell-command-next-line
cd exograph-examples/financial-advisor
```

## Start the MCP server

Let's start the MCP server. Assuming that you have either Postgres or Docker installed, you can start the MCP server with the following command:

```sh
# shell-command-next-line
exo yolo --seed financial-advisor-seed.sql
```

Alternatively, you can use the dev mode, which will help you use a remote database. See [here](/cli-reference/development/dev) for more details.

In any case, the console will print the MCP server URL.

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

In the default configuration, the MCP server will offer one tool: `execute_query`. Its description gives LLMs sufficient information to form appropriate queries.

## Using the MCP Server

Let's configure Claude Desktop or Cursor. For example, for Claude Desktop, follow the instructions [here](https://modelcontextprotocol.io/quickstart/user#2-add-the-filesystem-mcp-server) to create the configuration file and add the following to the `mcpServers` section (make sure to replace `<your home directory>` with your actual home directory):

```json
{
  "mcpServers": {
    "financial-advisor-mcp": {
      "command": "<your home directory>/.exograph/bin/exo-mcp-bridge",
      "args": ["--endpoint", "http://localhost:9876/mcp"]
    }
  }
}
```

Note that while Exograph MCP supports the HTTP Streaming protocol, client support is still limited, so currently the easiest approach is to use the "stdio" protocol with the `exo-mcp-bridge` command. The bridge is a simple wrapper that communicates with LLMs using the "stdio" protocol and forwards requests to the MCP server over HTTP. Once client support improves, you would be able to use the HTTP Streaming protocol directly.
