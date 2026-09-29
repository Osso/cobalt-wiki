mod render_identity;

use std::path::PathBuf;

fn main() {
    let backend_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let locales_root = backend_root.join("../locales");
    let identity = render_identity::fingerprint(&backend_root, &locales_root)
        .expect("Failed to fingerprint rendering inputs");
    println!("cargo:rustc-env=DEEPWELL_RENDER_ID={identity}");
    for path in render_identity::input_roots(&backend_root, &locales_root) {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    // Compile-time build information.
    built::write_built_file().expect("Failed to write build information");

    // Trigger recompilation when a new migration is added.
    println!("cargo:rerun-if-changed=migrations");
}
