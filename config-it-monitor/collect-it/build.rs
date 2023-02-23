use anyhow::anyhow;

fn main() {
    if let Err(e) = try_compile_flatc() {
        eprintln!("cargo:warning=Failed to compile flatc: {}", e);
    }
}

fn try_compile_flatc() -> anyhow::Result<()> {
    if let Ok(flatc) = which::which("flatc") {
        // Find desired flatc version.
        // To not violate working version of generated protocols, we'll use the same
        //  flatbuffers version of Cargo.toml
        let cargo_toml = include_str!("Cargo.toml");
        let desired_version = cargo_toml
            .split("flatbuffers = \"")
            .last()
            .unwrap()
            .split("\"")
            .next()
            .unwrap()
            .parse::<i32>()?;

        match std::process::Command::new(&flatc).arg("--version").output() {
            Err(e) => panic!("Failed to execute 'flatc' command: {e}"),
            Ok(o) => {
                let version = std::str::from_utf8(&o.stdout).expect("Non-utf8");
                let fn_err = || anyhow!("Version format does not match");

                let major_version_str = version
                    .split(" ") // We'll parse 'flatc version 23.x.x'
                    .last()
                    .ok_or_else(fn_err)?
                    .split(".")
                    .next()
                    .ok_or_else(fn_err)?;

                let major_version = major_version_str
                    .parse::<i32>()
                    .map_err(|e| anyhow!("Invalid major version: {major_version_str} {e}"))?;

                if major_version != desired_version {
                    anyhow::bail!(
                        "flatc version {} mismatches required {}, skipping compilation ...",
                        major_version,
                        desired_version
                    );
                }
            }
        }

        // If we reached here, we have the correct version of flatc. Register protocol file
        //  as a dependency for recompilation.
        println!("cargo:rerun-if-changed=protocol.fbs");

        // Generate Rust code from flatbuffer schema
        let mut cmd = std::process::Command::new(&flatc);
        cmd.args(["--rust", "--no-prefix", "--rust-module-root-file"]);
        cmd.args(["-o", "protocol"]);
        cmd.arg("protocol.fbs");

        if let Err(e) = cmd.status() {
            panic!("Failed to generate Rust code from flatbuffer schema: {}", e);
        }
    }

    Ok(())
}
