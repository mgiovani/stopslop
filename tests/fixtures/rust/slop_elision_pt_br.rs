fn processar_dados(entrada: Vec<u8>) -> Result<String, ()> {
    validar(&entrada)?;
    // ... resto do código sem alteração // expect: SLOP001
    Ok(String::new())
}

fn validar(_entrada: &[u8]) -> Result<(), ()> {
    Ok(())
}
