export const CONFIG = {
  apiUrl: process.env.API_URL || "https://api.production.com",
  apiKey: process.env.API_KEY || "",
};

export function greet(name: string): string {
  return `Hello, ${name}`;
}
