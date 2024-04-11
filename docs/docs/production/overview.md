---
sidebar_position: 0
slug: /production
---

# Overview

When putting your Exograph server into production, you will need to pay attention to a few key areas to ensure that your server is secure, reliable, and performant. This section covers the following topics:

- [Disabling introspection](introspection.md): This makes a hacker's job harder by hiding the server's schema.
- [Limiting the API surface](trusted-documents.md): While disabling introspection is a good start, it is not enough. Exograph offers to limit the API surface to only queries and mutations that you use from your client applications through the concept of trusted documents (also known as "persisted operations" or "persisted queries").
- [Testing](testing.md): Exograph offers a simple yet effective way to test your server using a declarative approach. This ensures that your access control rules and custom business logic are working as expected.
- [Telemetry](telemetry.md): Once you put your server into production, you will need to monitor its usage. Exograph offers OpenTelemetry integration to monitor your server's performance and usage.
