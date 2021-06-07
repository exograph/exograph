pub mod payastest;

use futures::future::join_all;
use payastest::loader::load_testfiles_from_dir;
use payastest::runner::run_testfile;

#[actix_web::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let current_dir: String = std::env::current_dir()
        .unwrap()
        .to_str().unwrap()
        .to_string();

    let directory = args.get(1).unwrap_or(&current_dir);
    println!("* Running tests in directory {} ...", directory);

    // Load testfiles
    let testfiles = load_testfiles_from_dir(&directory).unwrap();
   
    // Run testfiles in parallel
    let all_tests_succeded = join_all(testfiles.into_iter().map(|t| async move {
        run_testfile(&t, std::env::var("PAYAS_TEST_DATABASE_URL").unwrap()).await
    }))
        .await
        .into_iter()
        .fold(true, |accum, test_status| {
            match test_status {
                Ok(test_result) => { accum && test_result }
                Err(e) => { println!("Testfile failure: {:?}", e); false }
            }
        });

    if all_tests_succeded {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}