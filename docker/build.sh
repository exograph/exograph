#!/usr/bin/env bash

set -e

SCRIPT_DIRECTORY="$(dirname $BASH_SOURCE)"
ROOT_DIRECTORY=$SCRIPT_DIRECTORY/..

buildKind="$1" # "debian" or "aws"
buildType="$2" # "release" or "debug"

# TODO: Resolve the openssl issues and then "BASE_IMAGE=debian:buster-slim"

## DEFAULTS ##
BUILD_IMAGE=rust:1.65.0-buster # image to build Exograph with
BASE_IMAGE=rust:1.65.0-slim-buster # image to use when actually running Exograph
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

# Tuples of (name kind path)
declare -a SUBCRATES=(
    "cli bin crates"
    "server-actix bin crates"
    "server-aws-lambda bin crates"
    "builder lib crates"
    "resolver lib crates"
    "testing lib crates"
    "exo-sql lib libs"
    "exo-deno lib libs"
    "exo-wasm lib libs"
    "core-model lib crates\/core-subsystem"
    "core-model-builder lib crates\/core-subsystem"
    "core-plugin-shared lib crates\/core-subsystem"
    "core-plugin-interface lib crates\/core-subsystem"
    "core-resolver lib crates\/core-subsystem"
    "postgres-model lib crates\/postgres-subsystem"
    "postgres-model-builder lib crates\/postgres-subsystem"
    "postgres-resolver lib crates\/postgres-subsystem"
    "deno-model lib crates\/deno-subsystem"
    "deno-model-builder lib crates\/deno-subsystem"
    "deno-resolver lib crates\/deno-subsystem"
    "wasm-model lib crates\/wasm-subsystem"
    "wasm-model-builder lib crates\/wasm-subsystem"
    "wasm-resolver lib crates\/wasm-subsystem"
    "introspection-resolver lib crates\/introspection-subsystem"
    "subsystem-model-builder-util lib crates\/subsystem-util"
    "subsystem-model-util lib crates\/subsystem-util"
)

compute_create_empty_projects() {
    local RESULT

    for crate_info in "${SUBCRATES[@]}"
    do
        set -- $crate_info
        name="$1"
        kind="$2"
        path="$3"
        RESULT="${RESULT}RUN USER=root cargo new --vcs none --$kind $path\/$name\n"
    done

    echo $RESULT
}

compute_copy_cargo_tomls() {
    local RESULT

    for crate_info in "${SUBCRATES[@]}"
    do
        set -- $crate_info
        name="$1"
        path="$3"
        RESULT="${RESULT}COPY .\/$path\/$name\/Cargo.toml .\/$path\/$name\/Cargo.toml\n"
    done

    echo $RESULT
}

compute_rm_deps() {
    local RESULT

    artifacts_dir="target\/\${BUILD_DIR}"
    deps_dir="$artifacts_dir\/deps"
    for crate_info in "${SUBCRATES[@]}"
    do
        set -- $crate_info
        name="$1"
        path="$3"
        module_name=$(echo $name | sed 's/-/_/g')

        # Remove sources that were created by the empty projects
        RESULT="${RESULT}RUN rm $path\/$name\/src\/\*.rs "
        # Remove the artifacts created by the empty projects
        ## First, all the lib* files for our modules
        RESULT="${RESULT}\&\& rm -f ${artifact_dir}\/lib${module_name}.\* "
        ## Then, all the .d files for our modules (lib<module_name>.rlib, lib<module_name>.rmeta, <module_name>.d)
        RESULT="${RESULT}\&\& rm -f ${deps_dir}\/${module_name}* \&\& rm -f ${deps_dir}\/lib${module_name}* "
        ## Also remove the fingerprint files for our modules (note the names here use the crate name, not the module name)
        RESULT="${RESULT}\&\& rm -rf ${artifact_dir}\/.fingerprint\/${name}*\n"
    done

    echo $RESULT
}


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

    CREATE_EMPTY_PROJECTS=$(compute_create_empty_projects)
    COPY_CARGO_TOMLS=$(compute_copy_cargo_tomls)
    RM_DEPS=$(compute_rm_deps)

    sed -i "s/%%CREATE_EMPTY_PROJECTS%%/$CREATE_EMPTY_PROJECTS/" $GENERATED_DOCKERFILE
    sed -i "s/%%COPY_CARGO_TOMLS%%/$COPY_CARGO_TOMLS/" $GENERATED_DOCKERFILE
    sed -i "s/%%RM_DEPS%%/$RM_DEPS/g" $GENERATED_DOCKERFILE

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

docker_build "exo-builder$TAG_SUFFIX" "exo-builder"
docker_build "exo$TAG_SUFFIX" 
