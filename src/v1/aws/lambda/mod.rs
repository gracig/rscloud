pub mod function;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context};

pub fn get_cargo_lambda_function_code(function_name: &RustLambdaEntry) -> anyhow::Result<Vec<u8>> {
    find_function(function_name).and_then(|f| get_file_as_byte_vec(&f))
}
fn get_file_as_byte_vec(filename: &PathBuf) -> anyhow::Result<Vec<u8>> {
    let mut f = File::open(filename).context("no file found")?;
    let metadata = fs::metadata(filename).context("unable to read metadata")?;
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).context("buffer overflow")?;
    Ok(buffer)
}
use std::env;

use super::RustLambdaEntry;

fn find_function(function_name: &RustLambdaEntry) -> anyhow::Result<PathBuf> {
    let mut current_dir =
        env::current_dir().map_err(|_| anyhow!("Could not retrieve the current directory"))?;

    loop {
        let target_dir = current_dir.join("target");
        if target_dir.is_dir() {
            let function_path = target_dir
                .join("lambda")
                .join(&function_name.bin_name)
                .join("bootstrap.zip");

            let mut cmd = Command::new("rustup");
            cmd.arg("target")
                .arg("add")
                .arg("x86_64-unknown-linux-musl");
            let output = cmd.output().context("Could not install musl tools")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!(
                    "rustup isntall musl tools has failed: {}",
                    stderr
                ));
            }

            let mut cmd = Command::new("cargo");
            cmd.arg("lambda")
                .arg("build")
                .arg("--release")
                .arg("--output-format")
                .arg("zip")
                .arg("--package")
                .arg(&function_name.package_name)
                .arg("--bin")
                .arg(&function_name.bin_name)
                .arg("--target")
                .arg("x86_64-unknown-linux-musl");
            let output = cmd.output().context("Could not build function")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Cargo build failed: {}", stderr));
            }

            if function_path.is_file() {
                return Ok(function_path);
            }
        }
        if !current_dir.pop() {
            return Err(anyhow!(
                "Function not found in any 'target/lambda' directory"
            ));
        }
    }
}
