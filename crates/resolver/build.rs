// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Simplify this once https://github.com/rust-lang/cargo/pull/12158 lands
    let graphiql_folder_path = std::env::current_dir()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("graphiql");

    println!(
        "cargo:rerun-if-changed={}",
        graphiql_folder_path.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        graphiql_folder_path.join("public").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        graphiql_folder_path.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        graphiql_folder_path.join("package-lock.json").display()
    );

    let npm = which::which("npm").map_err(|e| format!("Failed to find npm: {}", e))?;

    if !std::process::Command::new(npm.clone())
        .arg("ci")
        .current_dir(&graphiql_folder_path)
        .spawn()?
        .wait()?
        .success()
    {
        panic!("Failed to install graphiql dependencies");
    }

    if !std::process::Command::new(npm)
        .arg("run")
        .arg("prod-build")
        .current_dir(graphiql_folder_path)
        .spawn()?
        .wait()?
        .success()
    {
        panic!("Failed to build graphiql");
    }

    Ok(())
}
