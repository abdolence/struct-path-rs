use struct_path::paths;

struct TestStruct {
    value_str: String,
}

fn main() {
    let _ = paths!(TestStruct::*);
}
