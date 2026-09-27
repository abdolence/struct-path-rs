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

        let test_default_visibility = paths!(StarParent::mixed_child.(StarChildMixedVisibility::*));
        assert_eq!(test_default_visibility, ["mixed_child.a"]);

        let test_all_visibility =
            paths!(StarParent::mixed_child.(StarChildMixedVisibility::*); visibility = "all");
        assert_eq!(test_all_visibility, ["mixed_child.a", "mixed_child.b"]);

        let test_module_path =
            paths!(StarParent::mod_child.(crate::tests::star_nested::StarNestedChild::*));
        assert_eq!(test_module_path, ["mod_child.y"]);

        const CONST_NAMES: [&str; 2] = paths!(StarParent::child.(StarChild::*));
        assert_eq!(CONST_NAMES, ["child.a", "child.b"]);
    }
}
