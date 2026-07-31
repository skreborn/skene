//! Workspace automation tasks.
//!
//! Run with `cargo xtask <command>`. The `web` command builds the `counter`
//! demo to wasm, generates its bindings, and serves it over HTTP.

use std::{
  env,
  ffi::OsStr,
  fs,
  io::{BufRead, BufReader, Write},
  net::{TcpListener, TcpStream},
  path::{Path, PathBuf},
  process::{Command, ExitCode},
  thread,
};

use clap::{Parser, Subcommand};

const DEMO_CRATE: &str = "counter";
const WASM_STEM: &str = "counter";
#[cfg(not(target_arch = "wasm32"))]
const SERVER_STEM: &str = "counter-server";
const DEMO_DIR: &str = "examples/counter";
const DEFAULT_PORT: u16 = 8080;

/// Workspace automation tasks.
#[derive(Parser)]
#[command(name = "xtask", about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Option<Task>,
}

#[derive(Subcommand)]
enum Task {
  /// Build the `counter-web` demo to wasm, generate bindings, and serve it.
  Web(WebArgs),
  /// Statically generate the demo: render each route through the embedded runner.
  Ssg(SsgArgs),
}

#[derive(clap::Args)]
struct WebArgs {
  /// Port to serve the demo on.
  #[arg(long, default_value_t = DEFAULT_PORT)]
  port: u16,
  /// Build without optimizations instead of `--release`.
  #[arg(long)]
  debug: bool,
  /// Build and generate bindings without starting the server.
  #[arg(long)]
  no_serve: bool,
}

#[derive(clap::Args)]
struct SsgArgs {
  /// Routes to render.
  #[arg(long, default_values_t = [String::from("/")])]
  path: Vec<String>,
  /// Output directory, relative to the workspace root.
  #[arg(long, default_value = "examples/counter/ssg")]
  out: String,
  /// Build without optimizations instead of `--release`.
  #[arg(long)]
  debug: bool,
}

impl Default for WebArgs {
  fn default() -> Self {
    Self {
      port: DEFAULT_PORT,
      debug: false,
      no_serve: false,
    }
  }
}

fn main() -> ExitCode {
  let cli = Cli::parse();
  let task = cli.command.unwrap_or(Task::Web(WebArgs::default()));

  let result = match task {
    Task::Web(args) => web(&args),
    Task::Ssg(args) => ssg(&args),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("error: {message}");

      ExitCode::FAILURE
    }
  }
}

/// Statically generate the demo: build the server component, render each route, write
/// `<out>/<route>/index.html`.
#[cfg(target_arch = "wasm32")]
fn ssg(_args: &SsgArgs) -> Result<(), String> {
  Err("ssg runs on the host".to_owned())
}

/// Statically generate the demo: build the server component, render each route, write
/// `<out>/<route>/index.html`.
#[cfg(not(target_arch = "wasm32"))]
fn ssg(args: &SsgArgs) -> Result<(), String> {
  let root = workspace_root();
  let profile = if args.debug { "debug" } else { "release" };
  let mut build = Command::new("cargo");

  build
    .current_dir(&root)
    .args(["build", "-p", DEMO_CRATE, "--target", "wasm32-wasip2"]);

  if !args.debug {
    build.arg("--release");
  }

  let status = build.status().map_err(|error| error.to_string())?;

  if !status.success() {
    return Err("building the server component failed".to_owned());
  }

  let artifact = root
    .join("target/wasm32-wasip2")
    .join(profile)
    .join(format!("{SERVER_STEM}.wasm"));
  let runner = skene_runner::Runner::load(&artifact).map_err(|error| format!("{error:#}"))?;
  let out_root = root.join(&args.out);

  for route in &args.path {
    let page = runner
      .render(route)
      .map_err(|error| format!("render {route}: {error:#}"))?;
    let directory = out_root.join(route.trim_start_matches('/'));

    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

    let file = directory.join("index.html");

    fs::write(&file, page).map_err(|error| error.to_string())?;
    println!("rendered {route} -> {}", file.display());
  }

  Ok(())
}

/// Build the demo to wasm, generate bindings, and serve it unless `--no-serve`.
fn web(args: &WebArgs) -> Result<(), String> {
  let release = !args.debug;
  let root = workspace_root();
  build_wasm(&root, release)?;
  generate_bindings(&root, release)?;

  if !args.no_serve {
    serve_dir(&root.join(DEMO_DIR), args.port)?;
  }

  Ok(())
}

fn build_wasm(root: &Path, release: bool) -> Result<(), String> {
  let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
  let mut command = Command::new(cargo);
  command.current_dir(root).args([
    "build",
    "--target",
    "wasm32-unknown-unknown",
    "-p",
    DEMO_CRATE,
  ]);

  if release {
    command.arg("--release");
  }

  run_command(&mut command, "cargo build")
}

