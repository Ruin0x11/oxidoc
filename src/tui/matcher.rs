use regex::{self, Regex};

pub struct Matcher<'a> {
    pub lines: &'a Vec<String>,
    pub matches: Vec<&'a String>,
    pub input: String,
}

impl<'a> Matcher<'a> {
    pub fn new(lines: &'a Vec<String>) -> Matcher<'a> {
        let mut matches: Vec<&'a String> = Vec::new();

        for line in lines {
            matches.push(&line);
        }

        Matcher {
            lines: lines,
            input: String::new(),
            matches: matches,
        }
    }

    pub fn input(&mut self, input: String) {
        self.input = input;
        self.update_matches();
    }

    fn update_matches(&mut self) {
        self.matches.clear();

        let regex_string: String = self.input.split("").fold(String::new(), |acc, input| {
            acc + ".*" + &regex::escape(&input)
        });

        let re = Regex::new(&regex_string).unwrap();

        for line in self.lines {
            if re.is_match(&line) {
                self.matches.push(&line);
            }
        }
    }
}
