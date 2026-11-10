//! Writes the test sample XDV file used by the Java test suite.
//!
//! Usage: `gen-sample <output-path>`

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: gen-sample <output-path>");
    let bytes = xdvjni::sample::generate_sample();
    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).expect("create output directory");
    }
    std::fs::write(&out, &bytes).expect("write sample file");
    eprintln!("wrote {} bytes to {}", bytes.len(), out);
}
