This is an example app to show how to use fly.io.

# Building

From this directory, run:

```sh
./build.sh -c example.clay -t example-fly -e example.env
```

# Running

1. Create an app

```sh
flyctl create --name clay-concert
```

2. Create a database

```sh
flyctl postgres create --name concerts-db
```

3. Attach the database to the app

```sh
flyctl postgres attach --app clay-concert --postgres-app concerts-db
```

Note down the POSTGRES_URL shown. Later, you can connect to it using `psql` (after creating a WireGuard tunnel).
