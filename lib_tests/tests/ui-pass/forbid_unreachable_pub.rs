#![forbid(unreachable_pub)]

use struct_path::{paths, StructPath};

#[derive(StructPath)]
pub struct TestStruct {
    pub value_str: String,
}

fn main() {
    let r: [&str; 1] = paths!(TestStruct::*);
    assert_eq!(r, ["value_str"]);
}
