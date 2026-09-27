#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
pub struct ReadmeDoctests;

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
}
