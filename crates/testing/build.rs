// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=package-lock.json");

    let npm = which::which("npm").map_err(|e| format!("Failed to find npm: {}", e))?;

    if !std::process::Command::new(npm)
        .arg("ci")
        .spawn()?
        .wait()?
        .success()
    {
        panic!("Failed to install graphql dependencies");
    }

    Ok(())
}
