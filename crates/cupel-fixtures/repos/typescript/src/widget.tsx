export function Badge({ label, count }: { label: string; count: number }) {
  return <span className="badge">{count > 0 ? `${label} (${count})` : label}</span>;
}
