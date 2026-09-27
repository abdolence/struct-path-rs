use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct TestStructA {
    value_str: String,
}

struct TestStructB {
    value_num: u64,
}

fn main() {
    let _ = paths!(TestStructB::value_num, TestStructA::*);
}
