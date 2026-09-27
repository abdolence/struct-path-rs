#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
pub struct ReadmeDoctests;

/// Not `#[cfg(test)]`: an integration test (`tests/*.rs`) needs this crate's
/// own name in the extern prelude to exercise `paths!(::struct_path_tests::...::*)`,
/// which a `#[cfg(test)]`-gated item would not give it.
#[derive(struct_path::StructPath)]
pub struct LeadingDoubleColonTarget {
    pub value_str: String,
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use struct_path::*;

    pub struct TestStructParent {
        pub value_str: String,
        pub value_num: u64,
        pub value_child: TestStructChild,
        pub opt_value_child: Option<TestStructChild>,
    }

    pub struct TestStructChild {
        pub child_value_str: String,
        pub child_value_num: u64,
    }

    #[derive(StructPath)]
    pub struct TestStructAllFields {
        pub value_str: String,
        pub value_num: u64,
    }

    #[derive(StructPath)]
    pub struct TestStructMixedVisibility {
        pub value_str: String,
        pub(crate) value_crate: u64,
        value_private: bool,
    }

    #[derive(StructPath)]
    pub struct TestStructOddFields {
        pub id_map: std::collections::HashMap<String, u64>,
        #[allow(dead_code)]
        pub value_num: u64,
        pub r#type: String,
    }

