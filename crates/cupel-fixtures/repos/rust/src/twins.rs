//! Pairs of functions that differ in one way, for structural-hash conformance.

pub fn net_total(items: &[i64], fee: i64) -> i64 {
    let gross: i64 = items.iter().sum();
    if gross > fee { gross - fee } else { 0 }
}

pub fn sum_after_charge(values: &[i64], charge: i64) -> i64 {
    let subtotal: i64 = values.iter().sum();
    if subtotal > charge { subtotal - charge } else { 0 }
}

pub fn net_total_floor(items: &[i64], fee: i64) -> i64 {
    let gross: i64 = items.iter().sum();
    if gross > fee { gross - fee } else { 100 }
}

pub fn net_total_laid_out(items: &[i64], fee: i64) -> i64 {
    // The same body as `net_total`, laid out differently.
    let gross: i64 = items
        .iter()
        .sum();
    if gross > fee {
        gross - fee
    } else {
        0
    }
}

pub fn net_total_plus(items: &[i64], fee: i64) -> i64 {
    let gross: i64 = items.iter().sum();
    if gross > fee { gross + fee } else { 0 }
}

pub fn net_total_capped(items: &[i64], fee: i64) -> i64 {
    let gross: i64 = items.iter().sum();
    if gross > fee {
        gross - fee
    } else if gross < 0 {
        gross
    } else {
        0
    }
}

pub fn net_total_product(items: &[i64], fee: i64) -> i64 {
    let gross: i64 = items.iter().product();
    if gross > fee { gross - fee } else { 0 }
}
