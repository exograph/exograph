---
sidebar_position: 2
---

# Using External Packages

The examples in the previous example used basic JavaScript functionality to add or square numbers. However, in real-world applications, you will often need a lot more: sending emails, processing payments, conversing with OpenAI, etc. To facilitate such possibilities, Exograph supports using external packages from the [Deno](https://deno.land/x) package registry and the [npm](https://www.npmjs.com) registry.

## Overview

Using external packages in Exograph is straightforward. You can import and use any package from the Deno package registry or the npm registry.

For example, if you want to use the [`case`](https://deno.land/x/case) package from the Deno package registry, you can import it as follows:

```typescript
import { camelCase } from "https://deno.land/x/case/mod.ts";
```

Then, use the `camelCase` function in your code as usual.

Similarly, if you want to use an npm package, you can import it as follows:

```typescript
import Color from "npm:color";
```

Note the `npm:` prefix in the import statement, which tells Exograph to look for the package in the npm registry.

## Example: Sending Emails using SMTP

While developing the [application tutorial](../application-tutorial/modules.md), we opted to print the email to the console instead of using an email server. Let's revisit that example to make it real!

Recall the definition of the `sendEmail` function from the tutorial:

```typescript
async function sendEmail(
  to: string[],
  subject: string,
  body: string
): Promise<boolean> {
  console.log(
    `Sending email
     to: ${to.join(", ")},
     subject: ${subject},
     body: ${body}`
  );
  return true;
}
```

We will replace the `console.log` statement with logic to send emails through a server. We will accomplish this in two ways: using the SMTP protocol and a provider-specific API. For the implementation, we will assume:
- The `to` array contains the recipients' email addresses, so we will use the `bcc` field to send the email to multiple recipients. 
- The necessary environment variables have been set (you could add a check for those and throw an error if they aren't).

### Using SMTP

First, let's send email using the SMTP protocol. For that, we will use the [`nodemailer`](https://nodemailer.com/) npm package to send emails.

```typescript
import nodemailer from "npm:nodemailer@6.9.8";

import type { Exograph } from "./exograph";

export async function sendNotification(
  concertId: number,
  exograph: Exograph
): Promise<boolean> {
  // same as before
}

async function sendEmail(
  to: string[],
  subject: string,
  body: string
): Promise<boolean> {
  const transport = nodemailer.createTransport({
    host: Deno.env.get("EMAIL_HOST"),
    port: Deno.env.get("EMAIL_PORT"),
    secure: true,
    auth: {
      user: Deno.env.get("SMTP_USER")!,
      pass: Deno.env.get("SMTP_PASS")!,
    },
  });

  await transport.sendMail({
    from: Deno.env.get("EMAIL_FROM")!,
    to: "noreply@example.com",
    bcc: to,
    subject: subject,
    html: body,
  });

  return true;
}
```

Once we import the `nodemailer` package, we can use it as usual in our code. If you have the necessary environment variables set, you can run the `sendNotification` mutation in GraphQL Playground to send an email.

### Using Resend

An alternative to using the SMTP protocol is to use a provider-specific API. Most email providers typically offer REST APIs to send emails (in addition to the SMTP protocol). While using the REST APIs directly is possible, most also provide an SDK to make the process easier. Let's reimplement the `sendEmail` function using the [`resend`](https://resend.com/) as the provider and the [`resend` npm package](https://www.npmjs.com/package/resend).

```typescript
import { Resend } from "npm:resend";

const resend = new Resend(Deno.env.get("RESEND_API_KEY")!);

async function sendEmail(
  to: string[],
  subject: string,
  body: string
): Promise<boolean> {
  await resend.emails.send({
    from: Deno.env.get("EMAIL_FROM")!,
    to: "noreply@example.com",
    bcc: to,
    subject: subject,
    html: body,
  });

  return true;
}
```

In this example, we import the `resend` package and use it as usual in our code. If you have the necessary environment variables set, you can run the `sendNotification` mutation in GraphQL Playground to send an email.
