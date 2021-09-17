use std::{env, time::SystemTime};

use payas_server::start_prod_mode;

fn main() {
    let system_start_time = SystemTime::now();

    let model_file = env::args().nth(1).expect("Usage: clay-server <model-file>");

    start_prod_mode(&model_file, Some(system_start_time)).unwrap();
}
