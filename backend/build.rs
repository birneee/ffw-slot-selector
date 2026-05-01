use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../openapi.yaml");
    println!("cargo:rerun-if-changed=migrations");

    setup_db();
    generate_openapi();
}

fn setup_db() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let db_path = out_dir.join("dev.db");
    let db_url = format!("sqlite:{}", db_path.display());

    let status = Command::new("sqlx")
        .args(["database", "create", "--database-url", &db_url])
        .status()
        .expect("sqlx not found — run inside nix develop");
    assert!(status.success(), "sqlx database create failed");

    let status = Command::new("sqlx")
        .args(["migrate", "run", "--database-url", &db_url, "--source", "migrations"])
        .status()
        .expect("sqlx not found — run inside nix develop");
    assert!(status.success(), "sqlx migrate run failed");

    println!("cargo:rustc-env=DATABASE_URL={db_url}");
}

fn generate_openapi() {
    let status = Command::new("openapi-generator-cli")
        .args([
            "generate",
            "--input-spec",
            "../openapi.yaml",
            "--generator-name",
            "rust-axum",
            "--output",
            "src/generated",
            "--global-property",
            "models,apis,supportingFiles=mod.rs:lib.rs:header.rs:types.rs:models.rs",
        ])
        .status()
        .expect("openapi-generator-cli not found — run inside nix develop");
    assert!(status.success(), "openapi-generator-cli failed");
}
