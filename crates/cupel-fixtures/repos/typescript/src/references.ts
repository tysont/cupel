// Reference cases for static.dead-code (ADR-135): an exported rate, one only an import reaches,
// one only a table that names it reaches, one nothing names, and code evaluated from text.

const RATES: Record<string, (amount: number) => number> = { tieredRate };

export function exportedRate(amount: number): number {
  return amount * 3;
}

export function importedRate(amount: number): number {
  return amount + amount;
}

function tieredRate(amount: number): number {
  if (amount > 100) {
    return amount / 3;
  }
  return amount;
}

function orphanRate(amount: number): number {
  return amount - amount / 4;
}

export function rateNamed(kind: string, amount: number): number {
  return RATES[kind](amount);
}

export function evaluate(source: string): number {
  return eval(source);
}

function logged(method: unknown): unknown {
  return method;
}

export class Tariff {
  @logged
  charge(amount: number): number {
    return amount + 1;
  }
}
