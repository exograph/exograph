use actix_cors::Cors;
use actix_web::http::header::{CacheControl, CacheDirective};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use core_resolver::system_resolver::SystemResolver;

use resolver::{allow_introspection, get_endpoint_http_path, get_playground_http_path, graphiql};
use server_actix::resolve;
use tracing_actix_web::TracingLogger;

use std::io::ErrorKind;
use std::path::Path;
use std::time;
use std::{env, process::exit};

/// Run the server in production mode with a compiled exo_ir file
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let start_time = time::SystemTime::now();

    let system_resolver = web::Data::new(server_common::init());

    let server_port = env::var("EXO_SERVER_PORT")
        .map(|port_str| {
            port_str
                .parse::<u32>()
                .expect("Failed to parse EXO_SERVER_PORT")
        })
        .unwrap_or(9876);
    let server_url = format!("0.0.0.0:{server_port}");

    let resolve_path = get_endpoint_http_path();
    let playground_path = get_playground_http_path();
    let playground_path_subpaths = format!("{playground_path}/{{path:.*}}");

    let server = HttpServer::new(move || {
        let cors = cors_from_env();

        App::new()
            .wrap(TracingLogger::default())
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .wrap(cors)
            .app_data(system_resolver.clone())
            .route(&resolve_path, web::post().to(resolve))
            .route(&playground_path, web::get().to(playground))
            .route(&playground_path_subpaths, web::get().to(playground))
    })
    .bind(&server_url);

    match server {
        Ok(server) => {
            println!(
                "Started server on {} in {:.2} ms",
                server.addrs()[0],
                start_time.elapsed().unwrap().as_micros() as f64 / 1000.0
            );

            let print_all_addrs = |suffix| {
                for addr in server.addrs() {
                    println!("\thttp://{addr}{suffix}");
                }
            };

            if let Ok(true) = allow_introspection() {
                println!("- Playground hosted at:");
                print_all_addrs(get_playground_http_path());
            }

            println!("- Endpoint hosted at:");
            print_all_addrs(get_endpoint_http_path());

            server.run().await
        }
        Err(e) => {
            if e.kind() == ErrorKind::AddrInUse {
                eprintln!("Error: Port {server_port} is already in use. Check if there is another process running at that port.");
            } else {
                eprintln!("Error: Failed to start server: {e}");
            }
            exit(1);
        }
    }
}

async fn playground(req: HttpRequest, resolver: web::Data<SystemResolver>) -> impl Responder {
    if !resolver.allow_introspection() {
        return HttpResponse::Forbidden().body("Introspection is not enabled");
    }

    let asset_path = req.match_info().get("path");

    // Adjust the path for "index.html" (which is requested with and empty path)
    let index = "index.html";
    let asset_path = asset_path
        .map(|path| if path.is_empty() { index } else { path })
        .unwrap_or(index);

    let asset_path = Path::new(asset_path);
    let extension = asset_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or(""); // If no extension, set it to an empty string, to use `actix_files::file_extension_to_mime`'s default behavior

    let content_type = actix_files::file_extension_to_mime(extension);

    // we shouldn't cache the index page, as we substitute in the endpoint path dynamically
    let cache_control = if index == "index.html" {
        CacheControl(vec![CacheDirective::NoCache])
    } else {
        CacheControl(vec![
            CacheDirective::Public,
            CacheDirective::MaxAge(60 * 60 * 24 * 365), // seconds in one year
        ])
    };

    match graphiql::get_asset_bytes(asset_path) {
        Some(asset) => HttpResponse::Ok()
            .content_type(content_type)
            .insert_header(cache_control)
            .body(asset),
        None => HttpResponse::NotFound().body("Not found"),
    }
}

fn cors_from_env() -> Cors {
    match env::var("EXO_CORS_DOMAINS").ok() {
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
