use struct_path::paths;

// `::` at the start of a type path names a crate in the extern prelude
// (`::struct_path_tests`), not a combination with another struct or field
// group; only an integration test crate has this crate under its own name
// to exercise that.
#[test]
fn leading_double_colon_resolves_the_type() {
    let result = paths!(::struct_path_tests::LeadingDoubleColonTarget::*);
    assert_eq!(result, ["value_str"]);
}
