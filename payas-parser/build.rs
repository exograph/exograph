use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let sitter_out = Command::new("npx")
        .arg("tree-sitter-cli@0.19.5")
        .arg("generate")
        .current_dir(fs::canonicalize("./grammar").unwrap())
        .output()
        .unwrap();

    if !sitter_out.status.success() {
        println!("{}", String::from_utf8_lossy(&sitter_out.stderr));
        panic!("BOO");
    }

    let dir: PathBuf = ["grammar", "src"].iter().collect();

    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .compile("grammar");
}
