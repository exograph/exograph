use std::{env, process::exit, time::SystemTime};

use payas_server::start_prod_mode;

fn main() {
    let system_start_time = SystemTime::now();

    let mut args = env::args().skip(1);

    if args.len() == 0 {
        // $ clay-server
        start_prod_mode("index.claypot", Some(system_start_time)).unwrap();
    } else if args.len() == 1 {
        let file_name = args.next().unwrap();

        let claypot_file = if file_name.ends_with(".claypot") {
            // $ clay-server concerts.claypot
            file_name
        } else if file_name.ends_with(".clay") {
            // $ clay-server concerts.clay
            format!("{}pot", file_name)
        } else {
            println!("The input file {} doesn't appear to be a claypot. You need build one with the 'clay build <model-file-name>' command.", file_name);
            exit(1);
        };

        start_prod_mode(&claypot_file, Some(system_start_time)).unwrap()
    } else {
        // $ clay-server <model-file-name> extra-arguments...
        println!("Usage: clay-server <claypot-file>");
        exit(1);
    }
}
