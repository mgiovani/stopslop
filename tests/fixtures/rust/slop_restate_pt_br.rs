fn registrar_hit(contador: &mut i32) {
    // incrementa o contador
    *contador += 1;
}

fn registrar_lote(contador: &mut i32, n: i32) {
    *contador += n; // incrementa o contador
}

fn main() {
    let mut acessos = 0;
    registrar_hit(&mut acessos);
    registrar_lote(&mut acessos, 3);
}

// expect-line: 2 SLOP042
// expect-line: 7 SLOP042
