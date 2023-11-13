/* Parts of bellow code are taken from cargo-expand(https://github.com/dtolnay/cargo-expand) licensed under MIT OR Apache-2.0 */
use anyhow::Error;
use clap::Parser;
use indexmap::IndexMap;
use serde_derive::Deserialize;
use std::fs::File;
use std::io::{self, BufRead, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

pub mod extractor;
pub mod generator;

const ABOUT: &str = r#"Rust I18n command for help you simply to extract all untranslated texts from source code.

It will iter all Rust files in and extract all untranslated texts that used `t!` macro.
And then generate a YAML file and merge for existing texts.

https://github.com/PingPongun/rust-i18n
"#;

// Help headings
const PACKAGE_SELECTION: &str = "Package Selection";
const TARGET_SELECTION: &str = "Target Selection";
const FEATURE_SELECTION: &str = "Feature Selection";
const COMPILATION_OPTIONS: &str = "Compilation Options";
const MANIFEST_OPTIONS: &str = "Manifest Options";

#[derive(Parser)]
#[command(bin_name = "cargo", version, author, disable_help_subcommand = true)]
pub enum Subcommand {
    /// Show the result of macro expansion.
    #[command(name = "i18n", version, author, disable_version_flag = true)]
    #[command(about = "Extract rust-i18n keys from source code", long_about = ABOUT)]
    I18N(I18N),
}

#[derive(Parser, Debug)]
pub struct I18N {
    /// Print command lines as they are executed
    #[arg(long)]
    pub verbose: bool,

    /// Unstable (nightly-only) flags to Cargo
    #[arg(short = 'Z', value_name = "FLAG")]
    pub unstable_flags: Vec<String>,

    /// Print version
    #[arg(long)]
    pub version: bool,

    /// Directory for extracted translations
    #[arg(long, value_name = "DIRECTORY")]
    pub locales_dir: Option<PathBuf>,

    /// Package to expand
    #[arg(short, long, value_name = "SPEC", num_args = 0..=1, help_heading = PACKAGE_SELECTION)]
    pub package: Option<Option<String>>,

    /// Expand only this package's library
    #[arg(long, help_heading = TARGET_SELECTION)]
    pub lib: bool,

    /// Expand only the specified binary
    #[arg(long, value_name = "NAME", num_args = 0..=1, help_heading = TARGET_SELECTION)]
    pub bin: Option<Option<String>>,

    /// Expand only the specified example
    #[arg(long, value_name = "NAME", num_args = 0..=1, help_heading = TARGET_SELECTION)]
    pub example: Option<Option<String>>,

    /// Expand only the specified test target
    #[arg(long, value_name = "NAME", num_args = 0..=1, help_heading = TARGET_SELECTION)]
    pub test: Option<Option<String>>,

    /// Include tests when expanding the lib or bin
    #[arg(long, help_heading = TARGET_SELECTION)]
    pub tests: bool,

    /// Expand only the specified bench target
    #[arg(long, value_name = "NAME", num_args = 0..=1, help_heading = TARGET_SELECTION)]
    pub bench: Option<Option<String>>,

    /// Space or comma separated list of features to activate
    #[arg(short = 'F', long, value_name = "FEATURES", help_heading = FEATURE_SELECTION)]
    pub features: Option<String>,

    /// Activate all available features
    #[arg(long, help_heading = FEATURE_SELECTION)]
    pub all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long, help_heading = FEATURE_SELECTION)]
    pub no_default_features: bool,

    /// Build artifacts in release mode, with optimizations
    #[arg(long, help_heading = COMPILATION_OPTIONS)]
    pub release: bool,

    /// Build artifacts with the specified profile
    #[arg(long, value_name = "PROFILE-NAME", help_heading = COMPILATION_OPTIONS)]
    pub profile: Option<String>,

    /// Target triple which compiles will be for
    #[arg(long, value_name = "TARGET", help_heading = COMPILATION_OPTIONS)]
    pub target: Option<String>,

    /// Directory for all generated artifacts
    #[arg(long, value_name = "DIRECTORY", help_heading = COMPILATION_OPTIONS)]
    pub target_dir: Option<PathBuf>,

    /// Path to Cargo.toml
    #[arg(long, value_name = "PATH", help_heading = MANIFEST_OPTIONS)]
    pub manifest_path: Option<PathBuf>,

    /// Require Cargo.lock and cache are up to date
    #[arg(long, help_heading = MANIFEST_OPTIONS)]
    pub frozen: bool,

    /// Require Cargo.lock is up to date
    #[arg(long, help_heading = MANIFEST_OPTIONS)]
    pub locked: bool,

    /// Run without accessing the network
    #[arg(long, help_heading = MANIFEST_OPTIONS)]
    pub offline: bool,
}

