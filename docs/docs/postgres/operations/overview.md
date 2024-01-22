---
sidebar_position: 10
---

# Overview

So far, we have defined types in a Postgres module. Now, it is time to reap its benefits.

In this section, we will explore all of these queries and mutations. Since queries and mutations return data, we will first take an [overview](overview.md) of how Exograph structures these operations and then dive deeper into [queries](queries.md) and [mutations](mutations.md).

For each type, Exograph automatically infers queries and mutations. Specifically, Exograph infers queries to obtain a single entity or a list of entities matching a filter. It also infers mutations to create, update, and delete an entity.

But first, we will examine at how Exograph [shapes](data-shape.md) the data returned by queries and mutations.
