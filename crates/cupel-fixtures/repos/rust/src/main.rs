use ledger::balance;

fn main() {
    println!("{}", balance(&[1, 2, 3]));
}

#[unsafe(no_mangle)]
pub extern "C" fn ledger_version() -> u32 {
    1
}
