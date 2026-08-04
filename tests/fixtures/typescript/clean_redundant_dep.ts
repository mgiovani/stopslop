import { formatISO } from 'date-fns';

export function formatDate(date: Date) {
  return formatISO(date, { representation: 'date' });
}

export function newId(prefix: string) {
  return `${prefix}-${crypto.randomUUID()}`;
}

export async function getJson(url: string) {
  const res = await fetch(url);
  return res.json();
}
