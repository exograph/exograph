#!/usr/bin/env bash

cargo build --release
zip -j target/release.zip target/release/clay target/release/clay-server target/release/*.dylib