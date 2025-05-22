fn main() {
    cynic_codegen::register_schema("zkverify")
        .from_sdl_file("schemas/zkverify.graphql")
        .unwrap()
        .as_default()
        .unwrap();
}
