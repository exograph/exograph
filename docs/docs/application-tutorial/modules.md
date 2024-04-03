---
sidebar_position: 30
---

# Sending Notifications

Let's create a module to send emails to announce a new concert. We will keep things simple by printing the email to the console instead of using an email server. Later, when we dive deeper into Deno support, we will [integrate with an email server using npm package](../deno/external-packages#example-sending-emails).

We will use TypeScript to define the module (the other choice would be JavaScript).

The module will expose a single function, `sendNotification`, which takes a single argument, `concertId`, which is the concert we want to notify. The implementation will need to query the database to get the concert details and the email addresses (from the `Subscriber` model). Effectively, we want the implementation to have access to the queries. We make this possible by adding another argument `@inject exograph: Exograph`. The Exograph runtime will supply the `exograph` object through which you can execute queries. Note that the mutation exposed through GraphQL will still have only one argument--`concertId`. In other words, the injected arguments are not exposed through the GraphQL mutation.

Note the `@access` annotation on the mutation. We want to restrict access to this mutation to only users with the "admin" role. We can do that by using `AuthContext` defined [earlier](model).

```exo
@deno("notification.ts")
module NotificationService {
  @access(AuthContext.role == "admin")
  mutation sendNotification(concertId: Int, @inject exograph: Exograph): Boolean
}
```

Before implementing the module, let's create the `Subscriber` model.

```exo
@postgres
module ConcertData {
  ...

  @access(AuthContext.role == "admin")
  type Subscriber {
    @pk id: Int = autoIncrement()
    email: String
    subscribed: Boolean
  }
}
```

Here, for simplicity, we restrict access to the `Subscriber` model to only users with the "admin" role. In an actual application, you would implement a subscription flow with a confirmation email.

With `exo yolo` watching, you see a new file `notification.ts` generated for you (it is just a starter code; you can also manually create it). Open that file and replace it with the following (essentially, we are supplying the function body):

```ts title="notification.ts"
import type { Exograph } from "./exograph";

export async function sendNotification(
  concertId: number,
  exograph: Exograph
): Promise<boolean> {
  const concertOperation = await exograph.executeQuery(
    `query($concertId: Int!) {
        concert(id: $concertId) {
          title
        }
     }`,
    {
      concertId: concertId,
    }
  );

  const subscribersOperation = await exograph.executeQuery(
    `{
       subscribers(where: {subscribed: {eq: true}}) {
         email
       }
     }`
  );

  const concertTitle = concertOperation.concert.title;
  const subscriberEmails = subscribersOperation.subscribers.map(
    (subscriber) => subscriber.email
  );

  const emailBody = `
    <h1>${concertTitle}</h1>
    <p>You have been invited to the concert!</p>
  `;

  return await sendEmail(subscriberEmails, "Concert Announcement", emailBody);
}

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

We execute two queries in the module: one to get the concert details and one to get the list of subscribers. Then, we compose the email body and send the email to the obtained addresses.

A quick recap:

- The mutation's access rule requires the user to have the "admin" role.
- The access rule for the `Subscriber` type also requires that the user has the "admin" role.
- The Exograph runtime injects the `exograph` object to the `sendNotification` function. This object exposes the `executeQuery` method, which the implementation may use to execute queries. The `executeQuery` method returns a promise that resolves to the query result.
- The `executeQuery` method takes two arguments: the GraphQL query and the variables. The variables are optional. In this case, we pass the `concertId` as a variable for the first query and leave it empty for the second query.
- Even in the TypeScript code, all queries and mutations still follow the same access control rules. In this case, since the invoker of the mutation must be an admin, the queries will also be executed as an admin. If you need to bend the rule, Exograph allows you to do so in a principled way. See the [ExographPriv](/deno/injection.md#the-exographpriv-object) section for more details.

Add a couple of subscribers to the database through our GraphiQL interface.

```graphql
mutation {
  createSubscribers(
    data: [
      { email: "foo@example.com", subscribed: true }
      { email: "bar@example.com", subscribed: false }
      { email: "baz@example.com", subscribed: true }
    ]
  ) {
    id
  }
}
```

And now send the email for the concert with id 1.

```graphql
mutation {
  sendNotification(concertId: 1)
}
```

In the console where `exo dev` is watching, you will see the following:

```
Sending email
     to: foo@example.com, baz@example.com,
     subject: Concert Announcement,
     body:
    <h1>An evening vocal concert</h1>
    <p>You have been invited to the concert!</p>
```

Note that since we filtered the subscribers to only those who are subscribed, we only sent the email to those. Specifically, 'bar@example.com' has not subscribed, so it did not receive the email.
