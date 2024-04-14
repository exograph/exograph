// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::env;
use std::io::Write;
use std::path::PathBuf;
use tree_sitter_cli::generate;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::Builder::new().prefix("grammar").tempdir()?;
    let grammar_file = dir.path().join("parser.c");
    let mut f = std::fs::File::create(grammar_file)?;

    let grammar = generate::load_grammar_file(&PathBuf::from("./grammar/grammar.js"))?;
    let (grammar_name, grammar_c) = generate::generate_parser_for_grammar(&grammar)?;
    f.write_all(grammar_c.as_bytes())?;
    drop(f);

    let header_dir = dir.path().join("tree_sitter");
    std::fs::create_dir(&header_dir)?;
    let mut parser_file = std::fs::File::create(header_dir.join("parser.h"))?;
    parser_file.write_all(tree_sitter::PARSER_HEADER.as_bytes())?;
    drop(parser_file);

    let sysroot_dir = dir.path().join("sysroot");
    if env::var("TARGET").unwrap().starts_with("wasm32") {
        std::fs::create_dir(&sysroot_dir).unwrap();
        let mut stdint = std::fs::File::create(sysroot_dir.join("stdint.h")).unwrap();
        stdint
            .write_all(include_bytes!("wasm-sysroot/stdint.h"))
            .unwrap();
        drop(stdint);

        let mut stdlib = std::fs::File::create(sysroot_dir.join("stdlib.h")).unwrap();
        stdlib
            .write_all(include_bytes!("wasm-sysroot/stdlib.h"))
            .unwrap();
        drop(stdlib);

        let mut stdio = std::fs::File::create(sysroot_dir.join("stdio.h")).unwrap();
        stdio
            .write_all(include_bytes!("wasm-sysroot/stdio.h"))
            .unwrap();
        drop(stdio);

        let mut stdbool = std::fs::File::create(sysroot_dir.join("stdbool.h")).unwrap();
        stdbool
            .write_all(include_bytes!("wasm-sysroot/stdbool.h"))
            .unwrap();
        drop(stdbool);
    }

    cc::Build::new()
        .include(&dir)
        .include(&sysroot_dir)
        .file(dir.path().join("parser.c"))
        .compile(&grammar_name);

    Ok(())
}
