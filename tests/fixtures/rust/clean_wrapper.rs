fn get_user(id: u32) -> String {
    println!("fetching {id}");
    fetch_user(id)
}

fn fetch_user(id: u32) -> String {
    format!("user-{id}")
}
