/// # Exemplos
///
/// ```
/// let itens = vec![1, 2, 3];
/// // resto da lógica fica no módulo de auth
/// let soma: i32 = itens.iter().sum();
/// assert_eq!(soma, 6);
/// ```
pub fn somar(itens: &[i32]) -> i32 {
    itens.iter().sum()
}
