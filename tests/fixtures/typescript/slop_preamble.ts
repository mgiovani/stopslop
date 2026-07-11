// Here's the revised version of the auth middleware: // expect: SLOP002
export function auth(req) {
  return req.headers.authorization;
}
