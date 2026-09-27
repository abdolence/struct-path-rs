use struct_path::{path, StructPath};

#[derive(StructPath)]
struct TestStruct {
    value_str: String,
}

fn main() {
    let _ = path!(TestStruct::*);
}
