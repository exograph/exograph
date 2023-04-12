# Building

Build docker images suitable for Fly and Azure ("debian") and for AWS Lambda ("amazonlinux2").

From the root directory of the project, run:

- To build a release version

```sh
docker/build.sh release
```

- To build a debug version

```sh
docker/build.sh debug
```
