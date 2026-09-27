use struct_path::path;

pub struct TestStructParent {
    pub value_str: String,
}

fn main() {
    let _ = path!(TestStructParentTypo::value_str);
}
