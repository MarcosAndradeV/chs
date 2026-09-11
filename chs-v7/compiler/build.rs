use std::path::{Path, PathBuf};
use std::process::Command;

fn get_workspace_dir() -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    let output = Command::new(cargo)
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .expect("Failed to execute cargo locate-project");

    let cargo_toml_path = Path::new(
        std::str::from_utf8(&output.stdout)
            .expect("Failed to parse cargo output as UTF-8")
            .trim(),
    );

    cargo_toml_path
        .parent()
        .expect("Failed to find workspace root directory")
        .to_path_buf()
}

fn main() {
    let workspace_dir = get_workspace_dir();
    let std_dir = workspace_dir.join("std/runtime/lib");

    let chs_runtime_c = std_dir.join("chs_runtime.c");
    let gc_c = std_dir.join("gc/gc.c");
    let log_c = std_dir.join("gc/log.c");
    let chs_runtime_o = std_dir.join("chs_runtime.o"); // Intermediate object file
    let libchs_runtime_so = std_dir.join("libchs_runtime.so"); // Shared output
    let libchs_runtime_a = std_dir.join("libchs_runtime.a"); // Static output
    let gc_o = std_dir.join("gc.o");
    let log_o = std_dir.join("log.o");

    // 1. Compile C to Object File (.o)
    // -fPIC is crucial here because this object will be used in a shared library
    let gcc_cmd = |file_c: &Path, flie_o: &Path| {
        Command::new("gcc")
            .arg(&file_c)
            .args([
                "-O3",
                "-fPIC",
                "-flto",
                "-ffunction-sections",
                "-fdata-sections",
                "-shared",
                "-Wl,--gc-sections",
                "-Wl,-O1",
                "-c",
                "-o",
            ])
            .arg(&flie_o)
            .status()
            .expect("Failed to execute gcc for object compilation")
    };
    let status = gcc_cmd(&chs_runtime_c, &chs_runtime_o);
    assert!(status.success(), "GCC object compilation failed");
    let status = gcc_cmd(&gc_c, &gc_o);
    assert!(status.success(), "GCC object compilation failed");
    let status = gcc_cmd(&log_c, &log_o);
    assert!(status.success(), "GCC object compilation failed");

    // 2. Create Static Library (.a) from the Object File
    let status = Command::new("ar")
        .arg("crus")
        .arg(&libchs_runtime_a)
        .arg(&chs_runtime_o)
        .arg(&gc_o)
        .arg(&log_o)
        .status()
        .expect("Failed to execute ar for static library");
    assert!(status.success(), "AR static archiving failed");

    // 3. Create Shared Library (.so) from the Object File
    let status = Command::new("gcc")
        .args(["-shared", "-o"])
        .arg(&libchs_runtime_so)
        .arg(&chs_runtime_o)
        .status()
        .expect("Failed to execute gcc for shared library linking");
    assert!(status.success(), "GCC shared linking failed");

    // (Optional) Clean up the intermediate .o file to keep the folder tidy
    let _ = std::fs::remove_file(chs_runtime_o);

    // Tell Cargo to re-run this script if any of these files change
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=std/runtime/lib/chs_runtime.c");
}
