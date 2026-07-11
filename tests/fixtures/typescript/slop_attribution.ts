// Written by Copilot // expect: SLOP004
export function auth(req) {
  return !!req.headers.auth;
}
