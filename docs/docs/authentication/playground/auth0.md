---
sidebar_position: 30
---

# Auth0 Authentication

Exograph's playground supports Auth0 authentication. This allows you to test APIs that require authentication without building a UI.

You don't need to do anything special to enable this support. Start the Exograph server by specifying the OIDC URL as an environment variable to `exo` or `exo-server` command as usual. If you set `EXO_OIDC_URL` that ends in `auth0.com`, the playground will show the Auth0 UI.

## Setting up Auth0

Set up an Auth0 project by following the [instructions](https://auth0.com/docs/quickstart/spa/react/interactive). Note down the value for "Domain" and "Client ID" from the Auth0 dashboard under the "Settings" section.

Pay particular attention to configuring "Allowed Callback URLs" (you must set it to include the playground URL beside your web application URLs. For example, assuming that your web app is running locally on port 300 and if you are running `exo` or `exo-server` locally and with the default port, you would set it to `http://localhost:9876/playground, http://localhost:3000`).

## Signing in

The first time you attempt to authenticate in the playground, it will ask for the "Domain" and "Client ID" you noted earlier. Later, if you want to change this information, you can do it through the "Configure Auth0" link in the sign-in dialog.

Then, sign in as usual. Once you sign in, you will see the user's profile picture near the key icon to indicate the currently signed-in user, and hovering over it will show more information about the user.

## Making a request

You make requests as usual. The playground will create a JWT token and pass it to each request in the `Authorization` header.

## Signing out

To sign out, click the same "Authenticate" button and "Sign Out".

Here is an example of this functionality in action:

import playgroundVideo from "./images/auth0-playground.mp4";

<video controls width="100%">
  <source src={playgroundVideo} />
</video>
