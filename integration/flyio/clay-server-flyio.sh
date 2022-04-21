#!/bin/sh

# Reexport DATABASE_URL provided by fly.io to what Clay needs
export CLAY_DATABASE_URL=${DATABASE_URL}

./clay-server "$@"