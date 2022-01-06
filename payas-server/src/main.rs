use anyhow::{Context, Result};
use bincode::deserialize_from;
use payas_model::model::system::ModelSystem;
use payas_server::start_server;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::{env, process::exit};

/// Run the server in production mode with a compiled claypot file
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);

    if args.len() > 1 {
        // $ clay-server <model-file-name> extra-arguments...
        println!("Usage: clay-server <claypot-file>");
        exit(1)
    }

    let claypot_file = if args.len() == 0 {
        // $ clay-server
        "index.claypot".to_string()
    } else {
        let file_name = args.next().unwrap();

        if file_name.ends_with(".claypot") {
            // $ clay-server concerts.claypot
            file_name
        } else if file_name.ends_with(".clay") {
            // $ clay-server concerts.clay
            format!("{}pot", file_name)
        } else {
            println!("The input file {} doesn't appear to be a claypot. You need build one with the 'clay build <model-file-name>' command.", file_name);
            exit(1);
        }
    };

    let server = open_claypot_file(&claypot_file)
        .and_then(start_server)
        .map_err(|e| {
            println!("{}", e);
            exit(1)
        });

    server.unwrap().await
}

fn open_claypot_file(claypot_file: &str) -> Result<ModelSystem> {
    if !Path::new(&claypot_file).exists() {
        anyhow::bail!("File '{}' not found", claypot_file);
    }
    match File::open(&claypot_file) {
        Ok(file) => {
            let claypot_file_buffer = BufReader::new(file);
            let in_file = BufReader::new(claypot_file_buffer);
            deserialize_from(in_file)
                .with_context(|| format!("Failed to read claypot file {}", claypot_file))
        }
        Err(e) => {
            anyhow::bail!("Failed to open claypot file {}: {}", claypot_file, e)
        }
    }
}
