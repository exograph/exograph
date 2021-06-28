#!/bin/sh

# Reexport DATABASE_URL provided by fly.io to what Payas needs
export CLAY_DATABASE_URL=${DATABASE_URL}

./clay-server "$@"