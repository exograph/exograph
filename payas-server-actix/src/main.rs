use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};

use payas_server_actix::request_context::ActixRequestContextProducer;
use payas_server_actix::resolve;
use payas_server_core::create_query_executor;
use payas_sql::Database;

use std::time;
use std::{env, process::exit};

/// Run the server in production mode with a compiled claypot file
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let start_time = time::SystemTime::now();
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

    let database = Database::from_env(None).expect("Failed to create database"); // TODO: error handling here
    let query_executor = web::Data::new(create_query_executor(&claypot_file, database).unwrap());
    let request_context_processor = web::Data::new(ActixRequestContextProducer::new());

    let server_port = env::var("CLAY_SERVER_PORT")
        .map(|port_str| {
            port_str
                .parse::<u32>()
                .expect("Failed to parse CLAY_SERVER_PORT")
        })
        .unwrap_or(9876);
    let server_url = format!("0.0.0.0:{}", server_port);

    let server = HttpServer::new(move || {
        let cors = cors_from_env();

        App::new()
            .wrap(cors)
            .app_data(query_executor.clone())
            .app_data(request_context_processor.clone())
            .route("/", web::get().to(playground))
            .route("/", web::post().to(resolve))
    })
    .workers(1) // see payas-deno/executor.rs
    .bind(&server_url)
    .unwrap();

    println!(
        "Started server on {} in {:.2} ms",
        server.addrs()[0],
        start_time.elapsed().unwrap().as_micros() as f64 / 1000.0
    );

    server.run().await
}

async fn playground() -> impl Responder {
    HttpResponse::Ok().body(include_str!("assets/playground.html"))
}

fn cors_from_env() -> Cors {
    match env::var("CLAY_CORS_DOMAINS").ok() {
        Some(domains) => {
            let domains_list = domains.split(',');

            let cors = domains_list.fold(Cors::default(), |cors, domain| {
                if domain == "*" {
                    cors.allow_any_origin()
                } else {
                    cors.allowed_origin(domain)
                }
            });

            // TODO: Allow more control over headers, max_age etc
            cors.allowed_methods(vec!["GET", "POST"])
                .allow_any_header()
                .max_age(3600)
        }
        None => Cors::default(),
    }
}
