This is an example workflow to show how to deploy Exograph as an Azure function.

# Prerequisites

- Docker
- azure-cli
- azure-functions-core-tools

# Getting started

0. `cd` into this directory on a shell. Make sure you are logged into Azure:
   `az login`.
1. Run `./create-azure-app.sh` to create a function app in Azure and follow the
   interactive instructions.
2. If needed, create a database and set `EXO_POSTGRES_URL` as an application
   setting in the Azure dashboard for your created function app.
3. Initialize the schema in your database:
   ```
   $ exo schema create example.exo | psql ...
   ```
4. Deploy your app using `./deploy.sh`:
   ```
   $ ./deploy.sh --appname <your function app's name> -c example.exo
   ```
5. Visit the `ExographApi` invoke url printed in the console to access the
   playground:
   ```
   ...
   Deployment completed successfully.
   Syncing triggers...
   Functions in exographtest:
       ExographApi - [httpTrigger]
           Invoke url: https://<your function app's name>.azurewebsites.net/api/exographapi
   ...
   ```
