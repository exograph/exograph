---
sidebar_position: 20
---

# Customizing tools

As we saw in the [how it works](/mcp/how-it-works) section, Exograph's MCP server offers the `execute_query` tool in `combined` mode and an additional `introspect` tool in `separate` mode. This is often sufficient when using Exograph directly from Claude Desktop or similar MCP clients. However, when using Exograph as part of an agentic workflow, you may want finer control over the tools to focus on specific parts of the schema.

Profiles allow you to specify which types, queries, and mutations are exposed through MCP tools.

## Specifying profiles

By default, all queries and no mutations are exposed. You can customize this behavior by creating profiles. In agentic workflows, this allows the agent to examine the prompt and context to select relevant tools. For example, if the prompt is "Get me the list of branches", there's little point in offering customer-related queries to the LLM. When you anticipate specific prompt patterns (often governed by evaluation criteria), you can create profiles that improve agent performance.

:::tip
Profiles relate closely to [bounded context](https://martinfowler.com/bliki/BoundedContext.html) from domain-driven design. You form boundaries around related domain parts and expose only tools specific to that bounded context. Agents can then choose appropriate tools based on the prompt and context.
:::

You specify profiles in the `exo.toml` file at the root of your project (for other usages of the `exo.toml` file, see [here](/cli-reference/config)).

```toml
[[mcp.profiles]]
name = "branch_management"
```

For each profile, Exograph exposes the `execute_query_<profile_name>` tool (and corresponding introspect tool in `separate` mode). The above profile exposes the `execute_query_branch_management` tool (and `introspect_branch_management` tool in `separate` mode). If you don't specify other attributes, the tool exposes all queries and no mutations.

You can control exposed queries and mutations using include/exclude patterns. Each property accepts an array of [wildcard patterns](https://docs.rs/globset/latest/globset/struct.Pattern.html). For example, `"branch*"` matches `branches`, `branch`, `branchAgg`, etc. Use `"*"` to match all types or operations.

**Query Control Properties:**
- `queries.operations.include`: Include queries matching pattern (default: `["*"]` - all queries)
- `queries.operations.exclude`: Exclude queries matching pattern (default: `[]` - none excluded)
- `queries.models.include`: Include models matching pattern (default: `["*"]` - all models)
- `queries.models.exclude`: Exclude models matching pattern (default: `[]` - none excluded)

**Mutation Control Properties:**
- `mutations.operations.include`: Include mutations matching pattern (default: `[]` - none included)
- `mutations.operations.exclude`: Exclude mutations matching pattern (default: `[]` - none excluded)
- `mutations.models.include`: Include models matching pattern (default: `[]` - none included)
- `mutations.models.exclude`: Exclude models matching pattern (default: `[]` - none excluded)

Let's see how to use each of these properties.

### Restricting queries by name

You can restrict queries by name using the `queries.operations.include` and `queries.operations.exclude` properties. For example, to expose only the `branches` query and all queries starting with `customer`:

```toml
[[mcp.profiles]]
name = "branch_management"
queries.operations.include = ["branches", "customer*"]
```

This configuration exposes the `branches` query and all queries starting with `customer` while excluding others like `branch`, `branchAgg`, and `financialAdvisors`.

Alternatively, you can exclude queries by name. For example, to expose all queries except aggregate queries:

```toml
[[mcp.profiles]]
name = "non_aggregate_queries"
queries.operations.exclude = ["*Agg"]
```

You can combine both properties to create a profile that exposes only the `branches` query and all queries starting with `customer`, except for aggregate queries:

```toml
[[mcp.profiles]]
name = "branch_management_non_aggregates"
queries.operations.include = ["branches", "customer*"]
queries.operations.exclude = ["*Agg"]
```

Query names refer to GraphQL operation names. For Postgres modules, query names follow the naming conventions described in [queries](/postgres/operations/queries.md) and [mutations](/postgres/operations/mutations.md).

### Restricting queries by type

Often, you'll want to control queries based on their domain model using the `queries.models.include` and `queries.models.exclude` properties.

For example, to expose only queries related to the `Branch` type:

```toml
[[mcp.profiles]]
name = "branch_management"
queries.models.include = ["Branch"]
```

This configuration exposes all queries that return a single `Branch` or list of `Branch` entities, such as `branch`, `branches`, `branchAgg`, etc.

You can also exclude models by name. For example, to exclude all models starting with `Membership`:

```toml
[[mcp.profiles]]
name = "non_membership_queries"
queries.models.exclude = ["Membership*"]
```

You can combine all these properties to create profiles that control exposure based on both names and types.

### Restricting mutations

Controlling exposed mutations works similarly. However, by default, Exograph doesn't expose any mutations. You can enable mutations selectively by creating profiles.

For example, to enable specific customer management operations while excluding delete operations:

```toml
[[mcp.profiles]]
name = "customer_management"
mutations.models.include = ["Customer"]
mutations.operations.include = ["createCustomer", "updateCustomer"]
mutations.operations.exclude = ["deleteCustomer"]
```

This profile exposes all Customer-related mutations while specifically excluding the `deleteCustomer` operation, providing a safe subset for customer management workflows.