pub fn extractor_main() -> Result<(), Error> {
    let Subcommand::I18N(args) = Subcommand::parse();

    let current_path: PathBuf = ".".into();
    let cfg = rust_i18n_support::config::load(
        &args.manifest_path.clone().unwrap_or(current_path.clone()),
    )?;
    let locales_dir = args
        .locales_dir
        .clone()
        .unwrap_or(cfg.load_path.clone().into());

    let temp_path = args
        .target_dir
        .clone()
        .unwrap_or_else(|| {
            let path: PathBuf = String::from_utf8(
                Command::new(cargo_binary())
                    .arg("locate-project")
                    .arg("--message-format")
                    .arg("plain")
                    .arg("--workspace")
                    .arg("-q")
                    .output()
                    .expect("failed to locate cargo project")
                    .stdout,
            )
            .expect("path contains unexpected characters")
            .trim()
            .into();
            path.parent().unwrap().join("target")
        })
        .join("temp___i18n_macro_expansion.rs");

    // expand macros by running cargo
    let mut cmd = Command::new(cargo_binary());
    apply_args(&mut cmd, &args, &temp_path);
    cmd.env("RUSTC_BOOTSTRAP", "1");

    if 0 == filter_err(&mut cmd)? {
        //read expanded file
        let mut s = String::new();
        let mut f = File::open(&temp_path).expect(&format!("Failed to open file: {:?}", temp_path));
        f.read_to_string(&mut s).expect("Failed to read file");

        //process file
        let mut results = IndexMap::new();
        extractor::extract(&mut results, &temp_path, &s)?;

        let mut messages: Vec<_> = results.values().collect();
        messages.sort_by_key(|m| m.index);

        generator::generate(&locales_dir, &cfg, messages.clone());
        std::process::exit(0);
    }
    Ok(())
}

fn cargo_binary() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".to_owned().into())
}
fn filter_err(cmd: &mut Command) -> io::Result<i32> {
    let mut child = cmd.stderr(Stdio::piped()).spawn()?;
    let mut stderr = io::BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    while let Ok(n) = stderr.read_line(&mut line) {
        if n == 0 {
            break;
        }
        if !ignore_cargo_err(&line) {
            let _ = write!(io::stderr(), "{}", line);
        }
        line.clear();
    }
    let code = child.wait()?.code().unwrap_or(1);
    Ok(code)
}

fn ignore_cargo_err(line: &str) -> bool {
    if line.trim().is_empty() {
        return true;
    }

    let discarded_lines = [
        "ignoring specified output filename because multiple outputs were \
         requested",
        "ignoring specified output filename for 'link' output because multiple \
         outputs were requested",
        "ignoring --out-dir flag due to -o flag",
        "ignoring -C extra-filename flag due to -o flag",
        "due to multiple output types requested, the explicitly specified \
         output file name will be adapted for each output type",
        "warning emitted",
        "warnings emitted",
        ") generated ",
    ];
    for s in &discarded_lines {
        if line.contains(s) {
            return true;
        }
    }

    false
}

