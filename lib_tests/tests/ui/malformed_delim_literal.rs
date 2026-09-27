use struct_path::path;

struct TestStruct {
    value_str: String,
}

fn main() {
    let _ = path!(TestStruct::value_str; delim = 5);
}
