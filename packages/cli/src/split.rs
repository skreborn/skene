//! Splitting the built app: `#[lazy]` bodies into lazily-fetched wasm modules.
//!
//! Runs after wasm-bindgen when `skene.toml` sets `[web] split = true`. The split tool
//! moves each `#[lazy]` body—and the code only lazy bodies reach—out of the main
//! module, into one module per lazy function plus shared chunks. The loader manifest,
//! `__wasm_split.js` beside the bindings, fetches and links a module on its first
//! call. A build carrying `#[lazy]` markers without the opt-in is an error: the
//! bindings would import loaders that nothing provides.

use std::{fmt::Write as _, fs, path::Path};

use wasm_split_cli::{OutputModules, SplitModule, Splitter};

use crate::manifest::App;

/// The prefix `#[lazy]` stamps into its split import and export names.
const MARKER: &[u8] = b"__wasm_split_00";

/// The glue fetching and linking a split module on its first call: `makeLoad(url, deps,
/// initSync)` makes a module's loader, whose load in flight every caller shares, whose
/// callback each caller's call runs exactly once with whether the load succeeded, and
/// whose failed load is forgotten, so the next call starts afresh. Called with no
/// callback, as a dependency is, it returns the load.
const LOADER_GLUE: &str = include_str!("split.js");

/// Splits the bindgened wasm in `dist/web` when the app opts in, `original` the wasm
/// cargo built it from.
///
/// Without `#[lazy]` markers in the build this is a no-op; with markers but no
/// `[web] split = true` it fails, naming the missing setting.
pub(crate) fn split_web(app: &App, original: &Path) -> Result<(), String> {
  let out = app.dir.join("dist/web");
  let main_path = out.join(format!("{}_bg.wasm", app.stem));
  let bindgened =
    fs::read(&main_path).map_err(|error| format!("{}: {error}", main_path.display()))?;

  if !contains(&bindgened, MARKER) {
    return Ok(());
  }

  if !app.split {
    return Err(
      "the app carries #[lazy] functions, which need split builds: set `[web] split = true` \
       in skene.toml"
        .into(),
    );
  }

  let original = fs::read(original).map_err(|error| format!("{}: {error}", original.display()))?;
  let output = Splitter::new(&original, &bindgened)
    .and_then(Splitter::emit)
    .map_err(|error| format!("wasm-split: {error:#}"))?;

  fs::write(&main_path, &output.main.bytes).map_err(|error| error.to_string())?;

  for (index, chunk) in output.chunks.iter().enumerate() {
    let file = out.join(chunk_file(index, chunk));

    fs::write(&file, &chunk.bytes).map_err(|error| format!("{}: {error}", file.display()))?;
  }

  for (index, module) in output.modules.iter().enumerate() {
    let file = out.join(module_file(index, module));

    fs::write(&file, &module.bytes).map_err(|error| format!("{}: {error}", file.display()))?;
  }

  let manifest = out.join("__wasm_split.js");

  fs::write(&manifest, loader_manifest(app, &output)?).map_err(|error| error.to_string())?;
  println!(
    "split {} lazy module(s) and {} shared chunk(s) out of {}",
    output.modules.len(),
    output.chunks.len(),
    main_path.display(),
  );

  Ok(())
}

/// A shared chunk's file name in `dist/web`.
fn chunk_file(index: usize, chunk: &SplitModule) -> String {
  format!("chunk_{index}_{}.wasm", chunk.module_name)
}

/// A lazy module's file name in `dist/web`.
fn module_file(index: usize, module: &SplitModule) -> String {
  format!(
    "module_{index}_{}.wasm",
    module.component_name.as_deref().unwrap_or("lazy"),
  )
}

/// The loader manifest: the fetch-and-link glue plus one loader export per chunk and
/// per lazy module, under the names the split bindings import.
fn loader_manifest(app: &App, output: &OutputModules) -> Result<String, String> {
  let mut manifest = format!(
    "import {{ initSync }} from \"./{}.js\";\n{LOADER_GLUE}",
    app.stem,
  );

  for (index, chunk) in output.chunks.iter().enumerate() {
    let _ = writeln!(
      manifest,
      "export const __wasm_split_load_chunk_{index} = \
       makeLoad(new URL(\"{file}\", import.meta.url), [], initSync);",
      file = chunk_file(index, chunk),
    );
  }

  for (index, module) in output.modules.iter().enumerate() {
    let hash = module
      .hash_id
      .as_deref()
      .ok_or("wasm-split: a lazy module carries no hash")?;
    let component = module
      .component_name
      .as_deref()
      .ok_or("wasm-split: a lazy module carries no function name")?;
    let mut chunks: Vec<usize> = module.relies_on_chunks.iter().copied().collect();

    chunks.sort_unstable();

    let dependencies = chunks
      .iter()
      .map(|chunk| format!("__wasm_split_load_chunk_{chunk}"))
      .collect::<Vec<_>>()
      .join(", ");
    let _ = writeln!(
      manifest,
      "export const __wasm_split_load_{module}_{hash}_{component} = \
       makeLoad(new URL(\"{file}\", import.meta.url), [{dependencies}], initSync);",
      module = module.module_name,
      file = module_file(index, module),
    );
  }

  Ok(manifest)
}

/// Whether `haystack` contains `needle`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
  haystack
    .windows(needle.len())
    .any(|window| window == needle)
}
