fn get_user(id: u32) -> String { // expect: SLOP039
    fetch_user(id)
}

fn fetch_user(id: u32) -> String {
    format!("user-{id}")
}
