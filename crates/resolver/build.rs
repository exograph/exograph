// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

fn main() {
    let graphiql_folder_path = std::env::current_dir()
        .unwrap()
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

    let npm = which::which("npm").unwrap();

    if !std::process::Command::new(npm.clone())
        .arg("ci")
        .current_dir(&graphiql_folder_path)
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
    {
        panic!("Failed to install graphiql dependencies");
    }

    if !std::process::Command::new(npm)
        .arg("run")
        .arg("prod-build")
        .current_dir(graphiql_folder_path)
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
    {
        panic!("Failed to build graphiql");
    }
}
