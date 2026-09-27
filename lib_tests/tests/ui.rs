// Malformed `path!`/`paths!` input must fail with a normal `compile_error!`
// rather than a "proc macro panicked" build failure.
#[test]
fn malformed_macro_input_reports_compile_error() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
