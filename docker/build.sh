#!/usr/bin/env bash

buildKind="$1" # "release" or "debug"

if [ "$buildKind" == "release" ]
then
    echo "Building release"
    BUILD_FLAG=--release
    BUILD_DIR=release
elif [ "$buildKind" == "debug" ]
then
    echo "Building debug"
    BUILD_FLAG=
    BUILD_DIR=debug
else
    echo "Unknown build kind: '$buildKind'. Must be 'release' or 'debug'."
    exit 1
fi

docker build -t clay -f docker/Dockerfile --build-arg BUILD_DIR="$BUILD_DIR" --build-arg BUILD_FLAG="$BUILD_FLAG" .
