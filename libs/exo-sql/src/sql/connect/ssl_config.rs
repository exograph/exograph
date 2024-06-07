#![cfg(feature = "postgres-url")]

use std::{fs::File, io::BufReader};

#[cfg(feature = "tls")]
use rustls::{Certificate, RootCertStore};
#[cfg(feature = "tls")]
use rustls_native_certs::load_native_certs;

use crate::database_error::DatabaseError;
use tokio_postgres::config::SslMode;

pub(crate) struct SslConfig {
    mode: SslMode,
    root_cert_path: Option<String>,
}

impl SslConfig {
    pub(crate) fn from_url(url: &str) -> Result<(String, Option<SslConfig>), DatabaseError> {
        let url = url::Url::parse(url)
            .map_err(|_| DatabaseError::Config("Invalid database URL".into()))?;

        let mut ssl_param_string: Option<String> = None;
        let mut ssl_mode_string: Option<String> = None;
        let mut ssl_root_cert_string = None;

        // Remove parameters from the url that typical postgres URL includes (for example, with YugabyteDB),
        // but the tokio-rust-postgres driver doesn't support yet.
        // Instead capture those parameters and use them later in the connection/ssl config.
        let query_pairs = url.query_pairs().filter(|(name, value)| {
            if name == "ssl" {
                ssl_param_string = Some(value.to_string());
                false
            } else if name == "sslmode" {
                ssl_mode_string = Some(value.to_string());
                false
            } else if name == "sslrootcert" {
                ssl_root_cert_string = Some(value.to_string());
                false
            } else {
                true
            }
        });

        let mut cleaned_url = url.clone();
        cleaned_url
            .query_pairs_mut()
            .clear()
            .extend_pairs(query_pairs);

        // We need to replace '+' (encoded from a space character) with '%20' since the tokio-rust-postgres driver doesn't seem to support
        // the encoding that uses '+' for a space.
        let url = cleaned_url.as_str().replace('+', "%20");

        let mut ssl_mode = SslMode::Prefer;

        // See: https://jdbc.postgresql.org/documentation/head/ssl-client.html
        // 1. "ssl" parameter is a quick way to specify SSL mode. If it is true, then it has the same effect as setting "sslmode" to "verify-full".
        //    So we process this first.
        if let Some(ssl_param) = ssl_param_string {
            let ssl_param_parsed = ssl_param.as_str().parse();
            match ssl_param_parsed {
                Ok(true) => ssl_mode = SslMode::Require,
                Ok(false) => ssl_mode = SslMode::Prefer,
                _ => {
                    return Err(DatabaseError::Config(format!(
                        "Invalid 'ssl' parameter value {ssl_param}. Must be a 'true' or 'false'",
                    )));
                }
            }
        }
        // 2. The tokio-postgres library doesn't have a way to map all possible values of "sslmode", so we pick the nearest stricter mode.
        //    We process this the next to allow any refinement of the SSL mode set through the simpler "ssl" parameter.
        if let Some(ssl_mode_string) = ssl_mode_string {
            match ssl_mode_string.as_str() {
                "verify-full" | "verify-ca" | "require" => ssl_mode = SslMode::Require,
                "prefer" | "allow" => ssl_mode = SslMode::Prefer,
                "disable" => ssl_mode = SslMode::Disable,
                _ => {
                    return Err(DatabaseError::Config(format!(
                        "Invalid 'sslmode' parameter value {ssl_mode_string}"
                    )))
                }
            }
        }

        let ssl_config = if ssl_mode == SslMode::Disable {
            None
        } else {
            Some(SslConfig {
                mode: ssl_mode,
                root_cert_path: ssl_root_cert_string,
            })
        };

        Ok((url, ssl_config))
    }

    #[cfg(feature = "tls")]
    pub(crate) fn updated_config(
        self,
        mut config: tokio_postgres::Config,
    ) -> Result<
        (
            tokio_postgres::Config,
            tokio_postgres_rustls::MakeRustlsConnect,
        ),
        DatabaseError,
    > {
        config.ssl_mode(self.mode);

        let tls = {
            let mut root_store = RootCertStore::empty();

            // If the cert path is provided, use it. Otherwise, use the native certs.
            match self.root_cert_path {
                Some(cert_path) => {
                    let cert_file = File::open(&cert_path).map_err(|e| {
                        DatabaseError::Config(format!(
                            "Failed to open certificate file '{cert_path}': {e}"
                        ))
                    })?;
                    let mut buf = BufReader::new(cert_file);
                    rustls_pemfile::certs(&mut buf)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| DatabaseError::Config("Invalid certificate".into()))?
                        .into_iter()
                        .map(|cert| root_store.add(&Certificate(cert.to_vec())))
                        .collect::<Result<Vec<_>, _>>()?;
                }
                None => {
                    for cert in load_native_certs()? {
                        root_store.add(&Certificate(cert.to_vec()))?;
                    }
                }
            }

            let config = rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            tokio_postgres_rustls::MakeRustlsConnect::new(config)
        };

        Ok((config, tls))
    }
}
