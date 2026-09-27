use struct_path::path;

pub struct TestStructParent {
    pub value_num: u64,
}

fn main() {
    let _ = path!(TestStructParent::value_num~child_value_str);
}
