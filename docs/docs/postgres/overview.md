---
sidebar_position: 0
slug: /postgres
---

# Overview

The Postgres plugin allows you to define a module that maps to a Postgres database. The module may contain type definitions, which Exograph maps to tables in the database.

In this section, we will explore:

- [Defining a Postgres module](defining-modules.md), which follows the general [abstraction of a module](/core-concept/module.md) in Exograph.
- [Defining types](defining-types.md) along with relationships between types.
- [Customizing types](customizing-types.md), since mapping types to tables in the database often requires some customization such as tweaking the name of the table.
- [Queries](operations/queries.md) and [mutations](operations/mutations.md) and how they are generated.
- [Access control](access-control.md) and how it applies to queries and mutations.
- [Configuration](configuration.md) options using environment variables.
