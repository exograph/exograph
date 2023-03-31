#!/usr/bin/env bash

set -e

function usage {
    echo "usage: ./deploy.sh --appname <Azure function app name> -c model.exo"
    exit 1
}

SCRIPT_DIRECTORY=`dirname $BASH_SOURCE`

while [[ "$#" -gt 0 ]]; do
    case $1 in
        -c) exo_file="$2"; shift 2;;
        --appname) appname="$2"; shift 2;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
done

[ "$appname" = "" ] && usage
[ "$exo_file" = "" ] && usage

rm -rf $SCRIPT_DIRECTORY/azure-app || true
mkdir -p $SCRIPT_DIRECTORY/azure-app

SCRIPT_FILE=$SCRIPT_DIRECTORY/azure-app/handler
echo -en "#!/bin/sh\n\n" > $SCRIPT_FILE
echo -en "export EXO_SERVER_PORT=\${FUNCTIONS_CUSTOMHANDLER_PORT}\n\n" >> $SCRIPT_FILE
echo "./exo-server" >> $SCRIPT_FILE
chmod +x $SCRIPT_FILE

docker build -t exo-azure-exo_ir -f $SCRIPT_DIRECTORY/Dockerfile --build-arg EXO_FILE="$exo_file" $SCRIPT_DIRECTORY
id=$(docker create exo-azure-exo_ir:latest)
docker cp $id:/usr/src/app/. $SCRIPT_DIRECTORY/azure-app/

cp -r $SCRIPT_DIRECTORY/ExographApi $SCRIPT_DIRECTORY/azure-app/
cp -r $SCRIPT_DIRECTORY/ExographPlaygroundStatic $SCRIPT_DIRECTORY/azure-app/
cp $SCRIPT_DIRECTORY/host.json $SCRIPT_DIRECTORY/azure-app/

# deploy
(cd $SCRIPT_DIRECTORY/azure-app; func azure functionapp publish $appname --custom)

# clean up
rm -rf $SCRIPT_DIRECTORY/azure-app
