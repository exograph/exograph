---
title: Symmetric Key Authentication
sidebar_position: 20
---

Exograph's playground support for symmetric key authentication allows specifying JWT claims and the secret to sign the JWT token. Then, for each request, it creates a JWT token with the specified claims and passes it in the `Authorization` header.

For example, if you wanted to test an API that requires the `role` claim to be set to `admin`, you would proceed as follows.

## Signing in

In the playground, click on the "Authenticate" button in the middle center of the screen. That will pop up a dialog box. In the dialog box, enter the following:

- For "Secret", enter the secret printed by the `exo yolo` command or the value of the `EXO_JWT_SECRET` environment variable passed to either `exo yolo`, `exo dev`, or `exo-server` command.
- For "Claims", enter the following:

```json
{
  "role": "admin"
}
```

And click "Sign In".

## Making a request

You make requests as usual. The playground will create a JWT token and pass it to each request in the `Authorization` header.

## Signing out

To sign out, click the same "Authenticate" button and "Sign Out".

Here is an example of this functionality in action:

import symmetricAuthPlayground from './images/symmetric-auth-playground.mp4';

<video controls width="100%">
  <source src={symmetricAuthPlayground}/>
</video>
