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

fn empty_arm() {
    match risky() {
        Err(e) => {}, // expect: SLOP005
        Ok(v) => println!("{}", v),
    }
}

fn comment_only_arm() {
    match risky() {
        Err(e) => { // expect: SLOP005
            // left blank
        },
        Ok(v) => println!("{}", v),
    }
}
