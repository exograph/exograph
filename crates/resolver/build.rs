fn main() {
    println!(
        "resolver/build.rs cwd = {:?} {:?}",
        std::env::current_dir().unwrap(),
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    let graphiql_folder_path = std::env::current_dir().unwrap().join("../../graphiql");
    let graphiql_folder = graphiql_folder_path.to_str().unwrap();

    println!("cargo:rerun-if-changed={graphiql_folder}/src");
    println!("cargo:rerun-if-changed={graphiql_folder}/public");
    println!("cargo:rerun-if-changed={graphiql_folder}/package.json");
    println!("cargo:rerun-if-changed={graphiql_folder}/package-lock.json");

    if !std::process::Command::new("npm")
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

    if !std::process::Command::new("npm")
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
