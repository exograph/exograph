---
sidebar_position: 40
---

# Clerk Authentication

Exograph's playground supports Clerk authentication, including support for templates (to customize the JWT claims). This allows you to test APIs that require authentication without building a UI.

You don't need to do anything special to enable this support. Start the Exograph server by specifying the OIDC URL as an environment variable to `exo` or `exo-server` command as usual. If you set `EXO_OIDC_URL` that ends in `clerk.accounts.dev`, it will show the Clerk UI.

:::note Short-lived JWT tokens
Clerk's JWT tokens are short-lived (by default, 60 seconds). This shortens the time window for a revoked user or changed claims to be effective. In a typical client, you would use Clerk's `useAuth` to obtain the latest JWT token (here is an [example](https://github.com/exograph/examples/blob/481555d3936d3fd92b64a761cc0774d5eb8104a2/todo-with-nextjs-clerk-auth/web/src/app/providers.tsx#L29) of such use).

Exograph's playground follows the same pattern, so a new JWT token is obtained for each request. If you delete the user or change their claims, you will see its effect shortly.

This also makes Exograph's playground support valuable. Without it, copying the JWT token from the UI and passing it in the `Authorization` header would require doing so every minute or configuring the JWT token to be longer-lived.
:::

## Setting up Clerk

Set up a Clerk project by following the [instructions](https://clerk.com/docs/quickstarts/setup-clerk) and note down the value for "Publishable key" and "JWT public key/JWKS URL" from the Clerk dashboard under the "API Keys" section. If you wish to add additional claims to the JWT token, you can create a [template](https://clerk.com/docs/backend-requests/making/jwt-templates#introduction) and note down the template id. For example, if you wanted to get the `role` in the claims, you would create a template with the following content:

```json
{
  "role": "{{user.public_metadata.role}}"
}
```

## Signing in

The first time you attempt to authenticate in the playground, it will ask for the "Publishable key" you noted earlier. You may specify the [template id](https://clerk.com/docs/backend-requests/making/jwt-templates#introduction).

Later, if you want to change this information, you can do it through the "Configure Clerk" link in the sign-in dialog.

Once configured, you will see the sign-in dialog. Sign in using any authentication method (e.g., email/password, Google Sign-in, etc.). Once you sign in, you will see the user's profile picture near the key icon to indicate the currently signed-in user, and hovering over it will show more information about the user.

## Making a request

You make requests as usual. The playground will create a JWT token and pass it to each request in the `Authorization` header.

## Signing out

To sign out, click the same "Authenticate" button and "Sign Out".

Here is an example of this functionality in action:

import playgroundVideo from './images/clerk-playground.mp4';

<video controls width="100%">
  <source src={playgroundVideo}/>
</video>
