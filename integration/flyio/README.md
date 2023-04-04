This is an example app to show how to use fly.io.

# Building

From this directory, run:

```sh
./build.sh -c example.exo -t example-fly -e example.env
```

# Running

1. Create an app

```sh
flyctl apps create --name exo-concerts
```

2. Create a database

```sh
flyctl postgres create --name exo-concerts-db
```

3. Attach the database to the app

```sh
flyctl postgres attach --app exo-concerts --postgres-app exo-concerts-db
```

Note down the POSTGRES_URL shown. Later, you can connect to it using `psql` (after creating a WireGuard tunnel).

4. Deploy the app

```sh
flyctl deploy --local-only -a exo-concerts -i example-fly:latest
```

5. Set up the database

```sh
flyctl postgres connect -a exo-concerts-db -d exo_concert -u exo_concert -p <password> < exo schema create example.exo
```

Here the password is the one printed by `flyctl postgres attach`.
