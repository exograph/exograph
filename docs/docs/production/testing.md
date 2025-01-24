---
sidebar_position: 3
---

# Testing

Testing ensures the correctness of current behavior and offers protection against regression as you add more functionality. While Exograph cuts down on the code you need to write considerably, you are still writing access control expressions, Deno modules, and interceptors. Therefore, you should test it to make sure that your code does what you expect it to do.

Exograph includes a declarative testing framework allowing you to write operations and their expected output. Then, Exograph will do the rest: setting up the database, starting a new Exograph server, running the operations, comparing the result, and so on. Exograph will also run your tests in parallel to make all this happen fast.

## What should you test?

Before we get into the details of how to write tests, let's talk about what you should test. Since Exograph is a declarative framework that does most of the heavy lifting, you need to write far fewer tests than you would if you were writing a GraphQL API from scratch. For example, you don't need to write tests for simple queries and mutations.

:::note
Exograph itself extensively uses this framework to test its code. You can find those tests in [its Github repository](https://github.com/exograph/exograph/tree/master/integration-tests). Note, however, that the purpose of those tests is to ensure the correctness of Exograph, so you will find tests that check for simple queries or correct error messages. Those tests help us ensure that similar functionality will work as expected in your code--and that you shouldn't need to write such tests for your code!
:::

So we recommend that you focus your testing efforts on the following:

- **Access control**: Since it is crucial to ensure that data is only accessible to the right users, you should test your access control expressions. Even here, you may skip testing simple role-based expressions such as `AuthContext.role == "admin"`. However, if you have more complex expressions that use the content of an entity or combine multiple sub-expressions, you should test them. Exograph provides a way to emulate users with different identities and roles to simplify testing for access.

- **Deno modules**: If you have anything but simple Deno code, you should test those. Since you are writing JavaScript/TypeScript code there, there is room for bugs. In addition to declarative testing through Exograph, you may also write unit tests for that code.

## Example

You write Exograph tests in "exotest" files. It is a YAML file with sections describing GraphQL operations and expected results. Let's explore this by writing a simple test to ensure that a non-admin user can see only published venues.

First, we let's seed some data so that tests don't start from a blank slate. We do that by writing an "init.gql" file (we will examine details [later](#initializing-seed-data)). Here, we add a few mutations to create venues. Since we are writing an integration test, any access has to play by the access control rules. Since the rule for the `Venue` type allows only admins to create venues, we need to emulate an admin user. We do that by specifying the `auth` section.

```graphql file=tests/init.gql
operation: |
    mutation {
        v1: createVenue(data: {name: "V1", published: true}) {
            id
        }
        v2: createVenue(data: {name: "V2", published: false}) {
            id
        }
        v3: createVenue(data: {name: "V3", published: true}) {
            id
        }
        v4: createVenue(data: {name: "V4", published: false}) {
            id
        }
    }
auth: |
    {
        "role": "admin"
    }
```

Next, we write the test, asserting that users with the `user` role see only the published venues. The `operation` section includes the GraphQL query we want to test. The `response` section contains the expected response. We also specify the `auth` section to emulate a user with the `user` role.

```graphql file=tests/venue-user-access.exotest
operation: |
  query {
      venues(orderBy: {id: ASC}) {
          id
          name
          published
      }
  }
auth: |
  {
      "role": "user"
  }
response: |
  {
    "data": {
      "venues": [
          {
              "id": 1,
              "name": "V1",
              "published": true
          },
          {
              "id": 3,
              "name": "V3",
              "published": true
          }
      ]
    }
  }
```

Now let's run the test!

```shell-session
# shell-command-next-line
exo test
* Running tests in directory .  ...
** Running integration tests
Launching PostgreSQL locally...
(venue-user-access)
 ::  Initializing schema for venue-user-access ...
(venue-user-access)
 ::  Initializing exo-server ...
(venue-user-access)
 ::  Initializing database...
(venue-user-access)
 ::  Testing ...
(venue-user-access)
 ::  PASS

* Test results: PASS. 1 passed out of 1 total in 1 seconds (16 cpus)
```

Everything looks good (you can try changing something in the response and see how `exo test` reports failures).

Let's write one more test to make sure that admins can see all venues:

```graphql file=tests/venue-access-admin.exotest
operation: |
  query {
      venues(orderBy: {id: ASC}) {
          id
          name
          published
      }
  }
auth: |
  {
      "role": "admin"
  }
response: |
  {
    "data": {
      "venues": [
          {
              "id": 1,
              "name": "V1",
              "published": true
          },
          {
              "id": 2,
              "name": "V2",
              "published": false
          },
          {
              "id": 3,
              "name": "V3",
              "published": true
          },
          {
              "id": 4,
              "name": "V4",
              "published": false
          }
      ]
    }
  }
```

Now let's rerun the tests:

```shell-session
# shell-command-next-line
exo test
* Running tests in directory .  ...
** Running integration tests
Launching PostgreSQL locally...
(venue-admin-access)
 ::  Initializing schema for venue-admin-access ...
(venue-admin-access)
 ::  Initializing exo-server ...
(venue-admin-access)
 ::  Initializing database...
(venue-admin-access)
 ::  Testing ...
(venue-user-access)
 ::  Initializing schema for venue-user-access ...
(venue-user-access)
 ::  Initializing exo-server ...
(venue-user-access)
 ::  Initializing database...
(venue-user-access)
 ::  Testing ...
(venue-admin-access)
 ::  PASS

(venue-user-access)
 ::  PASS

* Test results: PASS. 2 passed out of 2 total in 4 seconds (16 cpus)
```

Here too, everything looks good. We can also see that the tests are run in parallel (the interleaved output is due to how Exograph runs tests in parallel).

## Filtering tests

If you have a lot of tests, you may run only tests that have the word "user" in their name by supplying a filter:

```shell-session
# shell-command-next-line
exo test . "*user*"
* Running tests in directory . with pattern '*user*' ...
** Running integration tests
Launching PostgreSQL locally...
(venue-user-access)
 ::  Initializing schema for venue-user-access ...
(venue-user-access)
 ::  Initializing exo-server ...
(venue-user-access)
 ::  Initializing database...
(venue-user-access)
 ::  Testing ...
(venue-user-access)
 ::  PASS

* Test results: PASS. 1 passed out of 1 total in 2 seconds (16 cpus)
```

Note the quotes around `*user*` to avoid shell expansion.

## Initializing seed data

As discussed in the [example](#example) section, it is often a good idea to seed the database with some data before running the tests. Exograph provides a way through "gql" files. You can write files with names starting with `init` and with the `.gql` extension. Exograph will execute these files before running tests. If you have multiple matching files, Exograph will execute them in alphabetically sorted order. For example, if you have `init-1.gql` and `init-2.gql`, `init-1.gql` will be executed first.

## Arranging tests in folders

Once you write more than a handful of tests, you may want to organize them in folders. For example, you may want to have a folder that tests the `Concert` type and another folder for tests that test the `Artist` type. You can create a folder and put the tests in it. For example, if you have the following folder structure:

```
├── src
│   ├── index.exo
├── tests
│   ├── init.gql
│   ├── artists
│   │   ├── artist-access-admin.exotest
│   │   └── artist-access-user.exotest
│   ├── concerts
│   │   ├── concert-access-admin.exotest
│   │   └── concert-access-user.exotest
```

The name of each test will include the path to the test file. For example, the name of the test in `artist-access-admin.exotest` will be `tests/artist/artist-access-admin`. You can use this name to filter the tests. For example, if you want to run only the tests that have the word "artist" in their name, you can do the following:

```shell-session
# shell-command-next-line
exo test "*/artist/*"
* Running tests in directory . with pattern '*/artist/*' ...
** Running integration tests
```

Any folder may include additional init files to include additional seed data for the tests in that folder. Exograph will execute them after init files in the parent folder.

As you may expect, you can nest folders as deep as you want.

## Abstracting values

So far, we have used hard-coded values when specifying parameters and expected results. However, that is not a robust solution. For example, Postgres doesn't guarantee that the values for a SERIAL column will be strictly sequential. For example, the first row may have id 1, the second row may have id 5, and the third row will have id 10. Furthermore, for UUIDs, you can't even predict the value.

To solve this problem, Exograph provides a way to abstract values through the `@bind()` directive and then refer to them in the parameters and expected results.

For example, if you want to bind the value of the `id` field of venues, you can use the following `init.gql` file:

```graphql
operation: |
    mutation {
        v1: createVenue(data: {name: "V1", published: true}) {
            id @bind(name: "v1_id")
        }
        v2: createVenue(data: {name: "V2", published: false}) {
            id @bind(name: "v2_id")
        }
        v3: createVenue(data: {name: "V3", published: true}) {
            id @bind(name: "v3_id")
        }
        v4: createVenue(data: {name: "V4", published: false}) {
            id @bind(name: "v4_id")
        }
    }
auth: |
    {
        "role": "admin"
    }
```

Here, each of the ids is bound to a name. You can then refer to these names in the parameters and expected results. For example, if you want to test that the `venues` query returns only the published venues, you can use the following `venues.exotest` file:

```graphql
operation: |
  query {
      venues(orderBy: {id: ASC}) {
          id
          name
          published
      }
  }
auth: |
  {
      "role": "user"
  }
response: |
  {
    "data": {
      "venues": [
          {
              "id": $.v1_id,
              "name": "V1",
              "published": true
          },
          {
              "id": $.v3_id,
              "name": "V3",
              "published": true
          }
      ]
    }
  }
```

Here, instead of hard-coding the ids, we refer to the names specified in `@bind`: `$.v1_id` and `$.v2_id`

## Implementing custom assertions

All the assertions we have seen so far use the equality operation to test the actual and expected values. That is sufficient for most cases. However, sometimes you may want to implement custom assertion logic. For example, if you have a service that returns the temperature, you may want to test that the value falls within a specific range. You can do that by supplying a custom assertion function.

```graphql
...
response: |
  {
    "data": {
      "temperature": (actual) => {
          return actual >= 20 && actual <= 30
      }
    }
  }
```

Here, instead of supplying an object, we supply a function. The function takes the actual value as an argument and returns a boolean indicating whether the assertion passes.

If you want to use an externally implemented function to implement the assertion, you can import that code using the `deno` element. For example, if you want to assert the validity of a UUID, you can use the following `uuid.exotest` file:

````graphql
- deno: |
    import { v4 } from "https://deno.land/std@0.140.0/uuid/mod.ts";

- ...

- response: |
    {
        "data": {
            "uuid": (actual) => {
                for (const element of actual) {
                    if (!v4.validate(element.id)) {
                        throw new ExographError(id + " is not a valid UUID")
                    }
                }
                return true;
            }
        }
    }
    ```
````

By importing the `v4` function from the `uuid` module, you bring that code into your test file. You can then use it to implement the assertion.

## Adding invariants

Often, especially when testing access control rejection with mutations, you want to ensure that the system remains in the same state as before the operation. For example, if you are testing that a user without the `admin` role cannot create a new `Product`, you want to ensure that no new product enters the database. You can do that by adding invariants.

Invariants are specified using the `invariants` section of the test file. For example, if you want to ensure that all products remain the same after a mutation, you can specify an invariant that queries the products and compares the result with the previous state:

```graphql file=tests/create-product-user.exotest
invariants:
  - path: system-state.gql
operation: |
  mutation {
      createProduct(data: {name: "P1", price: 100}) {
          id
      }
  }
auth: |
  {
      "role": "user"
  }
...
```

Here, the `path` is relative to the test file.

The format for the invariant file is the same as [init files](#initializing-seed-data). For example, to ensure that all products and departments remain the same, you can use the following `system-state.gql` file:

```graphql file=tests/system-state.gql
- operation: |
    query {
        products @unordered {
            id
            name
        }
        departments @unordered {
            id
            name
        }
    }
- auth: |
    {
        "role": "admin"
    }
```

Exograph will execute the invariant operations before and after each test. If the results do not match, the test is considered to have failed.


<!-- TODO: Multi-stage tests -->
