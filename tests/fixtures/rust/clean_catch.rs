use std::fmt;

#[derive(Debug)]
struct MyError;

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "boom")
    }
}

fn risky() -> Result<i32, MyError> {
    Err(MyError)
}

fn log_and_return() -> i32 {
    match risky() {
        Err(e) => {
            eprintln!("failed: {}", e);
            -1
        }
        Ok(v) => v,
    }
}

fn intent_arm() {
    match risky() {
        Err(e) => {
            // intentional: best-effort operation, ignore failure
        }
        Ok(v) => println!("{}", v),
    }
}

fn other_pattern_empty() {
    let x: Option<i32> = None;
    match x {
        None => {}
        Some(v) => println!("{}", v),
    }
}
