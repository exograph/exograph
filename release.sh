#!/usr/bin/env bash

cargo build --release
zip -j target/release.zip target/release/exo target/release/exo-server target/release/*.dylib