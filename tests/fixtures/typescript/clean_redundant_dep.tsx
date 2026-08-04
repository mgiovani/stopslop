export function Timestamp({ date }: { date: Date }) {
  return <span>{new Intl.DateTimeFormat().format(date)}</span>;
}
