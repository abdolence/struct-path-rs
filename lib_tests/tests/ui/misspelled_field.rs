use struct_path::path;

pub struct TestStructParent {
    pub value_child: TestStructChild,
}

pub struct TestStructChild {
    pub child_value_str: String,
}

fn main() {
    let _ = path!(TestStructParent::value_child.child_value_sr);
}
