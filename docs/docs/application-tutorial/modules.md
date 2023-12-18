---
sidebar_position: 30
---

# Sending Notifications

Let's create a module to send emails to announce a new concert. To keep things simple, we will not integrate with an SMTP server. Instead, we will just print the email to the console (see the 'snippets' directory for a complete example of sending emails using SMTP). We will use TypeScript to define the module (the other choice would be JavaScript).

The module will expose a single function, `sendEmail`, which takes a single argument, `concertId`, which is the concert we want to notify. The implementation will need to query the database to get the concert details as well as the email addresses (from the `Subscriber` model). Effectively, we want the implementation to have access to the queries. We make this possible by adding another argument `@inject exograph: Exograph`. This will supply the `exograph` object through which you can execute queries. Note that the mutation exposed through GraphQL will still have only one argument--`concertId`. In other words, the injected arguments are not exposed through the GraphQL mutation.

Note the `@access` annotation on the mutation. We want to restrict access to this mutation to only users with the "admin" role. We can do that by using `AuthContext` defined [earlier](model.md).

```exo
@deno("email.ts")
module Email {
  @access(AuthContext.role == "admin")
  mutation sendEmail(concertId: Int, @inject exograph: Exograph): Boolean
}
```

Before we jump into implementing the module, let's create the `Subscriber` model.

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

Here, for simplicity, we restrict access to the `Subscriber` model to only users with the "admin" role. In a real application, you would implement a subscription flow with a confirmation email.

With `exo yolo` watching, you see a new file `email.ts` generated for you (it is just a starter code; you can also manually create it). Open that file and replace it with the following (essentially, we are supplying the function body):

```ts
import type { Exograph } from "./exograph";

export async function sendEmail(
  concertId: number,
  exograph: Exograph
): Promise<boolean> {
  const concertOperation = await exograph.executeQuery(
    `
		query($concertId: Int!) {
			concert(id: $concertId) {
				title
			}
		}`,
    {
      concertId: concertId,
    }
  );

  const subscribersOperation = await exograph.executeQuery(`
		{
			subscribers(where: {subscribed: {eq: true}}) {
				email
			}
		}`);

  const concertTitle = concertOperation.concert.title;
  const subscriberEmails = subscribersOperation.subscribers.map(
    (subscriber) => subscriber.email
  );

  const emailBody = `
		<h1>${concertTitle}</h1>
		<p>You have been invited to the concert!</p>
	`;

  console.log(`Sending email ${emailBody} to ${subscriberEmails.join(", ")}`);

  return true;
}
```

We execute two queries in the module: one to get the concert details and one to get the list of subscribers. Then we form the email body and send the email to the addresses we obtained.

A quick recap:

- The access rule for the mutation requires that the user has the "admin" role.
- The access rule for the `Subscriber` type also requires that the user has the "admin" role.
- The `exograph` object is injected into the `sendEmail` function. This object exposes the `executeQuery` method, which can be used to execute queries. The `executeQuery` method returns a promise which resolves to the result of the query.
- The `executeQuery` method takes two arguments: the GraphQL query and the variables. The variables are optional. In this case, we are passing the `concertId` as a variable for the first query and leaving it empty for the second query.
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
  sendEmail(concertId: 1)
}
```

In the console where `exo dev` is watching, you will see the following:

```
Sending email
                <h1>An evening vocal concert</h1>
                <p>You have been invited to the concert!</p>
         to foo@example.com, baz@example.com
```

Note that since we filtered the subscribers to only those who are subscribed, we only sent the email to those. Specifically, 'bar@example.com' has not subscribed, so it did not receive the email.
