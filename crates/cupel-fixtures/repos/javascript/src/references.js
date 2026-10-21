// Reference cases for static.dead-code (ADR-135): an exported rate, one only an import reaches,
// one only a table that names it reaches, one nothing names, and code evaluated from text.

const RATES = { tieredRate };

export function exportedRate(amount) {
  return amount * 3;
}

export function importedRate(amount) {
  return amount + amount;
}

function tieredRate(amount) {
  if (amount > 100) {
    return amount / 3;
  }
  return amount;
}

function orphanRate(amount) {
  return amount - amount / 4;
}

export function rateNamed(kind, amount) {
  return RATES[kind](amount);
}

export function evaluate(source) {
  return eval(source);
}

function logged(method) {
  return method;
}

export class Tariff {
  @logged
  charge(amount) {
    return amount + 1;
  }
}
