#!/usr/bin/env bash

set -e

SCRIPT_DIRECTORY="$(dirname $BASH_SOURCE)"
ROOT_DIRECTORY=$SCRIPT_DIRECTORY/..

buildKind="$1" # "debian" or "aws"
buildType="$2" # "release" or "debug"

# TODO: Resolve the openssl issues and then "BASE_IMAGE=debian:buster-slim"

## DEFAULTS ##
BUILD_IMAGE=rust:1.63.0-buster # image to build Claytip with
BASE_IMAGE=rust:1.63.0-slim-buster # image to use when actually running Claytip
DEPENDENCY_STYLE=deb # how to install or setup dependencies
TAG_SUFFIX="" # docker tag suffix

## set build flags/build dirs from buildType
if [ "$buildType" == "release" ]
then
    echo "Building release"
    BUILD_FLAG=--release
    BUILD_DIR=release
elif [ "$buildType" == "debug" ]
then
    echo "Building debug"
    BUILD_FLAG=
    BUILD_DIR=debug
    TAG_SUFFIX="-debug"
else
    echo "Unknown build type: '$buildType'. Must be 'release' or 'debug'.".
    exit 1
fi

## set options from buildKind
if [ "$buildKind" == "debian" ]
then
    echo "Building regularly with Debian"
elif [ "$buildKind" == "aws" ]
then
    echo "Building with Amazon Linux 2"
    BUILD_IMAGE=amazonlinux:2
    BASE_IMAGE=amazonlinux:2
    DEPENDENCY_STYLE=aws
    TAG_SUFFIX="$TAG_SUFFIX-aws"
else
    echo "Unknown build kind: '$buildKind'. Must be 'debian' or 'aws'."
    exit 1
fi

# Generates Dockerfile.generated and builds a docker image from it to the specified target.
# Final image is tagged with the specified tag.
docker_build() {
    TAG=$1 
    TARGET=$2 

    DOCKERFILE_TEMPLATE=$SCRIPT_DIRECTORY/Dockerfile.template

    # fragments to substitute into final dockerfile
    BUILD_SETUP=$SCRIPT_DIRECTORY/Dockerfile.$DEPENDENCY_STYLE.build
    RUNTIME_SETUP=$SCRIPT_DIRECTORY/Dockerfile.$DEPENDENCY_STYLE

    # where to create the generated dockerfile
    GENERATED_DOCKERFILE=$SCRIPT_DIRECTORY/Dockerfile.generated

    optional_target=()
    [[ ! -z "$TARGET" ]] && optional_target+=(--target "$TARGET")

    # generate a dockerfile based on selected dependency style
    cat $DOCKERFILE_TEMPLATE \
        | sed -e '/%%BUILD_SETUP%%/ {' -e "r $BUILD_SETUP" -e 'd' -e '}' \
        | sed -e '/%%RUNTIME_SETUP%%/ {' -e "r $RUNTIME_SETUP" -e 'd' -e '}' \
        > $GENERATED_DOCKERFILE

    docker build \
            -t $TAG \
            -f $GENERATED_DOCKERFILE \
            --build-arg BUILD_DIR="$BUILD_DIR" \
            --build-arg BUILD_FLAG="$BUILD_FLAG" \
            --build-arg BUILD_IMAGE="$BUILD_IMAGE" \
            --build-arg BASE_IMAGE="$BASE_IMAGE" \
            "${optional_target[@]}" \
            $ROOT_DIRECTORY 
}

docker_build "clay-builder$TAG_SUFFIX" "clay-builder"
docker_build "clay$TAG_SUFFIX" 
