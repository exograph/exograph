#!/usr/bin/env bash

# Example usage:
# ./build.sh -c example.clay 

SCRIPT_DIRECTORY=`dirname $BASH_SOURCE`

while [[ "$#" -gt 0 ]]; do
    case $1 in
        -c) clay_file="$2"; shift 2;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
done

rm -rf $SCRIPT_DIRECTORY/azure-app
mkdir -p $SCRIPT_DIRECTORY/azure-app

SCRIPT_FILE=$SCRIPT_DIRECTORY/azure-app/handler

echo -en "#!/bin/sh\n\n" > $SCRIPT_FILE
echo -en "export CLAY_SERVER_PORT=\${FUNCTIONS_CUSTOMHANDLER_PORT}\n\n" >> $SCRIPT_FILE
#if [ -n "$envfile" ]; then
#    cat "$envfile" >> $SCRIPT_FILE
#fi
#echo -en "\n\n" >> $SCRIPT_FILE
echo "./clay-server" >> $SCRIPT_FILE

chmod +x $SCRIPT_FILE

docker build -t clay-azure-claypot -f $SCRIPT_DIRECTORY/Dockerfile --build-arg CLAY_FILE="$clay_file" $SCRIPT_DIRECTORY
id=$(docker create clay-azure-claypot:latest)
docker cp $id:/usr/src/app/clay-server $SCRIPT_DIRECTORY/azure-app/
docker cp $id:/usr/src/app/index.claypot $SCRIPT_DIRECTORY/azure-app/

cp -r $SCRIPT_DIRECTORY/ClaytipApi $SCRIPT_DIRECTORY/azure-app/
cp -r $SCRIPT_DIRECTORY/*.json $SCRIPT_DIRECTORY/azure-app/