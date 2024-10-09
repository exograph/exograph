# Building

Build docker images suitable for Fly and Azure ("debian") and for AWS Lambda ("amazonlinux2").

This helps to try building Exograph in a CI-like environment. This is especially useful for building the AWS Lambda version, since it is sensitive to the build system version.

From the root directory of the project, run:

- To build a release version

```sh
docker/build.sh release
```

- To build a debug version

```sh
docker/build.sh debug
```
