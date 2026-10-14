// Ledger helpers of varying shape: each of the first four trips one complexity limit.

export function label(amount: number, limit: number): string {
  let name;
  if (amount < 0) {
    name = "refund";
  } else if (amount === 0) {
    name = "empty";
  } else if (amount > limit * 100) {
    name = "huge";
  } else if (amount > limit * 10) {
    name = "large";
  } else if (amount > limit) {
    name = "over";
  } else if (amount === limit) {
    name = "at-limit";
  } else if (amount > limit / 2) {
    name = "high";
  } else if (amount > limit / 4) {
    name = "medium";
  } else if (amount > limit / 10) {
    name = "low";
  } else if (amount > 1) {
    name = "tiny";
  } else {
    name = "unit";
  }
  return name;
}

export function settle(batches: number[][], limit: number): number {
  let settled = 0;
  for (const batch of batches) {
    if (batch.length > 0) {
      for (const amount of batch) {
        if (amount > 0) {
          while (settled < limit) {
            settled += amount;
          }
        }
      }
    }
  }
  return settled;
}

export function openAccount(
  owner: string,
  currency: string,
  limit: number,
  overdraft: boolean,
  branch: number,
  opened: number,
): string {
  return `${owner}/${currency}/${limit}/${overdraft}/${branch}/${opened}`;
}

export function statementTemplate(): string[] {
  return [
    "ACME LEDGER",
    "===========",
    "",
    "Account holder:",
    "Account number:",
    "Branch:",
    "Currency:",
    "Statement period:",
    "",
    "Opening balance",
    "---------------",
    "Balance brought forward:",
    "Pending credits:",
    "Pending debits:",
    "",
    "Credits",
    "-------",
    "Deposits:",
    "Transfers in:",
    "Interest:",
    "Refunds:",
    "Adjustments:",
    "",
    "Debits",
    "------",
    "Withdrawals:",
    "Transfers out:",
    "Card payments:",
    "Direct debits:",
    "Standing orders:",
    "Fees:",
    "Charges:",
    "",
    "Closing balance",
    "---------------",
    "Balance carried forward:",
    "Available balance:",
    "Overdraft limit:",
    "Overdraft used:",
    "",
    "Notices",
    "-------",
    "Rates changed this period.",
    "Fees are listed above.",
    "Keep this statement for your records.",
    "Report errors within thirty days.",
    "",
    "Contact",
    "-------",
    "Telephone:",
    "Email:",
    "Post:",
    "",
    "Opening hours",
    "-------------",
    "Weekdays: 9 to 5",
    "Saturday: 9 to 12",
    "Sunday: closed",
    "",
    "Page 1 of 1",
    "End of statement",
  ];
}

export function net(credits: number, debits: number): number {
  return credits - debits;
}

export class Statement {
  owner: string;

  constructor(owner: string) {
    this.owner = owner;
  }

  line(amount: number): string {
    return `${this.owner}: ${amount}`;
  }
}

// Names a month by its number: one switch, however many cases it has.
export function month(number: number): string {
  switch (number) {
    case 1:
      return "jan";
    case 2:
      return "feb";
    case 3:
      return "mar";
    case 4:
      return "apr";
    case 5:
      return "may";
    case 6:
      return "jun";
    case 7:
      return "jul";
    case 8:
      return "aug";
    case 9:
      return "sep";
    case 10:
      return "oct";
    case 11:
      return "nov";
    case 12:
      return "dec";
    default:
      return "none";
  }
}
