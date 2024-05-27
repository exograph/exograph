// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=TARGET");
    if !std::env::var("TARGET").unwrap().starts_with("wasm") {
        // TODO: Simplify this once https://github.com/rust-lang/cargo/pull/12158 lands
        let graphiql_folder_path = std::env::current_dir()?
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("graphiql");

        let graphiql_lib_path = graphiql_folder_path.join("lib");
        let graphiql_app_path = graphiql_folder_path.join("app");

        let npm = which::which("npm").map_err(|e| format!("Failed to find npm: {e}"))?;

        for sub_folder in &[&graphiql_lib_path, &graphiql_app_path] {
            for dependent_path in &[
                "src",
                "public",
                "package.json",
                "package-lock.json",
                "tsconfig.json",
                "index.html",
                "vite.config.js",
            ] {
                if sub_folder.join(dependent_path).exists() {
                    println!(
                        "cargo:rerun-if-changed={}",
                        sub_folder.join(dependent_path).display()
                    );
                }
            }

            if !std::process::Command::new(npm.clone())
                .arg("ci")
                .current_dir(sub_folder)
                .spawn()?
                .wait()?
                .success()
            {
                panic!("Failed to install graphiql dependencies");
            }

            if !std::process::Command::new(npm.clone())
                .arg("run")
                .arg("build")
                .current_dir(sub_folder)
                .spawn()?
                .wait()?
                .success()
            {
                panic!("Failed to build graphiql");
            }
        }
    }

    Ok(())
}
