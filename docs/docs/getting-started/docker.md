---
sidebar_position: 20
---

# Developing with Docker

An altenative to installing Exograph directly on your machine is to use Docker. Exograph provides an image suitable for development that includes all Exograph and Postgres binaries.

The development environment can be started using the following command:

```sh
# shell-command-next-line
docker run --rm --platform linux/x86_64 \
  --mount type=bind,source="$(pwd)",target=/usr/src/app \
  -p 9876:9876 \
  -it ghcr.io/exograph/dev:latest bash
```

This will start a container with the development environment and drop you into a bash shell. Note that the mount option ensure that any project you create will be persisted outside the container (specifically, in the current working directory).

Once in the shell, you can follow the steps in the [Getting Started](./local.md) guide.

You can also run this image for other purposes, which we will explore in the [Docker Deployment](../deployment/docker.md) guide.