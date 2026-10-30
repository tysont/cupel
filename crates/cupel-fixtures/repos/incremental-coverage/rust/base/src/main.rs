//! The ledger's command line: each line of the file it is given holds a value and a factor,
//! and it prints their product, one per line.

fn main() {
    let path = std::env::args().nth(1).expect("a file of value-factor pairs");
    let text = std::fs::read_to_string(path).expect("the file reads");
    for line in text.lines() {
        let mut words = line.split_whitespace().map(|word| word.parse::<i64>().unwrap());
        let (value, factor) = (words.next().unwrap(), words.next().unwrap());
        println!("{}", ledger::scale(value, factor));
    }
}
