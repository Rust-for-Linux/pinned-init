extern crate compiletest_rs as compiletest;

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn run_mode(mode: &'static str) {
    let mut config = compiletest::Config::default();

    config.mode = mode.parse().expect("Invalid mode");
    config.src_base = PathBuf::from(format!("tests/{}", mode));
    config.link_deps(); // Populate config.target_rustcflags with dependencies on the path
    config.clean_rmeta(); // If your tests import the parent crate, this helps with E0464
    #[cfg(target_os = "macos")]
    {
        // for some reason macos does not add this path automatically:
        let mut flags = config.target_rustcflags.take().unwrap_or("".to_owned());
        flags.push_str("-L target/debug/deps");
        config.target_rustcflags = Some(flags);
    }
    #[cfg(target_os = "windows")]
    {
        // windows paths are broken https://github.com/Manishearth/compiletest-rs/issues/149
        // we just set our own flag:
        config.target_rustcflags = Some("-L target\\debug\\deps".to_owned());
    }

    //config.bless = true;

    run_tests(&config);

    if config.bless {
        panic!("TURN OFF BLESS");
    }
}

fn run_tests(config: &compiletest::Config) {
    // Since our tests contain hints that reference rust-src, we have to insert the correct paths
    // into the `$RUSTC_SRC` variable contained in the tests. Then after we run the tests, we have
    // to revert this again.

    let rust_src_path = {
        let output = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .expect("Failed to determine rustc sysroot");
        let mut stdout = output.stdout;
        // remove trailing `\n`
        if stdout.last() == Some(&b'\n') {
            stdout.pop();
        }
        let mut path =
            PathBuf::from(String::from_utf8(stdout).expect("Non Unicode path to rust sysroot"));
        path.extend(["lib", "rustlib", "src", "rust"]);
        path.into_os_string()
            .into_string()
            .expect("Non Unicode path to rust-src")
    };
    #[cfg(windows)]
    let rust_src_path = rust_src_path.replace("\\", "/");
    let originals = inject_rust_src_recurse(&config.src_base, &rust_src_path)
        .expect("Failed to inject rust-src path");

    let res = std::panic::catch_unwind(|| compiletest::run_tests(config));

    // Restore the original contents.
    let errors = originals
        .into_iter()
        .map(|orig| std::fs::write(orig.path, orig.contents))
        .filter(|e| e.is_err())
        .map(|e| e.unwrap_err())
        .collect::<Vec<_>>();

    for err in &errors {
        println!("{err:?}");
    }

    match res {
        Ok(()) => {}
        Err(e) => std::panic::resume_unwind(e),
    }
    if errors.len() > 0 {
        panic!("Writing original files failed");
    }
}

struct OriginalContents {
    path: PathBuf,
    contents: String,
}

fn inject_rust_src_recurse(
    path: &Path,
    rust_src_path: &str,
) -> std::io::Result<Vec<OriginalContents>> {
    let mut vec = vec![];
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            vec.extend(inject_rust_src_recurse(&entry.path(), rust_src_path)?);
        } else if entry.file_type()?.is_file() {
            vec.push(inject_rust_src(&entry.path(), rust_src_path)?);
        }
    }
    Ok(vec)
}

fn inject_rust_src(file_path: &Path, rust_src_path: &str) -> std::io::Result<OriginalContents> {
    let contents = std::fs::read_to_string(file_path)?;
    // Windows again requires special treating, because of CLRF.................
    #[cfg(windows)]
    // somehow the contents we read have `\r\n`s added...
    let contents = contents.replace("\r\n", "\n");
    let orig = OriginalContents {
        path: file_path.to_owned(),
        contents: contents.clone(),
    };
    let contents = contents.replace("$RUSTC_SRC", rust_src_path);
    std::fs::write(file_path, contents)?;
    Ok(orig)
}

#[cfg_attr(not(any(miri, NO_UI_TESTS)), test)]
fn compile_test() {
    run_mode("ui");
}
