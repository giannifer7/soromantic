//! Test to export TypeScript bindings
//!
//! Run with: cargo test -p soromantic-core export_bindings -- --nocapture

#[test]
fn export_bindings() {
    // ts-rs automatically exports types with #[ts(export)] when tests run
    // The bindings are written to the path specified in #[ts(export_to = "...")]
    println!("TypeScript bindings exported to core/bindings/");
}
