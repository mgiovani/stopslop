import moment from 'moment'; // expect: SLOP038

export function Timestamp({ date }: { date: Date }) {
  return <span>{moment(date).format('YYYY-MM-DD')}</span>;
}
