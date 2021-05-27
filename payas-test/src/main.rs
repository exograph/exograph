pub mod payastest;

use payastest::loader::load_testfiles_from_dir;
use payastest::runner::run_testfile;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let current_dir: String = std::env::current_dir()
        .unwrap()
        .to_str().unwrap()
        .to_string();

    let directory = args.get(1).unwrap_or(&current_dir);
    println!("Running tests in directory {} ...", directory);

    // Load testfiles
    let testfiles = load_testfiles_from_dir(&directory);
   
    // Run testfiles
    for testfile in testfiles {
        run_testfile(&testfile, std::env::var("PAYAS_DATABASE_URL").unwrap());
    }
}