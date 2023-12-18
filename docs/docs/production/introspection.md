---
sidebar_position: 3
---

# Disabling Introspection

Turning off introspection support in production is generally considered a best practice since it is an additional security measure to make a hacker's job harder.

:::warning
While turning off introspection in production is a good idea, it offers only limited help in securing your application. At best, this security-by-obscurity measure makes playing with your APIs to explore vulnerabilities harder. At worst, it gives a false sense of security. Since it is still possible to predict the APIs (for example, by simply looking at the network traffic from a browser's "Inspect" tab), ensuring the correctness of your access control expressions and taking other security measures is still critical.
:::

To turn off introspection support, set the `EXO_INTROSPECTION` environment variable to `false` (the default value in production). So, all you need to do is ensure that this environment variable isn't set to `true` in production.
