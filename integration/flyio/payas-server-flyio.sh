#!/bin/sh

# Reexport DATABASE_URL provided by fly.io to what Payas needs
export PAYAS_DATABASE_URL=${DATABASE_URL}

./payas-server "$@"