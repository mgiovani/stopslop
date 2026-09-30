fn add(a: i32, b: i32) -> i32 {
    a + b
}

// Example usage: // expect: SLOP051
fn total() -> i32 {
    add(1, 2)
}

// Create sample data // expect: SLOP051
fn users() -> Vec<&'static str> {
    vec!["1"]
}
