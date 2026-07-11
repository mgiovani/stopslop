pub fn is_test_path(display_path: &str) -> bool {
    let p = display_path.replace('\\', "/");
    const DIRS: &[&str] = &[
        "tests",
        "test",
        "__tests__",
        "testdata",
        "fixtures",
        "fixture",
        "mocks",
        "mock",
        "examples",
        "example",
        "vendor",
        "node_modules",
        "generated",
    ];
    if p.split('/').any(|seg| DIRS.contains(&seg)) {
        return true;
    }
    let name = p.rsplit('/').next().unwrap_or("");
    name.starts_with("test_")
        || name == "conftest.py"
        || name.ends_with("_test.go")
        || name.ends_with("_test.py")
        || name.ends_with(".test.ts")
        || name.ends_with(".test.tsx")
        || name.ends_with(".spec.ts")
        || name.ends_with(".spec.tsx")
        || name.ends_with(".pyi")
        || name.ends_with(".pb.go")
        || name.ends_with("_pb2.py")
        || name.ends_with(".min.js")
}
