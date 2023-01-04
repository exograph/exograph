use actix_cors::Cors;
use actix_web::http::header::{CacheControl, CacheDirective};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use core_resolver::system_resolver::SystemResolver;
use resolver::{
    create_system_resolver_or_exit, get_endpoint_http_path, get_playground_http_path, graphiql,
};
use server_actix::resolve;
use tracing_actix_web::TracingLogger;

use std::io::ErrorKind;
use std::path::Path;
use std::time;
use std::{env, process::exit};

/// Run the server in production mode with a compiled claypot file
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let start_time = time::SystemTime::now();
    let claypot_file = get_claypot_file_name();

    resolver::init();

    let system_resolver = web::Data::new(create_system_resolver_or_exit(&claypot_file));

    let server_port = env::var("CLAY_SERVER_PORT")
        .map(|port_str| {
            port_str
                .parse::<u32>()
                .expect("Failed to parse CLAY_SERVER_PORT")
        })
        .unwrap_or(9876);
    let server_url = format!("0.0.0.0:{}", server_port);

    let resolve_path = get_endpoint_http_path();
    let playground_path = get_playground_http_path();
    let playground_path_subpaths = format!("{}/{{path:.*}}", playground_path);

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
                    println!("\thttp://{}{}", addr, suffix);
                }
            };

            println!("- Playground hosted at:");
            print_all_addrs(get_playground_http_path());

            println!("- Endpoint hosted at:");
            print_all_addrs(get_endpoint_http_path());

            server.run().await
        }
        Err(e) => {
            if e.kind() == ErrorKind::AddrInUse {
                eprintln!("Error: Port {} is already in use. Check if there is another process running at that port.", server_port);
            } else {
                eprintln!("Error: Failed to start server: {}", e);
            }
            exit(1);
        }
    }
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
