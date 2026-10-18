use std::fs::File;
use std::io::Write;

/// A counter that logs each step it takes when given a trace file.
pub struct Counter {
    count: u32,
    done: bool,
    trace: Option<File>,
}

impl Counter {
    pub fn new(trace: Option<File>) -> Self {
        Self::starting_at(0, trace)
    }

    pub fn starting_at(count: u32, trace: Option<File>) -> Self {
        let mut counter = Self {
            count,
            done: false,
            trace,
        };
        counter.log(None, line!());
        counter
    }

    pub fn inc(&mut self) {
        self.count += 1;
        self.log(Some("Inc"), line!());
    }

    pub fn bump(&mut self) {
        self.count += 2;
        self.log(Some("Inc"), line!());
    }

    pub fn finish(&mut self) {
        self.done = true;
        self.log(Some("Finish"), line!());
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    fn log(&mut self, action: Option<&str>, line: u32) {
        let (count, done) = (self.count, self.done);
        if let Some(trace) = self.trace.as_mut() {
            let action = action.map_or("null".to_owned(), |action| format!("\"{action}\""));
            writeln!(
                trace,
                "{{\"action\": {action}, \"state\": {{\"count\": {count}, \"done\": {done}}}, \
                 \"file\": \"{}\", \"line\": {line}}}",
                file!()
            )
            .unwrap();
        }
    }
}
