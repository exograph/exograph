use lambda_runtime::Error;
use lambda_runtime::LambdaEvent;

use resolver::create_system_resolver_or_exit;
use serde_json::Value;
use server_aws_lambda::resolve;

use std::sync::Arc;
use std::{env, process::exit};

/// Run the server in production mode with a compiled claypot file
#[tokio::main]
async fn main() -> Result<(), Error> {
    let claypot_file = get_claypot_file_name();

    resolver::init();

    let system_context = Arc::new(create_system_resolver_or_exit(&claypot_file));
    let service = lambda_runtime::service_fn(|event: LambdaEvent<Value>| async {
        resolve(event, system_context.clone()).await
    });

    lambda_runtime::run(service).await?;

    Ok(())
}

fn get_claypot_file_name() -> String {
    let mut args = env::args().skip(1);

    if args.len() > 1 {
        // $ clay-server <model-file-name> extra-arguments...
        println!("Usage: clay-server <claypot-file>");
        exit(1)
    }

    if args.len() == 0 {
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
    }
}

#[cfg(test)]
mod tests {}
