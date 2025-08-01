use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

static EXAMPLE_ARTIFACTS: [&str; 3] = ["minimal-test", "deadlock-test", "custom-import"];

fn main() {
    // Trigger rebuild if the build.rs itself changes
    println!("cargo::rerun-if-changed=build.rs");

    // Trigger rebuild if ANY example changes
    for entry in glob::glob("examples/**/*.rs").unwrap() {
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }

    // Trigger rebuild if ANY artifact changes
    for entry in glob::glob("wasm-artifacts/**/*.rs").unwrap() {
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }

    // Determine the path to the sub-workspace that contains the source files
    // for the wasm artifacts
    let workspace_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("wasm-artifacts");

    // Get the path to output directory, which can then be used by the examples to embed the 
    // wasm-artifact
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Compile all necessary example wasm artifacts
    for artifact_name in EXAMPLE_ARTIFACTS {
        for tracing in [true, false] {
            // Compile the wasm-artifact
            let artifact_dir = compile_example_artifact(artifact_name, &workspace_root, tracing);

            // Derive the exact artifact file name
            let mut artifact_file = artifact_name.replace("-", "_");
            let mut out_file = artifact_file.clone();

            if tracing {
                out_file.push_str("_tracing");
            }
            artifact_file.push_str(".wasm");
            out_file.push_str(".wasm");
            

            //  Copy the artifact to the output directory
            let artifact_path = artifact_dir.join(&artifact_file);
            let out_path = out_dir.join(&out_file);
            fs::copy(&artifact_path, &out_path).unwrap_or_else(|e| {
                panic!(
                    "Failed to copy example artifact: {artifact_name} to output directory.\nError: {e}"
                )
            });
        }
    }
}

fn compile_example_artifact(artifact_name: &str, cwd: &PathBuf, tracing: bool) -> PathBuf {
    let feature_flag = if tracing {
        vec![OsStr::new("--features"), OsStr::new("tracing")]
    } else {
        vec![]
    };

    // This command will compile the wasm-artifact
    //
    // Note: It inherits the cargo and toolchain configuration located
    // inside given workspace root `cwd` (should be `wasm-artifacts`
    // in our case). Look at the `rust-toolchain.toml` and `.cargo/config.toml`
    // inside that directory for the exact configuration settings.
    //
    // Unfortunately, this is a bit hacky: 
    // We wipe the complete cargo build environment context by cleaning all environment
    // variables and import the original PATH environment variable afterwards.
    // We rely on the fact, that rustup, cargo and rustc are present on the PATH to  
    // compile our wasm-artifacts.
    let output = Command::new("cargo")
        .args([
            OsStr::new("build"),
            OsStr::new("--release"),
            OsStr::new("-p"),
            OsStr::new(artifact_name),
        ])
        .args(feature_flag)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").expect("PATH must be set!"))
        .output()
        .unwrap_or_else(|e| {
            panic!("Failed to invoke cargo for example artifact: {artifact_name}\nError: {e}")
        });
    
    assert!(
        output.status.success(),
        "Failed to build example artifact: {artifact_name}\nStatus Code: {}",
        output.status
            .code()
            .map(|code| code.to_string())
            .unwrap_or("None".into())
    );

    cwd.join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
}
