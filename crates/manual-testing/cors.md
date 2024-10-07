# CORS testing

While we have unit tests, to be really sure (especially against a real browser behavior), we should do manual testing (until we automate this through Selenium or similar).

Switch to the directory where you cloned https://github.com/exograph/examples and enter the todo-with-nextjs directory.

## Testing playground

This ensures that Exograph doesn't enforce CORS for same-origin requests.

Do the following with `exo yolo` and `exo dev` (which sets `EXO_CORS_DOMAIN` to `*`) and `exo-server` (which doesn't set any `EXO_CORS_DOMAIN`).

- Start the API server

```
exo yolo
```

or

```
EXO_INTROSPECTION=true EXO_POSTGRES_URL=postgres://localhost/todo exo-server
```

- Load the playground. It should load successfully.
- Make a query such as `todos { id }`. It should return a list of todos (possibly empty).

## Testing the webapp

This ensure a more real-world scenario that involves a cross-origin request.

### With `exo yolo` or `exo dev` (which sets `EXO_CORS_DOMAIN` to `*`)

- Start the API server
- Run `npm run dev` from the `web` directory. This ensure that we don't enforce CORS for non-browser clients.
- Open the app in the browser. It should show the list of todos (possibly empty).

### With `exo-server`

#### Without `EXO_CORS_DOMAIN`

- Start the API server without setting `EXO_CORS_DOMAIN`.

```
EXO_INTROSPECTION=true EXO_POSTGRES_URL=postgres://localhost/todo exo-server
```

- Run `npm run dev` from the `web` directory. This should succeed (ensures that we don't enforce CORS for non-browser clients).
- Open the app in the browser. It should **fail to load**. If you were to inspect the network tab, you'd see a CORS error.

#### With `EXO_CORS_DOMAIN` incorrect domain

```
EXO_CORS_DOMAINS="http://localhost:3000" EXO_INTROSPECTION=true EXO_POSTGRES_URL=postgres://localhost/todo ~/exograph-org/exograph/target/debug/exo-server

Run the same steps as above and the result should be the same (success for non-browser clients and failure for the browser).

#### With `EXO_CORS_DOMAIN` correctly set

```
EXO_CORS_DOMAIN="http://localhost:3000" EXO_INTROSPECTION=true EXO_POSTGRES_URL=postgres://localhost/todo exo-server
```

Run the same steps as above and both the non-browser and browser clients should succeed.

#### With `EXO_CORS_DOMAIN` set to `*`

Run the same steps as above and both the non-browser and browser clients should succeed.





