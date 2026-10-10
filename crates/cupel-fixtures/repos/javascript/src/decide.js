export function classify(balance, limit) {
  if (balance < 0 && limit > 0) {
    return "overdrawn";
  } else if (balance === 0) {
    return "empty";
  }
  return balance > limit ? "rich" : "ok";
}

export function firstOverdraft(entries) {
  for (const entry of entries) {
    if (entry.balance < 0) {
      return entry;
    }
  }
  return null;
}

export function label(kind) {
  switch (kind) {
    case "deposit":
      return "money in";
    case "withdrawal":
      return "money out";
    default:
      return "unknown";
  }
}

export function parseCents(text) {
  if (!text) {
    throw new Error("empty amount");
  }
  try {
    return Math.round(Number.parseFloat(text) * 100);
  } catch (error) {
    return 0;
  }
}
