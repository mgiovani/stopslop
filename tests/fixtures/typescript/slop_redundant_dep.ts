import moment from 'moment'; // expect: SLOP038
import { v4 as uuidv4 } from 'uuid'; // expect: SLOP038
const fetchPolyfill = require('node-fetch'); // expect: SLOP038

export function formatDate(date: Date) {
  return moment(date).format('YYYY-MM-DD');
}

export function newId(prefix: string) {
  return `${prefix}-${uuidv4()}`;
}

export function getJson(url: string) {
  console.log('fetching', url);
  return fetchPolyfill(url);
}
