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
}
