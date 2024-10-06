use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::fs;
use reqwest;

const TOOLCHAIN_URL: &str = "https://developer.arm.com/-/media/Files/downloads/gnu-rm/10.3-2021.10/gcc-arm-none-eabi-10.3-2021.10-x86_64-linux.tar.bz2";
const TOOLCHAIN_ARCHIVE: &str = "gcc-arm-none-eabi-10.3-2021.10-x86_64-linux.tar.bz2";

fn main() {
    // Fetch TF-M
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let tfm_dir = out_dir.join("trusted-firmware-m");
    let toolchain_dir = out_dir.join("gcc-arm-none-eabi");
    let venv_dir = out_dir.join("tfm_venv");

    // Ensure TF-M is cloned and up-to-date
    fetch_tfm(&tfm_dir);

    // Download and prepare Arm GNU Embedded Toolchain
    prepare_toolchain(&out_dir, &toolchain_dir);

    // Prepare Python virtual environment
    prepare_python_venv(&venv_dir, &tfm_dir);

    // Configure and build TF-M
    let dst = cmake::Config::new(&tfm_dir)
        .define("TFM_PLATFORM", "arm/rse/tc/tc3")
        .define("TFM_PROFILE", "profile_medium")
        .define("TEST_S", "ON")
        .define("TEST_S_CRYPTO", "ON")
        .define("CMAKE_C_COMPILER", toolchain_dir.join("bin/arm-none-eabi-gcc"))
        .define("CMAKE_CXX_COMPILER", toolchain_dir.join("bin/arm-none-eabi-g++"))
        .define("CMAKE_ASM_COMPILER", toolchain_dir.join("bin/arm-none-eabi-gcc"))
        .env("VIRTUAL_ENV", &venv_dir)
        .env("PATH", format!("{}:{}", venv_dir.join("bin").display(), env::var("PATH").unwrap()))
        .build_arg("install")
        .build();

    // Generate PSA crypto bindings
    let interface_include = out_dir.join("interface").join("include");
    bindgen::Builder::default()
        .header(interface_include.join("psa/crypto.h").to_str().unwrap())
        .clang_arg(format!("-I{}", interface_include.display()))

        // For now, hardcode these to those used by TC3. This will require some rudimentary parsing
        // of spe_export.cmake.
        .clang_arg(format!("-DMBEDTLS_PSA_CRYPTO_CONFIG_FILE=\"{}\"", tfm_dir.join("lib/ext/mbedcrypto/mbedcrypto_config/crypto_config_profile_medium.h").to_str().unwrap()))
        .clang_arg(format!("-DMBEDTLS_CONFIG_FILE=\"{}\"", tfm_dir.join("lib/ext/mbedcrypto/mbedcrypto_config/tfm_mbedcrypto_config_client.h").to_str().unwrap()))

        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .use_core()
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_dir.join("tfm_bindings.rs"))
        .expect("Couldn't write bindings!");

    // Link the built libraries
    println!("cargo:rustc-link-arg={}/interface/lib/s_veneers.o", out_dir.display());

    // Set environment variables for other parts of the build process
    println!("cargo:rustc-env=TFM_BUILD_DIR={}", dst.display());
    println!("cargo:rustc-env=ARM_TOOLCHAIN_DIR={}", toolchain_dir.display());
}

fn fetch_tfm(tfm_dir: &PathBuf) {
    if tfm_dir.exists() {
        // Try to update existing repository
        let status = Command::new("git")
            .current_dir(tfm_dir)
            .args(&["pull", "origin", "main"])
            .status()
            .expect("Failed to update TF-M repository");

        if status.success() {
            println!("TF-M repository updated successfully");
            return;
        }
    }

    // Clone failed or directory doesn't exist, remove it if it exists
    if tfm_dir.exists() {
        fs::remove_dir_all(tfm_dir).expect("Failed to remove existing TF-M directory");
    }

    // Clone the repository
    let status = Command::new("git")
        .args(&[
            "clone",
            "https://git.trustedfirmware.org/TF-M/trusted-firmware-m.git",
            tfm_dir.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to clone TF-M repository");

    if !status.success() {
        panic!("Failed to clone TF-M repository");
    }

    println!("TF-M repository cloned successfully")
}

fn prepare_toolchain(out_dir: &PathBuf, toolchain_dir: &PathBuf) {
    if toolchain_dir.exists() {
        println!("Toolchain already exists, skipping download");
        return;
    }

    let archive_path = out_dir.join(TOOLCHAIN_ARCHIVE);

    // Download the toolchain
    println!("Downloading Arm GNU Embedded Toolchain...");
    let mut response = reqwest::blocking::get(TOOLCHAIN_URL).expect("Failed to download toolchain");
    let mut file = fs::File::create(&archive_path).expect("Failed to create toolchain archive");
    std::io::copy(&mut response, &mut file).expect("Failed to write toolchain archive");

    // Extract the toolchain
    println!("Extracting Arm GNU Embedded Toolchain...");
    let status = Command::new("tar")
        .args(&["-xjf", archive_path.to_str().unwrap(), "-C", out_dir.to_str().unwrap()])
        .status()
        .expect("Failed to extract toolchain");

    if !status.success() {
        panic!("Failed to extract toolchain");
    }

    // Rename the extracted directory to our standard name
    let extracted_dir = out_dir.join("gcc-arm-none-eabi-10.3-2021.10");
    fs::rename(extracted_dir, toolchain_dir).expect("Failed to rename toolchain directory");

    // Clean up the archive
    fs::remove_file(archive_path).expect("Failed to remove toolchain archive");

    println!("Arm GNU Embedded Toolchain prepared successfully");
}

fn prepare_python_venv(venv_dir: &PathBuf, tfm_dir: &PathBuf) {
    if venv_dir.exists() {
        println!("Python virtual environment already exists, skipping creation");
        return;
    }

    // Create virtual environment
    println!("Creating Python virtual environment...");
    let status = Command::new("python3")
        .args(&["-m", "venv", venv_dir.to_str().unwrap()])
        .status()
        .expect("Failed to create Python virtual environment");

    if !status.success() {
        panic!("Failed to create Python virtual environment");
    }

    // Activate virtual environment and install requirements
    println!("Installing Python dependencies...");
    let requirements_file = tfm_dir.join("tools/requirements.txt");
    let status = Command::new(venv_dir.join("bin/pip"))
        .args(&["install", "-r", requirements_file.to_str().unwrap()])
        .env("VIRTUAL_ENV", venv_dir)
        .env("PATH", format!("{}:{}", venv_dir.join("bin").display(), env::var("PATH").unwrap()))
        .status()
        .expect("Failed to install Python dependencies");

    if !status.success() {
        panic!("Failed to install Python dependencies");
    }

    println!("Python virtual environment prepared successfully");
}