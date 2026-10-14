export interface Entry {
  balance: number;
}

export function classify(balance: number, limit: number): string {
  if (balance < 0 && limit > 0) {
    return "overdrawn";
  } else if (balance === 0) {
    return "empty";
  }
  return balance > limit ? "rich" : "ok";
}

export function firstOverdraft(entries: Entry[]): Entry | null {
  for (const entry of entries) {
    if (entry.balance < 0) {
      return entry;
    }
  }
  return null;
}

export function label(kind: "deposit" | "withdrawal" | string): string {
  switch (kind) {
    case "deposit":
      return "money in";
    case "withdrawal":
      return "money out";
    default:
      return "unknown";
  }
}

export function parseCents(text: string | undefined): number {
  if (!text) {
    throw new Error("empty amount");
  }
  try {
    return Math.round(Number.parseFloat(text) * 100);
  } catch (error: unknown) {
    return 0;
  }
}
