use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct TestStruct {
    value_str: String,
}

fn main() {
    let _ = paths!(TestStruct::*; delim);
}
