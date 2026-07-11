trait Handler {
    fn process(&self, data: &[u8]) -> String;
}

fn real(data: &[u8]) -> String {
    String::from_utf8_lossy(data).to_string()
}

#[cfg(test)]
mod tests {
    fn placeholder() {
        todo!()
    }
}
