export function discounted(price, member) {
  const cut = (price * 15) / 100;
  if (member && cut !== 0) {
    return price - cut;
  }
  return price;
}

export function tally(entries) {
  let total = 0;
  let overdrawn = false;
  for (const entry of entries) {
    total += entry;
    if (total < -1) {
      overdrawn = true;
    }
  }
  logTotal(total);
  return { total, overdrawn };
}

function logTotal(total) {
  console.log(total == null ? "none" : total);
}
