use actix_cors::Cors;
use actix_web::http::header::{CacheControl, CacheDirective};
use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use payas_server_actix::request_context::ActixRequestContextProducer;
use payas_server_actix::{resolve, telemetry};
use payas_server_core::graphiql;
use payas_server_core::{create_operations_executor, OperationsExecutor};
use tracing_actix_web::TracingLogger;

use std::path::Path;
use std::time;
use std::{env, process::exit};

/// Run the server in production mode with a compiled claypot file
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let start_time = time::SystemTime::now();
    let claypot_file = get_claypot_file_name();

    let subscriber_name = claypot_file.trim_end_matches(".claypot");
    telemetry::init(subscriber_name);

    let operations_executor = web::Data::new(create_operations_executor(&claypot_file).unwrap());
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
            .wrap(TracingLogger::default())
            .wrap(cors)
            .app_data(operations_executor.clone())
            .app_data(request_context_processor.clone())
            .service(playground)
            .service(resolve)
    })
    .bind(&server_url)
    .unwrap();

    println!(
        "Started server on {} in {:.2} ms",
        server.addrs()[0],
        start_time.elapsed().unwrap().as_micros() as f64 / 1000.0
    );

    server.run().await
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

#[get("/{path:.*}")]
async fn playground(req: HttpRequest, executor: web::Data<OperationsExecutor>) -> impl Responder {
    if !executor.allow_introspection {
        return HttpResponse::Forbidden().body("Introspection is not enabled");
    }

    let asset_path = req.match_info().get("path");

    // Adjust the path for "index.html" (which is requested with and empty path)
    let asset_path = asset_path.map(|path| if path.is_empty() { "index.html" } else { path });

    match asset_path {
        Some(asset_path) => {
            let asset_path = Path::new(asset_path);
            let extension = asset_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""); // If no extension, set it to an empty string, to use `actix_files::file_extension_to_mime`'s default behavior

            let content_type = actix_files::file_extension_to_mime(extension);

            // js/cs files in the static folder have content-hashed names, so we can cache them for a long duration
            let cache_control = if asset_path.starts_with("static/") {
                CacheControl(vec![
                    CacheDirective::Public,
                    CacheDirective::MaxAge(60 * 60 * 24 * 365), // seconds in one year
                ])
            } else {
                CacheControl(vec![CacheDirective::NoCache])
            };

            match graphiql::get_asset_bytes(asset_path) {
                Some(asset) => HttpResponse::Ok()
                    .content_type(content_type)
                    .insert_header(cache_control)
                    .body(asset),
                None => HttpResponse::NotFound().body(""),
            }
        }
        None => HttpResponse::NotFound().body("Not found"),
    }
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
