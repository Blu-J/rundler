// This file is part of Rundler.
//
// Rundler is free software: you can redistribute it and/or modify it under the
// terms of the GNU Lesser General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later version.
//
// Rundler is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
// without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with Rundler.
// If not, see https://www.gnu.org/licenses/.

use std::{error, io::ErrorKind, process::Command};

fn main() -> Result<(), Box<dyn error::Error>> {
    println!("cargo:rerun-if-changed=tracer/package.json");
    println!("cargo:rerun-if-changed=tracer/src/validationTracer.ts");
    compile_tracer()?;
    Ok(())
}

fn compile_tracer() -> Result<(), Box<dyn error::Error>> {
    let install_url = "https://bun.sh/docs/installation";
    let action = "compile tracer";
    run_command(
        Command::new("bun").arg("install").current_dir("tracer"),
        install_url,
        action,
    )?;
    run_command(
        Command::new("bun")
            .args(["run", "bundle"])
            .current_dir("tracer"),
        install_url,
        action,
    )
}

fn run_command(
    command: &mut Command,
    install_page_url: &str,
    action: &str,
) -> Result<(), Box<dyn error::Error>> {
    let output = match command.output() {
        Ok(o) => o,
        Err(e) => {
            if let ErrorKind::NotFound = e.kind() {
                let program = command.get_program().to_str().unwrap();
                Err(format!(
                    "{program} not installed. See instructions at {install_page_url}"
                ))?;
            }
            Err(e)?
        }
    };
    if !output.status.success() {
        if let Ok(error_output) = String::from_utf8(output.stderr) {
            eprintln!("{error_output}");
        }
        Err(format!("Failed to {action}."))?;
    }
    Ok(())
}
