---
title: Overview
sidebar_position: 10
---

# Overview

Without Exograph, testing authentication support through the playground is complex. You need a UI with at least the login functionality. Then, you can grab the JWT token from the UI and pass it in the `Authorization` header. Exograph Playground makes it easy to try out APIs that require authentication by integrating Auth0's and Clerk's UI in the playground and passing the JWT token to each request.

The playground automatically detects the authentication mechanism based on environment passed to `exo` or `exo-server` command. If you set:

- `EXO_JWT_SECRET`, it will show the [symmetric key authentication UI](./symmetric.md).
- `EXO_OIDC_URL`, it will show either [Auth0](./auth0.md) or [Clerk](./clerk.md) UI based on the URL.
