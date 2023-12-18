---
title: Getting Started
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
import InstallOsDetector from './InstallOsDetector';

<InstallOsDetector/>

Let's set up Exograph and create a simple application to ensure everything works as expected.

## Setting up

### Install Prerequisites

Install either [Postgres](https://www.postgresql.org/download/) or [Docker](https://docs.docker.com/install). We need one of these to enable the "yolo" mode during development.

### Install Exograph

<Tabs groupId="install-os">
  <TabItem value="mac-linux" label="Mac and Linux">

```shell-session
# shell-command-next-line
curl -fsSL https://raw.githubusercontent.com/exograph/exograph/main/installer/install.sh | sh
```

  </TabItem>
  <TabItem value="windows" label="Windows">

```shell-session
# shell-command-next-line
irm https://raw.githubusercontent.com/exograph/exograph/main/installer/install.ps1 | iex
```

  </TabItem>
</Tabs>

### Install the VS Code extension

Click [here](vscode:extension/exograph.exograph) to install the Exograph VS Code extension.

import ThemedImage from '@theme/ThemedImage';
import useBaseUrl from '@docusaurus/useBaseUrl';

## Creating a simple application

Let's create a simple application to get a taste of working with Exograph. Later, we will develop [a more fully featured application](/application-tutorial/overview.md) with access control, performance monitoring, and more.

### Creating the model

We will follow the well-worn tradition and create an API server for a Todo app.

Execute the following command:

```shell-session
# shell-command-next-line
exo new todo-app
```

This command will create a new directory named `todo-app` with the following structure:

```
todo-app
├── src
│   └── index.exo
├── tests
│   ├── basic-query.exotest
│   └── init.gql
├── .gitignore
```

Now change into the `todo-app` directory and check the contents of the `index.exo` file:

```shell-session
# shell-command-next-line
cd todo-app
# shell-command-next-line
cat src/index.exo
```

You will see the following (to see syntax highlighting, open the file in VS Code after installing the [Exograph VS Code Extension](https://marketplace.visualstudio.com/items?itemName=exograph.exograph)):

```exo title="src/index.exo"
@postgres
module TodoDatabase {
  @access(true)
  type Todo {
    @pk id: Int = autoIncrement()
    title: String
    completed: Boolean
  }
}
```

### Launching the server

Now we can launch the server.

```shell-session
# shell-command-next-line
exo yolo
```

The `yolo` mode is a simple way to start the server _during development_ without requiring a database. Exograph will create a temporary database in this mode and apply migrations whenever the model changes.

You should see the output in the console that the server is running.

```
Launching PostgreSQL locally...
Watching the src directory for changes...
Starting with a temporary database (will be wiped out when the server exits)...
Postgres URL: postgres://exo@%2Fvar%2Ffolders%2F8g%2Fttrcklpj7879w6fbk26dgrbh0000gn%2FT%2F.tmpcYt5yp/yolo
Generated JWT secret: c1d22ndtjjxlxni
Applying migrations...
Started server on localhost:9876 in 6.14 ms
- Playground hosted at:
        http://localhost:9876/playground
- Endpoint hosted at:
        http://localhost:9876/graphql
```

### Using the GraphiQL interface

Visit the playground at [http://localhost:9876/playground](http://localhost:9876/playground).

<ThemedImage
className="screenshot"
alt="Exograph GraphiQL interface"
sources={{
    light: useBaseUrl('/exograph-graphiql-light.png'),
    dark: useBaseUrl('/exograph-graphiql-dark.png'),
  }}
/>

#### Performing GraphQL mutations

Let's create a couple of todo items.

Paste the following code in the GraphiQL interface to create a todo titled "Install". Since that task is complete, we will set the `completed` property to `true`.

```graphql
mutation {
  createTodo(data: { title: "Install", completed: true }) {
    id
  }
}
```

And hit the "Execute Query" button. You should see the following output:

```json
{
  "data": {
    "createTodo": {
      "id": 1
    }
  }
}
```

Let's create another todo titled "Create Simple App". Since we are working on that task, we will set the `completed` property to `false`. You can query back any fields you want, so let's query the `title` and `completed` fields in addition to `id`.

```graphql
mutation {
  createTodo(data: { title: "Create Simple App", completed: false }) {
    id
    title
    completed
  }
}
```

You should see the following output:

```json
{
  "data": {
    "createTodo": {
      "id": 2,
      "title": "Create Simple App",
      "completed": false
    }
  }
}
```

#### Performing GraphQL queries

Now we can query the todos.

First, let's get all of the todos. Execute the following query in the GraphiQL interface:

```graphql
query {
  todos {
    id
    title
    completed
  }
}
```

You should see the following output:

```json
{
  "data": {
    "todos": [
      {
        "id": 1,
        "title": "Install",
        "completed": true
      },
      {
        "id": 2,
        "title": "Create Simple App",
        "completed": false
      }
    ]
  }
}
```

You can do much more: getting a todo by id or some filtering criteria, updating and deleting todos, etc. We will leave all those details for later.
