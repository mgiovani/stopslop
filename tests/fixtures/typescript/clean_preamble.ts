/**
 * Here's the main API handler.
 * Sure! This processes requests.
 */
export function handler(req) {
  return req.body;
}

// The first step here validates the payload before it reaches the handler.
export function validate(payload) {
  return payload != null && typeof payload === "object" && "id" in payload;
}
