//! The generated header, kept honest. Adapted from Fathom's, line for line.
//!
//! `targets/vcv` compiles against a committed `karplus.h`. If it were
//! generated at build time only, a change to the Rust side that the C++ side
//! had not seen would show up as a linker error in someone else's checkout.
//! Committing it and failing the build when it drifts is the cheaper failure.

use std::path::PathBuf;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The header the current source would produce.
fn generate() -> String {
    let mut buffer = Vec::new();
    cbindgen::generate(crate_dir())
        .expect("cbindgen could not read the crate")
        .write(&mut buffer);
    String::from_utf8(buffer).expect("cbindgen produced invalid UTF-8")
}

/// Everything from the include guard on, trimmed. The banner above it carries
/// no interface information.
fn interface(header: &str) -> String {
    header
        .lines()
        .skip_while(|line| !line.contains("KARPLUS_H"))
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_committed_header_matches_the_source() {
    let committed = std::fs::read_to_string(crate_dir().join("include/karplus.h"))
        .expect("crates/karplus-ffi/include/karplus.h is missing. Run `just header`.");
    assert_eq!(
        interface(&committed),
        interface(&generate()),
        "\nkarplus.h is out of date. Run `just header` and commit the result.\n"
    );
}

#[test]
fn the_header_declares_everything_the_shim_calls() {
    let header = generate();
    for symbol in [
        "karplus_voice_new",
        "karplus_voice_free",
        "karplus_voice_reset",
        "karplus_voice_process",
        "karplus_params_default",
        "KarplusParams",
        "KarplusExciter",
        "KarplusVoice",
    ] {
        assert!(header.contains(symbol), "the header is missing `{symbol}`");
    }
}

#[test]
fn the_header_is_usable_from_cpp() {
    let header = generate();
    assert!(header.contains("extern \"C\""), "no C++ linkage guard");
    assert!(header.contains("KARPLUS_H"), "no include guard");
}

/// Compile the committed header on its own, as C and as C++.
///
/// The strongest statement available without the Rack SDK: whatever else is
/// true of the shim, the interface it includes is valid in both languages.
fn compile_header_as(language: &str, compiler: &str) {
    let header = crate_dir().join("include/karplus.h");
    let dir = std::env::temp_dir().join(format!("karplus-header-{language}"));
    std::fs::create_dir_all(&dir).expect("could not create a scratch directory");
    let source = dir.join(format!(
        "main.{}",
        if language == "c" { "c" } else { "cpp" }
    ));
    std::fs::write(
        &source,
        format!(
            "#include \"{}\"\n\
             #include \"{}\"\n\
             int main(void) {{ return (int)sizeof(KarplusParams) > 0 ? 0 : 1; }}\n",
            header.display(),
            header.display()
        ),
    )
    .expect("could not write the probe");

    let output = std::process::Command::new(compiler)
        .arg("-fsyntax-only")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(&source)
        .output();
    let Ok(output) = output else {
        eprintln!("skipping: {compiler} is not installed");
        return;
    };
    assert!(
        output.status.success(),
        "{compiler} rejected the header:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_committed_header_compiles_as_c() {
    // Included twice, so the include guard is exercised too.
    compile_header_as("c", "cc");
}

#[test]
fn the_committed_header_compiles_as_cpp() {
    compile_header_as("cpp", "c++");
}
