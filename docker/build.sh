#!/usr/bin/env bash

set -x

buildKind="$1" # "release" or "debug"

BUILD_IMAGE=rust:1.60.0-buster
BASE_IMAGE=rust:1.60.0-slim-buster
DEPENDENCY_STYLE=deb
TAG_SUFFIX=""

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
    TAG_SUFFIX="-debug"
elif [ "$buildKind" == "aws" ]
then
    echo "Building with Amazon Linux 2"
    BUILD_FLAG=
    BUILD_DIR=debug
    BUILD_IMAGE=amazonlinux:2
    BASE_IMAGE=amazonlinux:2
    DEPENDENCY_STYLE=aws
    TAG_SUFFIX="-aws"
else
    echo "Unknown build kind: '$buildKind'. Must be 'release' or 'debug'."
    exit 1
fi

docker_build() {
    TAG=$1 
    TARGET=$2 

    DOCKERFILE_TEMPLATE="$(dirname $BASH_SOURCE)/Dockerfile.template"

    # fragments to substitute into final dockerfile
    BUILD_SETUP="$(dirname $BASH_SOURCE)/Dockerfile.$DEPENDENCY_STYLE.build"
    RUNTIME_SETUP="$(dirname $BASH_SOURCE)/Dockerfile.$DEPENDENCY_STYLE"

    # where to create the generated dockerfile
    GENERATED_DOCKERFILE="$(dirname $BASH_SOURCE)/Dockerfile.generated"

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
            .
}

docker_build "clay-builder$TAG_SUFFIX" "clay-builder"
docker_build "clay$TAG_SUFFIX" 
