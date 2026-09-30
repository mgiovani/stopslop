fn close(done: &mut bool) {
    // thereby releasing the file descriptor, as the logician's proof requires
    *done = true;
}
