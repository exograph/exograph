---
sidebar_position: 1
---

# Setup

## Prerequisites

Exograph is built using Rust, and makes use of Rust-specific frameworks and features. Because of this, plugins are written in Rust as well. You must first set up the Rust toolchain in your development environment.

There are many resources available online to set up Rust! As such, this guide will not cover setting up the toolchain.

:::warning
Exograph plugins currently target the internal Rust ABI! Because of this, plugins currently need to be built in-tree inside the Exograph source repository in order to work properly.
:::

## Steps

1. Make a project folder.

   ```shell-session
   # shell-command-next-line
   mkdir exograph-kv-plugin
   # shell-command-next-line
   cd exograph-kv-plugin
   ```

2. Using `cargo`, create new Rust library projects for each component:

   ```shell-session
   # shell-command-next-line
   cargo new --lib kv-model

   # shell-command-next-line
   cargo new --lib kv-model-builder
   # shell-command-next-line
   cargo new --lib kv-resolver
   ```

3. Create a `Cargo.toml` file with the following contents. This will declare a workspace to hold our crates.

   ```toml
   [workspace]

   members = [
       "kv-model",

       "kv-model-builder",
       "kv-resolver"
   ]

   # Exograph dependency
   [workspace.dependencies]
   core-plugin-interface = { git = "https://github.com/exograph/exograph" }
   ```

We now have a skeleton project for our plugin!

- `kv-model` will define the internal model of our plugin. This is what ultimately gets serialized into `.exo_ir` files.
- `kv-model-builder` will output a shared library used at build time to parse the `.exo` AST and build a plugin model (the builder portion of our plugin).
- `kv-resolver` will output a shared library used at runtime to resolve plugin operations (the resolver portion of our plugin).

Do the following for the three crates:

1. In `Cargo.toml`, add a dependency on the `core-plugin-interface` crate:
   ```toml
   [dependencies]
   core-plugin-interface.workspace = true
   ```
