#!/usr/bin/env bash

SCRIPT_DIR=`dirname -- "$0"`;

cargo about generate $SCRIPT_DIR/about.hbs > $SCRIPT_DIR/../licenses.html