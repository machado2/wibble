#[test]
fn all_templates_parse_successfully() {
    let tera = tera::Tera::new("templates/**/*");
    match tera {
        Ok(_) => {}
        Err(e) => panic!("Templates failed to parse:\n{}", e),
    }
}
