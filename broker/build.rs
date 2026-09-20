use std::fs;
use std::path::Path;

fn main() {
    // This is an accidental-mismatch fingerprint, not an authentication MAC.
    // Companion path ownership/mode and the transport token are checked apart.
    let mut hash = 0xcbf29ce484222325_u64;
    for path in [
        "Cargo.toml",
        "src/bootstrap.rs",
        "src/client.rs",
        "src/execution.rs",
        "src/identity.rs",
        "src/lib.rs",
        "src/manifest.rs",
        "src/server.rs",
        "src/transport.rs",
        "src/trusted_url.rs",
        "src/wire.rs",
    ] {
        println!("cargo:rerun-if-changed={path}");
        for byte in fs::read(Path::new(path)).expect("broker cohort source must be readable") {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rustc-env=WAYSCRIBER_BROKER_COHORT={hash:016x}");
}
