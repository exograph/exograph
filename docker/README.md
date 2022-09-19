# Building

## Building a general purpose image (for fly and Azure)

From the root directory of the project, run:

- To build a release version

```sh
docker/build.sh debian release
```

- To build a debug version

```sh
docker/build.sh debian debug
```

# Building a Docker image for AWS Lambda that works with Amazon Linux 2

- To build a release version

```sh
docker/build.sh aws release
```

- To build a debug version

```sh
docker/build.sh aws debug
```
