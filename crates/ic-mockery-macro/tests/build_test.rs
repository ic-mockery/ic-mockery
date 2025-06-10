#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    // these should compile
    t.pass("tests/components/basic.rs");
}
