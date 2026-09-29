export const CONFIG = {
  apiUrl: process.env.API_URL || "https://api.production.com",
  apiKey: process.env.API_KEY || "",
};

export function greet(name: string): string {
  return `Hello, ${name}`;
}

export const ASSET = "/files/Leave_Your_Dog_at_Home_600x.png";
export const GREETING = "hello world";