fn generate_bindings(root: &Path, release: bool) -> Result<(), String> {
  warn_on_version_mismatch(root);

  let profile = if release { "release" } else { "debug" };
  let wasm = root
    .join("target/wasm32-unknown-unknown")
    .join(profile)
    .join(format!("{WASM_STEM}.wasm"));
  let out_dir = root.join(DEMO_DIR).join("dist");
  let mut command = Command::new("wasm-bindgen");
  command
    .arg(&wasm)
    .arg("--out-dir")
    .arg(&out_dir)
    .args(["--target", "web"]);

  run_command(&mut command, "wasm-bindgen").map_err(|error| {
    format!("{error}\nis `wasm-bindgen-cli` installed? `cargo install wasm-bindgen-cli`")
  })
}

/// Warn when the installed `wasm-bindgen` CLI disagrees with the locked crate,
/// the usual cause of a failed bindings step.
fn warn_on_version_mismatch(root: &Path) {
  let Some(crate_version) = locked_wasm_bindgen_version(root)
  else {
    return;
  };
  let Some(cli_version) = Command::new("wasm-bindgen")
    .arg("--version")
    .output()
    .ok()
    .filter(|output| output.status.success())
    .and_then(|output| String::from_utf8(output.stdout).ok())
    .and_then(|text| text.split_whitespace().nth(1).map(str::to_string))
  else {
    return;
  };

  if cli_version != crate_version {
    eprintln!(
      "warning: wasm-bindgen CLI {cli_version} != crate {crate_version}; \
       run `cargo install -f wasm-bindgen-cli --version {crate_version}` if bindings fail"
    );
  }
}

fn locked_wasm_bindgen_version(root: &Path) -> Option<String> {
  let lock = fs::read_to_string(root.join("Cargo.lock")).ok()?;
  let mut lines = lock.lines();

  while let Some(line) = lines.next() {
    if line == "name = \"wasm-bindgen\"" {
      let version = lines.next()?;

      return version
        .strip_prefix("version = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string);
    }
  }

  None
}

fn run_command(command: &mut Command, label: &str) -> Result<(), String> {
  let status = command
    .status()
    .map_err(|error| format!("failed to run {label}: {error}"))?;

  if status.success() {
    Ok(())
  }
  else {
    Err(format!("{label} exited with {status}"))
  }
}

/// Serve `dir` over HTTP on `port` until interrupted, one thread per request.
fn serve_dir(dir: &Path, port: u16) -> Result<(), String> {
  let listener = TcpListener::bind(("127.0.0.1", port))
    .map_err(|error| format!("failed to bind 127.0.0.1:{port}: {error}"))?;
  println!("serving {} at http://localhost:{port}", dir.display());

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let dir = dir.to_path_buf();
        thread::spawn(move || handle_request(&stream, &dir));
      }
      Err(error) => eprintln!("connection failed: {error}"),
    }
  }

  Ok(())
}

fn handle_request(mut stream: &TcpStream, dir: &Path) {
  let Some(target) = read_target(stream)
  else {
    let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");

    return;
  };
  let relative = target.trim_start_matches('/');
  let relative = if relative.is_empty() {
    "index.html"
  }
  else {
    relative
  };
  let path = dir.join(relative);

  if !path.starts_with(dir) || !path.is_file() {
    let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"not found");

    return;
  }

  match fs::read(&path) {
    Ok(body) => {
      let _ = write_response(&mut stream, "200 OK", content_type(&path), &body);
    }
    Err(_) => {
      let _ = write_response(
        &mut stream,
        "500 Internal Server Error",
        "text/plain",
        b"read error",
      );
    }
  }
}

/// Parse the request path from the first line, rejecting directory traversal.
fn read_target(stream: &TcpStream) -> Option<String> {
  let mut line = String::new();
  BufReader::new(stream).read_line(&mut line).ok()?;
  let target = line.split_whitespace().nth(1)?;
  let target = target.split(['?', '#']).next().unwrap_or(target);

  if target.contains("..") {
    return None;
  }

  Some(target.to_string())
}

fn content_type(path: &Path) -> &'static str {
  match path.extension().and_then(OsStr::to_str) {
    Some("html") => "text/html; charset=utf-8",
    Some("js" | "mjs") => "text/javascript; charset=utf-8",
    Some("wasm") => "application/wasm",
    Some("css") => "text/css; charset=utf-8",
    Some("json" | "map") => "application/json",
    _ => "application/octet-stream",
  }
}

fn write_response(
  stream: &mut &TcpStream,
  status: &str,
  content_type: &str,
  body: &[u8],
) -> std::io::Result<()> {
  let header = format!(
    "HTTP/1.1 {status}\r\n\
     Content-Type: {content_type}\r\n\
     Content-Length: {}\r\n\
     Connection: close\r\n\r\n",
    body.len()
  );
  stream.write_all(header.as_bytes())?;
  stream.write_all(body)?;
  stream.flush()
}

fn workspace_root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("xtask lives under the workspace root")
    .to_path_buf()
}
