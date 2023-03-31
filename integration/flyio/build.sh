#!/usr/bin/env bash

# Example usage:
# ./build.sh -c example.exo -t example-fly -e example.env

while [[ "$#" -gt 0 ]]; do
    case $1 in
        -c) exo_file="$2"; shift 2;;
        -t) tag="$2"; shift 2;;
        -e) envfile="$2"; shift 2;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
done

if [ -z "$tag" ]; then
    echo "No tag specified. Exiting."
    exit 1
fi

SCRIPT_FILE=run-exo-fly.sh

echo -en "#!/bin/sh\n\n" > $SCRIPT_FILE
echo -en "export EXO_POSTGRES_URL=\${POSTGRES_URL}\n\n" >> $SCRIPT_FILE
if [ -n "$envfile" ]; then
    cat "$envfile" >> $SCRIPT_FILE
fi
echo -en "\n\n" >> $SCRIPT_FILE
echo "./exo-server ./${exo_file}_ir" >> $SCRIPT_FILE

chmod +x $SCRIPT_FILE

docker build -t $tag -f Dockerfile --build-arg EXO_FILE="$exo_file" .
