fn record_hit(counter: &mut i32) {
    // increment the counter
    *counter += 1;
}

fn record_batch(counter: &mut i32, n: i32) {
    *counter += n; // increment the counter
}

fn main() {
    let mut hits = 0;
    record_hit(&mut hits);
    record_batch(&mut hits, 3);
}

// expect-line: 2 SLOP042
// expect-line: 7 SLOP042
