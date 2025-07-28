---
sidebar_position: 20
---

# How it works

The MCP protocol specifies several [capabilities](https://modelcontextprotocol.io/specification/2025-06-18/architecture/index#capability-negotiation) that an MCP server may offer. Exograph's MCP server implements the [tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools) capability.

Here's how it works:

1. **Initialization**: When an MCP client (such as Claude Desktop or VS Code) connects, it queries each configured MCP server for its capabilities ([the initialization phase](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle#initialization)).

    Exograph declares the `tools` capability during the initialization phase.

2. **Tools discovery**: For each capability declared, the MCP client asks for more details. For the tools capability, it asks for the [tools list](https://modelcontextprotocol.io/specification/2025-06-18/server/tools#listing-tools).

    Exograph responds to this request by sending the list of tools along with their descriptions. The descriptions include the GraphQL schema, which the LLM can use to form valid queries. If you added [doc comments](/core-concept/file-definition.md#documentation-comments) to your Exograph model, those are also included.

3. **Query Execution**: When a user submits a prompt, the MCP client passes the tools information along with that prompt to the LLM. The LLM may request the MCP client to invoke tools one or more times.

    For Exograph, the tool invocation includes the GraphQL query and variables, which are invoked in the normal manner. Exograph's standard access control mechanism ensures that the LLM gets access to only the data permitted by the user.

## Exograph's Tools

By default, the MCP server operates in `combined` mode. In this mode, the MCP server offers a single tool: `execute_query`. Its description gives LLMs sufficient information to form appropriate queries. Specifically, the description includes the GraphQL schema. Exograph also offers `separate` mode, where it offers two tools: `execute_query` and `introspect`. To set the mode, set the `EXO_MCP_MODE` environment variable to `separate` or `combined`.

```sh
EXO_MCP_MODE=separate exo dev
```

By default, Exograph's MCP server doesn't expose any mutations (even if access control allows them). However, you can enable mutations selectively by [creating a profile](profiles.md).

## Disabling MCP API

Exograph's MCP server is enabled by default. To disable the MCP endpoint, set the `EXO_ENABLE_MCP` environment variable to `false`.
