use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=./grammar");

    let sitter_out = Command::new("tree-sitter")
        .arg("generate")
        .current_dir(fs::canonicalize("./grammar").unwrap())
        .output()
        .unwrap();

    if !sitter_out.status.success() {
        println!("{}", String::from_utf8_lossy(&sitter_out.stderr));
        panic!("Compiling the grammar failed");
    }

    let dir: PathBuf = ["grammar", "src"].iter().collect();

    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .compile("grammar");
}
