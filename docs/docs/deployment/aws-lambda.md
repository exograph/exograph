---
sidebar_position: 30
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

# AWS Lambda

We will deploy our application as an AWS Lambda Function! The bulk of this tutorial is about setting up AWS. The Exograph-specific part is quite simple.

## Creating a new application

import CreatingApp from './\_creating_app.md';

<CreatingApp/>

## Setting up AWS

You will need to have an AWS account and have the AWS CLI installed and configured. Please follow the [AWS CLI installation guide](https://docs.aws.amazon.com/cli/latest/userguide/install-cliv2.html) to install the AWS CLI.

:::tip
The rest of the section describes the setup needed to deploy Exograph as a function (and is one of many possible ways to do so). So feel free to deviate from the instructions if you have prior experience with AWS Lambda.
:::

### Creating a role

You will need a role with sufficient permissions to manage a lambda function.

1. Visit the [IAM Console](https://console.aws.amazon.com/iamv2/home#/roles/create?step=selectEntities).
2. Choose the default "AWS Service", and in the "Use case" section, choose "Lambda" and then click "Next".
3. In the "Permissions" section, choose "Attach existing policies directly" and then search for `AWSLambda_FullAccess` and select it. Similarly, search for `AWSLambdaBasicExecutionRole` and select it.
4. Click "Next" and after giving it a name (e.g. `exograph-lambda-role`), click "Create role".

### Creating an IAM user

You will also need to create an `iam` user (you could use the root user, but it is not a good idea).

1. Visit the [IAM Console](https://console.aws.amazon.com/iamv2/home#/users) and click on "Add users".
2. Enter a new user name (e.g. `exograph-lambda-user`).
3. Check "Provide user access to the AWS Management Console". Choose a way to manage the password (for example, choose "I want to create an IAM user", "Custom password", keep "Users must create a new password at next sign-in" unchecked, and supply a password).
4. Choose "Attach policies directly", and then search for `AWSLambda_FullAccess` and select it.
5. Click Next to go to the "Review and create" page.
6. Click "Create user".

Then you will see the "Users" page. Click on the user you just created and then on the "Security credentials" tab. Click on "Create access key", choose "Command Line Interface", and you will get a key and a secret. You will need these to configure the AWS CLI.

### Configuring the AWS CLI

Let's use the credentials we just created to configure the AWS CLI by running the following command:

```shell-session
# shell-command-next-line
aws configure
AWS Access Key ID [None]: <your-access-key-id>
AWS Secret Access Key [None]: <your-secret-access-key>
```

### Creating a PostgreSQL database

We will show two ways to create a PostgreSQL database. The first is to use AWS RDS, and the second is to use an external database.

:::warning
You will get the best performance (including low cold start times) if you create your database in the same region as your lambda function. Keep that in mind as you set up your database.
:::

<Tabs groupId="database-choice">
  <TabItem value="rds" label="RDS Postgres" default>

You can skip this step if you already have a database you want to use.

We will create a simple setup, but if you want to deploy to production, you must take steps to secure your database. Of course, you may also follow other ways to create an RDS PostgreSQL database. As long as there is a way to create a schema in it and make it accessible to our lambda function, it will suffice.

#### Creating a VPC

First, we will need a VPC (you may use an existing one if you prefer). From the [VPC console page](https://console.aws.amazon.com/vpc/home):

1. Click on "Create VPC" and choose "VPC and more":

   - Give it a name tag (e.g. "exo-lambda-vpc")
   - Specify a CIDR block (e.g. "10.0.0.0/16").
   - Ensure that number of availability zone is 2. Click on "Create VPC".
   - Choose 0 private subnets and 2 public subnets.
   - Ensure that both DNS hostnames and DNS resolution are enabled.

2. Click on "Security Groups" so that we can access the database from the internet:

   - Pick the one associated with the VPC you just created.
   - Click on "Edit inbound rules", then on "Add rule", choose "PostgreSQL" and in "Source", choose "0.0.0.0/0".
   - Click on "Save rules".

#### Creating a database instance

Now let's create a database. From the [RDS console page](https://console.aws.amazon.com/rds/home):

1. Click on "Create database" and choose:

   - "Standard Create" method
   - "Aurora (PostgreSQL Compatible)" engine
   - "Dev/Test" template

2. Give it a name in the "DB cluster identifier" (e.g. "exo-lambda-db-cluster"), a master username, and a master password.

3. Choose "Memory optimized" instance type (or "Serverless v2").

4. Choose "Public access".

5. Click on "Create database".

Now we have the database server ready!

  </TabItem>
  <TabItem value="external" label="External Postgres">

Follow the steps in the [cloud deployment](flyio.md) chapter to create a Neon database. Choose the "External Database" tab in the documentation and follow the steps to create a database.

  </TabItem>
</Tabs>

## Create a database instance

<Tabs groupId="database-choice">
  <TabItem value="rds" label="RDS Postgres" default>

We will create a new database instance.

```shell-session
# shell-command-next-line
createdb --user postgres --host <endpoint> todo-db
```

  </TabItem>
  <TabItem value="external" label="External Postgres">

Since we have already created a Neon database, there isn't anything to do here.

  </TabItem>
</Tabs>

## Creating a schema

Now that the database is ready, we will create the schema using `exo schema migrate`.

```shell-session
# shell-command-next-line
exo schema migrate --apply-to-database --database postgres://postgres@<endpoint>/todo-db
```

## The `exo deploy aws-lambda` command

Exograph has a dedicated command to work with AWS Lambda. It will:

- Create a package with your application
- Provide deployment instructions

It offers a few options to customize the deployment, and you can learn more by running `exo deploy aws-lambda --help`.

```shell-session
# shell-command-next-line
exo deploy aws-lambda --help
Deploy to AWS Lambda

Usage: exo deploy aws-lambda --app <app-name>

Options:
  -a, --app <app-name>  The name of the application
  -h, --help            Print help
```

## Creating the function

Let's use it to deploy the app:

```shell-session
# shell-command-next-line
exo deploy aws-lambda --app todo
```

This command will create a zip file with the application and print out the instructions to deploy it to AWS Lambda. The instructions will look something like this:

```shell-session
Creating a new AWS Lambda function.

If haven't already done so, run `aws configure` to set up access to your AWS account.

To deploy the function for the first time, run:
exo schema migrate --apply-to-database --database <your-postgres-url>
aws lambda create-function --function-name todo --zip-file fileb://target/aws-lambda/function.zip --role arn:aws:iam::<account-id>:role/<role> --runtime=provided.al2 --handler=bootstrap --environment "Variables={EXO_POSTGRES_URL=<your-postgres-url>}"

To deploy a new version of an existing app, run:
aws lambda update-function-code --function-name todo --zip-file fileb://target/aws-lambda/function.zip
```

Follow the instructions (replacing the highlighted values), and you should be all set!

:::note
Currently, Exograph doesn't support using the AWS secrets manager to store the database credentials. We will add support for it in the future.
:::

## Updating the function

To update the application, run the `exo deploy aws-lambda` command again. It will create a new zip file with the application. After that, you can follow the instructions to update the function.

## Testing the function

Now that we have deployed the function, it is time to test it. We will use the AWS Lambda console as well as Postman to test the function.

### Through the console

Visit the [AWS Lambda Console](https://console.aws.amazon.com/lambda/home) to see your function. You will need to provide a test payload. The `body` part of the payload needs to match the GraphQL spec and other attributes need to match the AWS Lambda event payload specification.

Exograph responds to both `GET` or `POST` methods, so you can use either. After providing the payload, click the "Test" button. Here is an example of a payload for a `GET` request (using the default `httpMethod`):

```json
{
  "body": "{\"query\":\"{\\n\\ttodos {\\n\\t  id\\n\\t}\\n}\"}"
}
```

The same payload for a `POST` request looks like this:

```json
{
  "httpMethod": "POST",
  "body": "{\"query\":\"{\\n\\ttodos {\\n\\t  id\\n\\t}\\n}\"}"
}
```

You should get back a result. Nice, but specifying the payload this way is cumbersome, so let's explore a different way to test our function.

### Through Postman

Visit the [AWS Lambda Console](https://console.aws.amazon.com/lambda/home). Click on the just created function, then switch to the "Configuration" tab and the "Function URL". Click on the "Create Function URL" button and choose "NONE" for "Auth type" (you will be exposing your URL publicly, which may be fine for this exploration, but later you can set appropriate restrictions). You will get a URL for your function.

Download Postman from [here](https://www.postman.com/downloads/) and open it. Create a new request and set the method to "POST" and the URL to the function URL you just created. Switch to the "Body" tab and select "GraphQL" from the dropdown. Enter the following query:

```graphql
query {
  todos {
    id
    title
    completed
  }
}
```

Click on the "Send" button and you should get back a result. After that, you can play by invoking any of the queries and mutations you have defined in your application.

### Through the playground

import TestingApp from './\_testing_app.md';

<TestingApp/>

:::warning
Make sure that you delete any resources you no longer need to avoid incurring charges.
:::
