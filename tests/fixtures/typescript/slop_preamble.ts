// Here's the revised version of the auth middleware: // expect: SLOP002
export function auth(req) {
  return req.headers.authorization;
}

// Let's think about this differently before touching the retry logic. // expect: SLOP002
export function retry(fn) {
  return fn();
}
