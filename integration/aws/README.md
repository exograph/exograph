This is an example app to show how to deploy Claytip as an AWS lambda. This folder contains a CloudFormation template, as well as a build script to build the lambdas needed by the template. The following steps show how to deploy `example.clay` to `us-west-1`.

1. Provision a VPC using the VPC wizard in the [VPC Dashboard](https://us-west-1.console.aws.amazon.com/vpc/home). Use the following settings:
    - VPC, subnets, etc.
    - Availability Zones (AZs): 2
    - Number of private subnets: 2
    - VPC endpoints: S3 Gateway
    - Enable DNS hostnames
    - Enable DNS resolution

2. Grab the subnet IDs of the two private subnets in the created VPC and the security group you'd like to use (the default security group of the created VPC will suffice). They will look like the following:
    - subnet-xxxxxxxxxxxxxxxxx
    - subnet-xxxxxxxxxxxxxxxxx
    - sg-xxxxxxxxxxxxxxxxxx

    These two subnets have automatically been created by the VPC wizard in two different AZs; this is necessary, as RDS mandates subnets in multiple AZs even if a DBInstance is a single-AZ deployment.

3. In this directory, build the necessary functions:
    ```sh
    ./build.sh -c example.clay
    ```

    This commands will build the folders `aws-app` (containing the actual lambda that will serve your Claytip API), and `aws-cf-func` (a lambda that will run once at deployment to initialize the database schema) in this directory.

4. Make sure AWS credentials are configured on your local system:
    ```sh
    aws configure
    ```

5. Deploy the CloudFormation template using `aws-sam-cli`. For first-time deployment, use the SAM guided deploy in this directory:
    ```sh
    sam deploy --guided
    ```

    Fill in the desired database username, password, and dbname. Fill in the subnet IDs and security groups obtained earlier. Say `Y` to the authorization prompt if it appears:

    ```
        Setting default arguments for 'sam deploy'
        =========================================
        Stack Name [sam-app]: clay-lambda
        AWS Region [us-west-1]: 
        Parameter ClaytipDatabaseUsername [clay]: 
        Parameter ClaytipDatabasePassword [claytipdbpassword]: 
        Parameter ClaytipDatabaseName [claytipdb]: 
        Parameter ClaytipSubnetAZ1 []: subnet-xxxxxxxxxxxxxxxxx
        Parameter ClaytipSubnetAZ2 []: subnet-xxxxxxxxxxxxxxxxx
        Parameter ClaytipSecurityGroup []: sg-xxxxxxxxxxxxxxxxxx

        ...

        ClaytipFunction may not have authorization defined, Is this okay? [y/N]: y
    ```

    Wait for the stack to deploy. Once finished, SAM will print the endpoint URL (you can also get it by looking at ClaytipFunction's triggers in the [Lambda dashboard](https://us-west-1.console.aws.amazon.com/lambda/home)):
    ```
    CloudFormation outputs from deployed stack
    ------------------------------------------
    Outputs
    ------------------------------------------
    ...

    Key                 ClaytipApi                                                                                               
    Description         API Gateway endpoint URL for Prod stage for Claytip function                                             
    Value               https://xxxxxxxxxx.execute-api.us-west-1.amazonaws.com/Prod/ 
    ...
    ```

    For future deployments, just `sam deploy` is sufficient.

6. To clean up your deployment, simply run the following with your stack name:
    ```sh
    aws cloudformation delete-stack --stack-name <your stack name>
    ```
    ... or delete your stack manually through the CloudFormation dashboard.
