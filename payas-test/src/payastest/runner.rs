use crate::payastest::loader::ParsedTestfile;
use crate::payastest::dbutils::{createdb_psql, dropdb_psql};

// TODO actually run operations
pub fn run_testfile(testfile: &ParsedTestfile, dburl: String) {
    let mut test_counter: usize = 0;

    // iterate through our tests
    for (test_name, test_op) in &testfile.test_operations {
        test_counter += 1;

        let dbname = format!("{}test{}", &testfile.unique_dbname, &test_counter);

        // create a database
        createdb_psql(&dbname, &dburl).unwrap();

        // run the setup section
        for operation in testfile.setup_operations.iter() {
            println!("#{} SETUP {:?}", test_counter, operation)
        }

        // run the init section
        for operation in testfile.init_operations.iter() {
            println!("#{} INIT {:?}", test_counter, operation)
        }
            
        // run tests
        println!("#{} TEST {:?}", test_counter, test_op);

        // drop the database
        dropdb_psql(&dbname, &dburl).unwrap();
    }
    
}