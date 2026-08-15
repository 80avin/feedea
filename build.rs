use std::path::Path;

fn main() {
    if !Path::new("frontend/dist/index.html").exists() {
        panic!(
            "frontend/dist is missing — run 'bun run build' in frontend/ first (see Makefile: make build)"
        );
    }
    println!("cargo:rerun-if-changed=frontend/dist");
}
