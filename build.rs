use std::{env, ffi::OsStr, path::Path, process::Command};

fn main() {
    let path = env::var("CARGO_MANIFEST_DIR").unwrap();
    let dir = Path::new(&path);

    let mut command = Command::new("tailwindcss");

    command.args([
        OsStr::new("-i"),
        dir.join("static/input.css").as_os_str(),
        OsStr::new("-o"),
        dir.join("public/out.css").as_os_str(),
    ]);

    if cfg!(not(debug_assertions)) {
        command.arg("-m");
    }

    command.output().unwrap();
}
