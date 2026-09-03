// Claro! Aqui está a versão atualizada da função de login: // expect: SLOP002
fn login(user: &str) -> bool {
    let ok = autenticar(user);
    ok
}

fn autenticar(user: &str) -> bool {
    !user.is_empty()
}