fn apply_args(cmd: &mut Command, args: &I18N, outfile: &Path) {
    cmd.arg("rustc");

    if args.verbose {
        cmd.arg("--verbose");
    }

    for unstable_flag in &args.unstable_flags {
        cmd.arg("-Z");
        cmd.arg(unstable_flag);
    }

    if let Some(package) = &args.package {
        cmd.arg("--package");
        cmd.args(package);
    }

    let mut has_explicit_build_target = false;
    if args.lib {
        cmd.arg("--lib");
        has_explicit_build_target = true;
    }

    if let Some(bin) = &args.bin {
        cmd.arg("--bin");
        cmd.args(bin);
        has_explicit_build_target = true;
    }

    if let Some(example) = &args.example {
        cmd.arg("--example");
        cmd.args(example);
        has_explicit_build_target = true;
    }

    if let Some(test) = &args.test {
        cmd.arg("--test");
        cmd.args(test);
        has_explicit_build_target = true;
    }

    if let Some(bench) = &args.bench {
        cmd.arg("--bench");
        cmd.args(bench);
        has_explicit_build_target = true;
    }

    if !has_explicit_build_target {
        if let Ok(cargo_manifest) = parse_manifest(args.manifest_path.as_deref()) {
            if let Some(root_package) = cargo_manifest.package {
                if let Some(default_run) = &root_package.default_run {
                    cmd.arg("--bin");
                    cmd.arg(default_run);
                }
            }
        }
    }

    if let Some(features) = &args.features {
        cmd.arg("--features");
        cmd.arg(features);
    }

    if args.all_features {
        cmd.arg("--all-features");
    }

    if args.no_default_features {
        cmd.arg("--no-default-features");
    }

    cmd.arg("--profile");
    if let Some(profile) = &args.profile {
        cmd.arg(profile);
    } else if args.tests && args.test.is_none() {
        if args.release {
            cmd.arg("bench");
        } else {
            cmd.arg("test");
        }
    } else if args.release {
        cmd.arg("release");
    } else {
        cmd.arg("check");
    }

    if let Some(target) = &args.target {
        cmd.arg("--target");
        cmd.arg(target);
    }

    if let Some(target_dir) = &args.target_dir {
        cmd.arg("--target-dir");
        cmd.arg(target_dir);
    }

    if let Some(manifest_path) = &args.manifest_path {
        cmd.arg("--manifest-path");
        cmd.arg(manifest_path);
    }

    if args.frozen {
        cmd.arg("--frozen");
    }

    if args.locked {
        cmd.arg("--locked");
    }

    if args.offline {
        cmd.arg("--offline");
    }

    cmd.arg("--");

    cmd.arg("-o");
    cmd.arg(outfile);
    cmd.arg("-Zunpretty=expanded");
}

#[derive(Deserialize, Debug)]
pub struct CargoManifest {
    pub package: Option<CargoPackage>,
}

#[derive(Deserialize, Debug)]
pub struct CargoPackage {
    #[serde(rename = "default-run")]
    pub default_run: Option<String>,
}

pub fn parse_manifest(manifest_path: Option<&Path>) -> Result<CargoManifest, Error> {
    let manifest_path = find_cargo_manifest(manifest_path)?;
    let content = fs::read_to_string(manifest_path)?;
    let cargo_manifest: CargoManifest = toml::from_str(&content)?;
    Ok(cargo_manifest)
}

fn find_cargo_manifest(manifest_path: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(manifest_path) = manifest_path {
        return Ok(manifest_path.to_owned());
    }

    let dir = env::current_dir()?;
    let mut dir = dir.as_path();
    loop {
        let path = dir.join("Cargo.toml");
        if path.try_exists()? {
            return Ok(path);
        }
        dir = match dir.parent() {
            Some(parent) => parent,
            None => return Err(io::Error::new(ErrorKind::NotFound, "Cargo.toml not found")),
        };
    }
}
