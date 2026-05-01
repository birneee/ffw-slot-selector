use progenitor::{GenerationSettings, Generator, TagStyle};
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../openapi.yaml");

    let src = std::fs::read_to_string("../openapi.yaml").expect("openapi.yaml not found");
    let spec: openapiv3::OpenAPI = serde_yaml::from_str(&src).expect("invalid OpenAPI spec");

    let mut generator = Generator::new(
        GenerationSettings::default().with_tag(TagStyle::Merged),
    );

    let tokens = generator.generate_tokens(&spec).expect("codegen failed");
    let ast = syn::parse2(tokens).expect("generated code is not valid Rust");
    let content = prettyplease::unparse(&ast);

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("client.rs");
    std::fs::write(&out, content).unwrap();
}
