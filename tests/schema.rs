/*
** Copyright (C) 2025 Sylvain Fargier
**
** This software is provided 'as-is', without any express or implied
** warranty.  In no event will the authors be held liable for any damages
** arising from the use of this software.
**
** Permission is granted to anyone to use this software for any purpose,
** including commercial applications, and to alter it and redistribute it
** freely, subject to the following restrictions:
**
** 1. The origin of this software must not be misrepresented; you must not
**    claim that you wrote the original software. If you use this software
**    in a product, an acknowledgment in the product documentation would be
**    appreciated but is not required.
** 2. Altered source versions must be plainly marked as such, and must not be
**    misrepresented as being the original software.
** 3. This notice may not be removed or altered from any source distribution.
**
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{
    fs,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, ensure};
use serial_test::file_serial;

fn find_jsonschema() -> Result<String> {
    match Command::new("check-jsonschema").arg("--help").status() {
        Ok(ret) if ret.success() => Ok(String::from("check-jsonschema")),
        _ => {
            if !std::fs::exists("target/venv/bin/check-jsonschema").unwrap_or(false) {
                ensure!(
                    Command::new("python")
                        .args(["-m", "venv", "target/venv"])
                        .status()?
                        .success(),
                    "failed to create venv"
                );
                ensure!(
                    Command::new("target/venv/bin/pip")
                        .args(["install", "check-jsonschema"])
                        .status()?
                        .success(),
                    "failed to run pip"
                );
            }
            Ok(String::from("target/venv/bin/check-jsonschema"))
        }
    }
}

#[test]
#[file_serial(venv)]
fn validate_schema() -> Result<()> {
    let json_schema_exe = find_jsonschema()?;
    for file in fs::read_dir("./data")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|f| f.contains("config") && f.ends_with("yml"))
        })
        .map(|e| e.path())
    {
        println!("validating schema on {:?}", file);
        let yq = Command::new("yq")
            .arg("-o=json")
            .arg(file)
            .stdout(Stdio::piped())
            .spawn()
            .context("yq failed")?;

        let mut validate = Command::new(&json_schema_exe)
            .args(["--schemafile", "./data/config.schema.json", "-"])
            .stdin(yq.stdout.unwrap())
            .spawn()
            .context("check-jsonschema failed")?;

        assert!(validate.wait()?.success());
    }
    Ok(())
}
