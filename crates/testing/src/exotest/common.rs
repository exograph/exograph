use anyhow::{bail, Context, Error, Result};

use exo_sql::testing::db::EphemeralDatabase;
use isahc::HttpClient;
use std::{
    collections::HashMap,
    ffi::OsStr,
    fmt,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

/// Structure to hold open resources associated with a running testfile.
/// When dropped, we will clean them up.
pub(crate) struct TestfileContext {
    pub server: ServerInstance,
    pub db: Box<dyn EphemeralDatabase + Send + Sync>,
    pub jwtsecret: String,
    pub client: HttpClient,
    pub testvariables: HashMap<String, serde_json::Value>,
}

/// The result of running a testfile.
pub enum TestResultKind {
    Success,
    Fail(Error),
    SetupFail(Error),
}

impl Eq for TestResultKind {}

// We use a custom implementation of PartialEq (needed for sorting)
// that disregards the inner Error because they do not implement PartialEq themselves.
impl PartialEq for TestResultKind {
    fn eq(&self, other: &Self) -> bool {
        match self {
            TestResultKind::Success => matches!(other, TestResultKind::Success),
            TestResultKind::Fail(_) => matches!(other, TestResultKind::Fail(_)),
            TestResultKind::SetupFail(_) => matches!(other, TestResultKind::SetupFail(_)),
        }
    }
}

impl PartialOrd for TestResultKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(match self {
            TestResultKind::Success => {
                if matches!(other, TestResultKind::Success) {
                    std::cmp::Ordering::Equal
                } else {
                    std::cmp::Ordering::Greater
                }
            }

            _ => {
                if matches!(other, TestResultKind::Success) {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            }
        })
    }
}

// Represents the result of a test.
#[derive(PartialEq, Eq)]
pub struct TestResult {
    pub log_prefix: String,
    pub result: TestResultKind,

    /// The collected output (`stdout`, `stderr`) from a instance of `exo`.
    pub output: String,
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        matches!(self.result, TestResultKind::Success)
    }
}

impl PartialOrd for TestResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TestResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // If `a` is successful and `b` isn't, mark `a < b`, so that we get all successful tests
        // shown before the failed ones.
        if self.is_success() && !other.is_success() {
            std::cmp::Ordering::Less
        } else if !self.is_success() && other.is_success() {
            std::cmp::Ordering::Greater
        } else {
            // If both are successful or both are failure, compare it by their log_prefix
            // so multiple tests from the same folder are grouped together
            self.log_prefix.cmp(&other.log_prefix)
        }
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.result {
            TestResultKind::Success => {
                writeln!(
                    f,
                    "{} {}",
                    self.log_prefix,
                    ansi_term::Color::Green.paint("PASS")
                )
            }
            TestResultKind::Fail(e) => writeln!(
                f,
                "{} {}\n{:?}",
                self.log_prefix,
                ansi_term::Color::Yellow.paint("FAIL"),
                e
            ),
            TestResultKind::SetupFail(e) => writeln!(
                f,
                "{} {}\n{:?}",
                self.log_prefix,
                ansi_term::Color::Red.paint("TEST SETUP FAILED"),
                e
            ),
        }
        .unwrap();

        if !matches!(&self.result, TestResultKind::Success) {
            write!(
                f,
                "{}\n{}\n",
                ansi_term::Color::Purple.paint(":: Output:"),
                ansi_term::Color::Fixed(240).paint(&self.output)
            )
        } else {
            Ok(())
        }
    }
}

pub(crate) struct ServerInstance {
    server: Child,
    pub output: Arc<Mutex<String>>,
    pub endpoint: String,
}

impl Drop for ServerInstance {
    fn drop(&mut self) {
        // kill the started server
        if let e @ Err(_) = self.server.kill() {
            println!("Error killing server instance: {e:?}")
        }
    }
}

pub(crate) fn cmd(binary_name: &str) -> Command {
    // Pick up the current executable path and replace the file with the specified binary
    // This allows us to invoke `target/debug/exo test ...` or `target/release/exo test ...`
    // without updating the PATH env.
    // Thus, for the former invocation if the `binary_name` is `exo-server` the command will become
    // `<full-path-to>/target/debug/exo-server`
    let mut executable =
        std::env::current_exe().expect("Could not retrieve the current executable");
    executable.set_file_name(binary_name);
    Command::new(
        executable
            .to_str()
            .expect("Could not convert executable path to a string"),
    )
}

pub(crate) fn spawn_exo_server<I, K, V>(model_file: &Path, envs: I) -> Result<ServerInstance>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let _cli_child = cmd("exo")
        .arg("build")
        .arg(model_file.as_os_str())
        .output()?;

    let mut server = cmd("exo-server")
        .arg(model_file.as_os_str())
        .envs(envs) // add extra envs specified in testfile
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("exo-server failed to start")?;

    // wait for it to start
    const MAGIC_STRING: &str = "Started server on 0.0.0.0:";

    let mut server_stdout = BufReader::new(server.stdout.take().unwrap());
    let mut server_stderr = BufReader::new(server.stderr.take().unwrap());

    let mut line = String::new();
    server_stdout.read_line(&mut line).context(format!(
        r#"Failed to read output line for "{}" server"#,
        model_file.display()
    ))?;

    if !line.starts_with(MAGIC_STRING) {
        bail!(
            r#"Unexpected output from exo-server "{}": {}"#,
            model_file.display(),
            line
        )
    }

    // spawn threads to continually drain stdout and stderr
    let output_mutex = Arc::new(Mutex::new(String::new()));

    let stdout_output = output_mutex.clone();
    let _stdout_drain = std::thread::spawn(move || loop {
        let mut buf = String::new();
        let _ = server_stdout.read_line(&mut buf);
        stdout_output.lock().unwrap().push_str(&buf);
    });

    let stderr_output = output_mutex.clone();
    let _stderr_drain = std::thread::spawn(move || loop {
        let mut buf = String::new();
        let _ = server_stderr.read_line(&mut buf);
        stderr_output.lock().unwrap().push_str(&buf);
    });

    // take the digits part which represents the port (and ignore other information such as time to start the server)
    let port: String = line
        .trim_start_matches(MAGIC_STRING)
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let endpoint = format!("http://127.0.0.1:{port}/graphql");

    Ok(ServerInstance {
        server,
        output: output_mutex,
        endpoint,
    })
}
