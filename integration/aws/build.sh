#!/usr/bin/env bash
# Example usage:
# ./build.sh -c example.clay

set -e
set -x

while [[ "$#" -gt 0 ]]; do
    case $1 in
        -c) clay_file="$2"; shift 2;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
done

SCRIPT_DIRECTORY=`dirname $BASH_SOURCE`

rm -rf $SCRIPT_DIRECTORY/aws-app
rm -rf $SCRIPT_DIRECTORY/aws-cf-func
mkdir -p $SCRIPT_DIRECTORY/aws-app
mkdir -p $SCRIPT_DIRECTORY/aws-cf-func

docker build -t clay-aws-claypot -f $SCRIPT_DIRECTORY/Dockerfile --build-arg CLAY_FILE="$clay_file" $SCRIPT_DIRECTORY
id=$(docker create clay-aws-claypot:latest)

docker cp $id:/usr/src/app/bootstrap $SCRIPT_DIRECTORY/aws-app/
docker cp $id:/usr/src/app/index.claypot $SCRIPT_DIRECTORY/aws-app/
docker cp $id:/usr/src/app/index.sql $SCRIPT_DIRECTORY/aws-cf-func/
docker cp $id:/usr/src/app/python-deps/. $SCRIPT_DIRECTORY/aws-cf-func/
docker rm -v $id

cp $SCRIPT_DIRECTORY/lambda_function.py aws-cf-func/
