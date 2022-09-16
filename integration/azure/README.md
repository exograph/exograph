This is an example workflow to show how to deploy Claytip as an Azure function.

0. `cd` into this directory on a shell.
1. Run `./create-azure-app.sh` to create a function app in Azure and follow the
   interactive instructions.
2. If needed, create a database and set `CLAY_DATABASE_URL` as an application
   setting in the Azure dashboard for your created function app.
3. Initialize the schema in your database:
   ```
   $ clay schema create example.clay | psql ...
   ```
4. Deploy your app using `./deploy.sh`:
   ```
   $ ./deploy.sh --appname <your function app's name> -c example.clay
   ```
5. Visit the `ClaytipApi` invoke url printed in the console to access the playground:
   ```
   ...
   Deployment completed successfully.
   Syncing triggers...
   Functions in claytiptest:
       ClaytipApi - [httpTrigger]
           Invoke url: https://<your function app's name>.azurewebsites.net/api/claytipapi
   ...
   ```