#!/usr/bin/env bash

set -e

RED='\033[0;31m'
NC='\033[0m' # No Color

# check if azure-cli is logged in
az account show > /dev/null

# check that jq is installed
if ! command -v jq &> /dev/null; then
    echo "Please install \`jq\`."
    exit 1
fi

function queryUser () {
    QUERY=$1
    DEFAULT_RESPONSE=$2

    if [ "$DEFAULT_RESPONSE" = "" ]; then 
        printf "$QUERY: " >&2
        read

        if [ "$REPLY" = "" ]; then
            printf "Did not get a response, exiting..." >&2
            exit 1 
        fi

        echo "$REPLY"
    else
        printf "$QUERY: [$DEFAULT_RESPONSE] " >&2
        read

        if [ "$REPLY" = "" ]; then
            echo "$DEFAULT_RESPONSE"
        else
            echo "$REPLY"
        fi
    fi
}

appname=$(queryUser "Enter the name of the new function app")
location=$(queryUser "Enter location to create app in" "westus")
resourceGroup=$(queryUser "Enter new resource group name" "$appname-claytip-rg")
storageAccount=$(queryUser "Enter new storage account name" "$appname")

functionsVersion="4"
skuStorage="Standard_LRS" # https://docs.microsoft.com/en-us/rest/api/storagerp/srp_sku_types

set -x

az group create \
    --name "$resourceGroup" \
    --location "$location" \
    --tags "claytip"

set -o errtrace
trap "set +x; echo -e \"\${RED}Error encountered, deleting resource group...\${NC}\"; set -x; az group delete --name $resourceGroup" ERR

az storage account create \
    --name "$storageAccount" \
    --location "$location" \
    --resource-group "$resourceGroup" \
    --sku "$skuStorage"

az functionapp create \
    --name "$appname" \
    --storage-account $storageAccount \
    --consumption-plan-location "$location" \
    --resource-group "$resourceGroup" \
    --functions-version $functionsVersion \
    --os-type Linux \
    --runtime custom

az functionapp config appsettings set \
    --resource-group "$resourceGroup" \
    --name "$appname" \
    --settings \
    CLAY_INTROSPECTION=true \
    CLAY_JWT_SECRET=abcd \
    CLAY_CORS_DOMAINS=* \
    CLAY_ENDPOINT_HTTP_PATH=/api/claytipapi \
    CLAY_PLAYGROUND_HTTP_PATH=/api/claytipapi \
    FUNCTIONS_WORKER_RUNTIME=custom

set +x
echo ""
echo "Creation of Azure function app \`$appname\` successful."
echo "- Delete all resources after finishing to avoid incurring additional charges:"
echo "  \$ az group delete --name $resourceGroup"

echo "- Please create and initialize a database, then set CLAY_DATABASE_URL in this app's Application Settings."
if [ $(queryUser "Do this automatically?" "y") != "y" ]; then
    echo "Exiting..."
    exit 0
fi

postgresServer=$(queryUser "Enter new PostgreSQL server name" "$appname-psql")
postgresUsername=$(queryUser "Enter new database username" "claytip")
postgresPassword=$(queryUser "Enter new database password" "ClayDev1234")
skuPostgres="GP_Gen5_2" # https://docs.microsoft.com/en-us/azure/postgresql/single-server/concepts-pricing-tiers

set -x

az postgres server create \
    --name "$postgresServer" \
    --resource-group "$resourceGroup" \
    --location "$location" \
    --admin-user "$postgresUsername" \
    --admin-password "$postgresPassword" \
    --sku "$skuPostgres"

postgresFQDN=$(az postgres server show --resource-group "$resourceGroup" --name "$postgresServer" | jq -r ."fullyQualifiedDomainName")
postgresConnectionString="postgresql://$postgresFQDN:5432/postgres?user=$postgresUsername@$postgresServer&password=$postgresPassword&sslmode=require"

az functionapp config appsettings set \
    --resource-group "$resourceGroup" \
    --name "$appname" \
    --settings \
    CLAY_DATABASE_URL=$postgresConnectionString 

az postgres server firewall-rule create \
    --resource-group $resourceGroup \
    --server "$postgresServer" \
    --name AllowClaytip \
    --start-ip-address "0.0.0.0" \
    --end-ip-address "0.0.0.0"

currentOutgoingIp=$(curl "https://api.ipify.org/")
az postgres server firewall-rule create \
    --resource-group $resourceGroup \
    --server "$postgresServer" \
    --name AllowDevIp \
    --start-ip-address $currentOutgoingIp \
    --end-ip-address $currentOutgoingIp

set +x

echo ""
echo "- A new Azure database instance was successfully set up, along with \`$appname\`. Please initialize it with your schema:"
echo "  \$ clay schema create model.clay | psql \"$postgresConnectionString\""
echo "- A new firewall rule was created for the instance, allowing \`$currentOutgoingIp\` to connect (your current outgoing IP)"