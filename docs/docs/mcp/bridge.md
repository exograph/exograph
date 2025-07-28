---
sidebar_position: 50
---

# Bridge

Exograph offers the [Streamable HTTP protocol](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http). However, not all MCP clients can connect over this transport, and even those that do have limitations. For example, Claude Desktop requires the MCP server to be available on a publicly accessible server. To address this, Exograph ships with an executable `exo-mcp-bridge` that offers the `stdio` protocol on one side and connects to the MCP server over HTTP on the other. It also supports setting headers and cookies to pass through to the MCP server, which is useful for authentication.

You typically use `exo-mcp-bridge` in your MCP client configuration as shown in the [MCP tutorial](../mcp-tutorial). There's no need to invoke `exo-mcp-bridge` directly (for example, from the command line)—the MCP client will invoke it for you.

To configure this in Claude Desktop, add the following to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "finance-advisor-mcp": {
      "command": "exo-mcp-bridge",
      "args": ["--endpoint", "http://localhost:9876/mcp"]
    }
  }
}
```

The bridge requires one mandatory argument: `--endpoint`, which specifies the MCP server URL.

The bridge also supports the `--header` and `--cookie` arguments to pass through headers and cookies to the MCP server. For example, to configure this with headers and cookies in Claude Desktop:

```json
{
  "mcpServers": {
    "finance-advisor-mcp": {
      "command": "exo-mcp-bridge",
      "args": [
        "--endpoint", "http://localhost:9876/mcp",
        "--header", "Authorization: Bearer <your-token>",
        "--cookie", "session_id=<your-session-id>"
      ]
    }
  }
}
```

Here, the bridge passes through the `Authorization` header and the `session_id` cookie to the MCP server, which has the same effect as logging into the MCP server.