    #[derive(StructPath)]
    pub struct TestStructWithLifetime<'a> {
        pub value_str: &'a str,
    }

    #[derive(StructPath)]
    pub struct TestStructWithWhereClause<'a>
    where
        'a: 'static,
    {
        pub x: &'a u8,
    }

    #[derive(StructPath)]
    pub struct TestStructWhereClauseWithParens<'a>
    where
        (): Sized,
        'a: 'static,
    {
        pub x: &'a u8,
    }

    #[derive(StructPath)]
    pub struct TestStructWhereClauseWithRefParens<'a>
    where
        &'a (i32, i32): 'a,
    {
        pub x: &'a u8,
    }

    #[derive(StructPath)]
    pub struct TestStructFnPointerField {
        pub before: fn(u8) -> u8,
        pub middle: u32,
        pub after: u32,
    }

    #[derive(StructPath)]
    pub struct TestStructBoxedDynFnField {
        pub callback: Box<dyn Fn(u8) -> u8>,
        pub after: u32,
    }

    macro_rules! mk_struct_from_vis_fragments {
        ($n:ident { $($fv:vis $f:ident : $t:ty),* }) => {
            #[derive(StructPath)]
            pub struct $n { $($fv $f: $t),* }
        };
    }

    mk_struct_from_vis_fragments!(TestStructFromVisFragments { pub visible: u8, hidden: u8 });

    macro_rules! mk_struct_with_item_vis_fragment {
        ($v:vis $n:ident) => {
            #[derive(StructPath)]
            $v struct $n {
                pub value_str: String,
            }
        };
    }

    // An unmatched `$v:vis` (private) arrives at the derive as an empty
    // `Group(Delimiter::None)`, not as zero tokens; this struct's own item
    // visibility exercises that unwrapping, distinct from the field-level
    // `$fv:vis` fragments `TestStructFromVisFragments` already covers.
    mk_struct_with_item_vis_fragment!(TestStructFromItemVisFragment);

    pub mod nested {
        use struct_path::StructPath;

        #[derive(StructPath)]
        pub struct NestedStruct {
            pub value_str: String,
        }
    }

    #[test]
    fn struct_path() {
        let test_simple = path!(TestStructParent::value_str);
        assert_eq!(test_simple, "value_str");

        let test_with_child = path!(TestStructParent::value_child.child_value_str);
        assert_eq!(test_with_child, "value_child.child_value_str");

        let test_another_delim =
            path!(TestStructParent::value_child.child_value_str; delim = "/", case = "camel");

        assert_eq!(test_another_delim, "valueChild/childValueStr");

        let test_full_path = path!(crate::tests::TestStructParent::value_str);
        assert_eq!(test_full_path, "value_str");

        let test_mixed_path = path!(
            TestStructParent::value_str,
            TestStructChild::child_value_str
        );
        assert_eq!(test_mixed_path, "value_str.child_value_str");

        let test_opt_child = path!(TestStructParent::opt_value_child~child_value_str);
        assert_eq!(test_opt_child, "opt_value_child.child_value_str");

        let test_opt_child_with_delim =
            path!(TestStructParent::opt_value_child~child_value_str; delim = "/");
        assert_eq!(test_opt_child_with_delim, "opt_value_child/child_value_str");

        let test_opt_child_with_case =
            path!(TestStructParent::opt_value_child~child_value_str; case = "pascal");
        assert_eq!(test_opt_child_with_case, "OptValueChild.ChildValueStr");
    }

    #[test]
    fn struct_paths() {
        let test_multiple = paths!(TestStructParent:: { value_str, value_num } );
        assert_eq!(test_multiple, ["value_str", "value_num"]);

        let test_multiple_types = paths!(
            TestStructParent::value_str,
            TestStructChild::child_value_str
        );
        assert_eq!(test_multiple_types, ["value_str", "child_value_str"]);

        let test_multiple_types = paths!(
            TestStructParent::opt_value_child~child_value_str
        );
        assert_eq!(test_multiple_types, ["opt_value_child.child_value_str"]);
    }

    /// Locks in the plain (non-`*`) grammar's own edge cases against a
    /// parity run against master (`1d104cd`) on the same inputs: a `(...)`
    /// field group, a `[...]` field group, a `(...)` group combined with a
    /// second struct, and the option grammar's escaped delimiters, char
    /// delimiter, unquoted option value and a bare trailing key. `paths!`'s
    /// nested `Type::*` routing must never divert any of these into its own
    /// parser, since none of them contain a `*`.
    ///
    /// `#[rustfmt::skip]`: rustfmt formats a macro call's arguments as if
    /// they were a Rust expression when it can, and treats `Type::(...)`
    /// as a redundant turbofish colon before a call, deleting the `::` this
    /// macro's grammar requires there.
    #[test]
    #[rustfmt::skip]
    fn struct_paths_master_parity() {
        let test_paren_group = paths!(TestStructParent::(value_str, value_num));
        assert_eq!(test_paren_group, ["value_str", "value_num"]);

        let test_bracket_group = paths!(TestStructParent::[value_str, value_num]);
        assert_eq!(test_bracket_group, ["value_str", "value_num"]);

        let test_paren_group_plus_second_struct = paths!(
            TestStructParent::(value_str, value_num),
            TestStructChild::child_value_str
        );
        assert_eq!(
            test_paren_group_plus_second_struct,
            ["value_str", "value_num", "child_value_str"]
        );

        let test_delim_tab = path!(TestStructParent::value_child.child_value_str; delim = "\t");
        assert_eq!(test_delim_tab, "value_child\tchild_value_str");

        let test_delim_backslash =
            path!(TestStructParent::value_child.child_value_str; delim = "\\");
        assert_eq!(test_delim_backslash, "value_child\\child_value_str");

        let test_delim_quote = path!(TestStructParent::value_child.child_value_str; delim = "\"");
        assert_eq!(test_delim_quote, "value_child\"child_value_str");

        let test_delim_char = path!(TestStructParent::value_child.child_value_str; delim = '/');
        assert_eq!(test_delim_char, "value_child/child_value_str");

        let test_case_unquoted = path!(TestStructParent::value_str; case=camel);
        assert_eq!(test_case_unquoted, "valueStr");

        let test_bare_key_ignored = path!(TestStructParent::value_str; delim);
        assert_eq!(test_bare_key_ignored, "value_str");
    }

    #[test]
    fn struct_paths_all_fields() {
        let test_all = paths!(TestStructAllFields::*);
        assert_eq!(test_all, ["value_str", "value_num"]);

        let test_camel = paths!(TestStructAllFields::*; case = "camel");
        assert_eq!(test_camel, ["valueStr", "valueNum"]);

        let test_pascal = paths!(TestStructAllFields::*; case = "pascal");
        assert_eq!(test_pascal, ["ValueStr", "ValueNum"]);

        let test_default_visibility = paths!(TestStructMixedVisibility::*);
        assert_eq!(test_default_visibility, ["value_str"]);

        let test_all_visibility = paths!(TestStructMixedVisibility::*; visibility = "all");
        assert_eq!(
            test_all_visibility,
            ["value_str", "value_crate", "value_private"]
        );

        let test_odd_fields = paths!(TestStructOddFields::*);
        assert_eq!(test_odd_fields, ["id_map", "value_num", "type"]);

        let test_lifetime = paths!(TestStructWithLifetime::*);
        assert_eq!(test_lifetime, ["value_str"]);

        let test_nested_path = paths!(crate::tests::nested::NestedStruct::*);
        assert_eq!(test_nested_path, ["value_str"]);
    }

    #[test]
    fn struct_paths_all_fields_with_where_clauses() {
        let test_where = paths!(TestStructWithWhereClause::*);
        assert_eq!(test_where, ["x"]);

        let test_where_with_parens = paths!(TestStructWhereClauseWithParens::*);
        assert_eq!(test_where_with_parens, ["x"]);

        let test_where_with_ref_parens = paths!(TestStructWhereClauseWithRefParens::*);
        assert_eq!(test_where_with_ref_parens, ["x"]);
    }

    #[test]
    fn struct_paths_all_fields_past_a_return_arrow() {
        let test_fn_pointer = paths!(TestStructFnPointerField::*);
        assert_eq!(test_fn_pointer, ["before", "middle", "after"]);

        let test_boxed_dyn_fn = paths!(TestStructBoxedDynFnField::*);
        assert_eq!(test_boxed_dyn_fn, ["callback", "after"]);
    }

    #[test]
    fn struct_paths_all_fields_from_macro_rules_vis_fragments() {
        let test_pub_only = paths!(TestStructFromVisFragments::*);
        assert_eq!(test_pub_only, ["visible"]);

        let test_all = paths!(TestStructFromVisFragments::*; visibility = "all");
        assert_eq!(test_all, ["visible", "hidden"]);
    }

    // Mirrors how a downstream crate (firestore-rs) wraps `path!`/`paths!` in
    // its own `macro_rules!` to re-export them under its own name, forwarding
    // the caller's tokens untouched via `$($x:tt)*`.
    macro_rules! wrap {
        ($($x:tt)*) => { struct_path::paths!($($x)*) };
    }

    macro_rules! wrap_path {
        ($($x:tt)*) => { struct_path::path!($($x)*) };
    }

    // Mirrors a wrapper that appends its own option rather than forwarding
    // the caller's tokens untouched, such as firestore-rs's camelCase helper.
    macro_rules! camel_paths {
        ($($x:tt)*) => { struct_path::paths!($($x)*; case = "camel") };
    }

    #[test]
    fn path_and_paths_through_a_macro_rules_wrapper() {
        // Plain `path!`/`paths!`: `.`, `~`, multiple structs, `{}` groups, options.
        assert_eq!(
            wrap_path!(TestStructParent::value_child.child_value_str),
            "value_child.child_value_str"
        );
        assert_eq!(
            wrap_path!(TestStructParent::opt_value_child~child_value_str),
            "opt_value_child.child_value_str"
        );
        assert_eq!(
            wrap_path!(
                TestStructParent::value_str,
                TestStructChild::child_value_str
            ),
            "value_str.child_value_str"
        );
        assert_eq!(
            wrap!(TestStructParent::{ value_str, value_num }),
            ["value_str", "value_num"]
        );
        assert_eq!(
            wrap_path!(TestStructParent::value_child.child_value_str; delim = "/", case = "camel"),
            "valueChild/childValueStr"
        );

        // Bare `Type::*`.
        assert_eq!(wrap!(TestStructAllFields::*), ["value_str", "value_num"]);

        // Nested `.(Child::*)` / `~(Child::*)`, with and without options.
        assert_eq!(
            wrap!(StarParent::child.(StarChild::*)),
            ["child.a", "child.b"]
        );
        assert_eq!(
            wrap!(StarParent::opt_child~(StarChild::*)),
            ["opt_child.a", "opt_child.b"]
        );
        assert_eq!(
            wrap!(StarParent::child.(StarChild::*); case = "pascal"),
            ["Child.A", "Child.B"]
        );
        assert_eq!(
            wrap!(StarParent::opt_child~(StarChild::*); case = "camel", delim = "/"),
            ["optChild/a", "optChild/b"]
        );

        // A wrapper that appends its own options rather than forwarding the
        // caller's tokens untouched, for both a field list and `Type::*`.
        assert_eq!(
            camel_paths!(TestStructParent::{ value_str, value_num }),
            ["valueStr", "valueNum"]
        );
        assert_eq!(camel_paths!(StarChild::*), ["a", "b"]);

        // The wrapper appends its own group after one the caller already
        // supplied, rather than being the caller's only group.
        assert_eq!(
            camel_paths!(TestStructMixedVisibility::*; visibility = "all"),
            ["valueStr", "valueCrate", "valuePrivate"]
        );
    }

    #[test]
    fn camel_wrapper_appends_after_caller_options_past_forty_dotted_paths() {
        // A wrapper that appends its own options group must still accept a
        // long list of dotted paths from the caller.
        let result = camel_paths!(
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str,
            TestStructParent::value_child.child_value_str;
            delim = "/"
        );
        assert_eq!(result.len(), 45);
        assert!(result.iter().all(|p| *p == "valueChild/childValueStr"));
    }

    #[test]
    fn repeated_option_groups() {
        // `path!`: which group holds which key does not matter for merging.
        assert_eq!(
            path!(TestStructParent::value_child.child_value_str; case = "camel"; delim = "/"),
            "valueChild/childValueStr"
        );

        // `paths!` with a `{}` field group.
        assert_eq!(
            paths!(TestStructParent::{ value_child.child_value_str }; delim = "/"; case = "camel"),
            ["valueChild/childValueStr"]
        );

        // Strict `Type::*`.
        assert_eq!(
            paths!(TestStructMixedVisibility::*; visibility = "all"; case = "camel"),
            ["valueStr", "valueCrate", "valuePrivate"]
        );

        // Nested `Type::*`, split across two groups instead of one.
        assert_eq!(
            paths!(StarParent::opt_child~(StarChild::*); delim = "/"; case = "camel"),
            ["optChild/a", "optChild/b"]
        );

        // Empty groups: a trailing `;` and `;;`, for the lenient and the
        // strict/nested forms alike.
        assert_eq!(path!(TestStructParent::value_str;;), "value_str");
        assert_eq!(paths!(TestStructAllFields::*;;), ["value_str", "value_num"]);
        assert_eq!(
            paths!(StarParent::child.(StarChild::*);;),
            ["child.a", "child.b"]
        );
    }

    #[test]
    fn struct_paths_all_fields_from_item_vis_fragment() {
        let test_pub_only = paths!(TestStructFromItemVisFragment::*);
        assert_eq!(test_pub_only, ["value_str"]);

        let test_all = paths!(TestStructFromItemVisFragment::*; visibility = "all");
        assert_eq!(test_all, ["value_str"]);
    }

    #[derive(StructPath)]
    pub struct StarChild {
        pub a: String,
        pub b: u64,
    }

    #[derive(StructPath)]
    pub struct StarChildMixedVisibility {
        pub a: String,
        pub(crate) b: u64,
    }

    pub struct StarMiddle {
        pub child: StarChild,
    }

    pub mod star_nested {
        use struct_path::StructPath;

        #[derive(StructPath)]
        pub struct StarNestedChild {
            pub y: String,
        }
    }

    pub struct StarParent {
        pub child: StarChild,
        pub middle: StarMiddle,
        pub opt_child: Option<StarChild>,
        pub vec_child: Vec<StarChild>,
        pub boxed_child: Box<StarChild>,
        pub mixed_child: StarChildMixedVisibility,
        pub mod_child: star_nested::StarNestedChild,
    }

    #[test]
    fn struct_paths_nested_all_fields() {
        let test_direct = paths!(StarParent::child.(StarChild::*));
        assert_eq!(test_direct, ["child.a", "child.b"]);

        let test_two_level = paths!(StarParent::middle.child.(StarChild::*));
        assert_eq!(test_two_level, ["middle.child.a", "middle.child.b"]);

        let test_option = paths!(StarParent::opt_child~(StarChild::*));
        assert_eq!(test_option, ["opt_child.a", "opt_child.b"]);

        let test_vec = paths!(StarParent::vec_child~(StarChild::*));
        assert_eq!(test_vec, ["vec_child.a", "vec_child.b"]);

        // `Box<StarChild>` passes the `&StarChild` check through deref
        // coercion; no `~` is needed the way `Option`/`Vec` need one.
        let test_boxed = paths!(StarParent::boxed_child.(StarChild::*));
        assert_eq!(test_boxed, ["boxed_child.a", "boxed_child.b"]);

        let test_case_delim =
            paths!(StarParent::opt_child~(StarChild::*); case = "camel", delim = "/");
        assert_eq!(test_case_delim, ["optChild/a", "optChild/b"]);

        let test_pascal = paths!(StarParent::child.(StarChild::*); case = "pascal");
        assert_eq!(test_pascal, ["Child.A", "Child.B"]);

        // An escaped delimiter must decode to the same character through
        // the nested form as it already does through the flat form: a
        // literal `\t` in the source, not a backslash and a `t`.
        let test_delim_tab = paths!(StarParent::child.(StarChild::*); delim = "\t");
        assert_eq!(
            test_delim_tab,
            [
                path!(StarParent::child.a; delim = "\t"),
                path!(StarParent::child.b; delim = "\t"),
            ]
        );
        assert_eq!(test_delim_tab, ["child\ta", "child\tb"]);

        let test_default_visibility = paths!(StarParent::mixed_child.(StarChildMixedVisibility::*));
        assert_eq!(test_default_visibility, ["mixed_child.a"]);

        let test_all_visibility =
            paths!(StarParent::mixed_child.(StarChildMixedVisibility::*); visibility = "all");
        assert_eq!(test_all_visibility, ["mixed_child.a", "mixed_child.b"]);

        let test_module_path =
            paths!(StarParent::mod_child.(crate::tests::star_nested::StarNestedChild::*));
        assert_eq!(test_module_path, ["mod_child.y"]);

        // A trailing `;` with no option pairs after it is accepted the same
        // way for the nested form as it already is for the bare form.
        let test_empty_options_bare = paths!(StarChild::*;);
        assert_eq!(test_empty_options_bare, ["a", "b"]);
        let test_empty_options_nested = paths!(StarParent::child.(StarChild::*););
        assert_eq!(test_empty_options_nested, ["child.a", "child.b"]);

        const CONST_NAMES: [&str; 2] = paths!(StarParent::child.(StarChild::*));
        assert_eq!(CONST_NAMES, ["child.a", "child.b"]);
    }
}
