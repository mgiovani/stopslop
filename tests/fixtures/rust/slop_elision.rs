fn process_data(input: Vec<u8>) -> Result<String, ()> {
    validate(&input)?;
    // ... rest of function // expect: SLOP001
    Ok(String::new())
}

fn validate(_input: &[u8]) -> Result<(), ()> {
    Ok(())
}
