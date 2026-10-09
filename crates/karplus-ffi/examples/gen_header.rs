//! `gen_header` — write `include/karplus.h` from the Rust source.
//!
//! Run by `just header`. The result is committed, and `just verify` fails if
//! it has drifted from the source.
//!
//! An example rather than a binary because it needs `cbindgen`, which is a
//! dev-dependency: nothing that ships is allowed to depend on it.

use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = crate_dir.join("include/karplus.h");
    std::fs::create_dir_all(out.parent().expect("include/ has a parent"))
        .expect("could not create the include directory");
    cbindgen::generate(&crate_dir)
        .expect("cbindgen could not read the crate")
        .write_to_file(&out);
    println!("wrote {}", out.display());
}
