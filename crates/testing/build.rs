fn main() {
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=package-lock.json");

    if !std::process::Command::new("npm")
        .arg("ci")
        .current_dir(".")
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
    {
        panic!("Failed to install graphql dependencies");
    }
}
