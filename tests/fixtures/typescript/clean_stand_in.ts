export function close(socket: { end(): void }) {
  // thereby releasing the file descriptor, as the logician's proof requires
  socket.end();
}

/** Logic goes here when the feature flag is set. */
export function gated() {
  return 1;
}
