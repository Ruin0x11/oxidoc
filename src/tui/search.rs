use tui::score::{self, Match};
use tui::sorted_result_set::SortedResultSet;
use std::ascii::AsciiExt;

#[derive(Debug)]
pub struct Search<'s> {
    pub query: String,
    pub current: usize,
    pub result: Vec<Match<'s>>,
    choice_stack: ChoiceStack<'s>,
    pub visible_limit: usize,
    done: bool,
}

#[derive(Debug)]
struct ChoiceStack<'s> {
    content: Vec<Vec<&'s String>>,
}

impl <'s>ChoiceStack<'s> {
    pub fn new(input: &'s Vec<String>) -> ChoiceStack<'s> {
        let initial_choices = input.iter().map(|x| x).collect();

        ChoiceStack { content: vec![initial_choices] }
    }

    pub fn push(&mut self, frame: Vec<&'s String>) {
        self.content.push(frame);
    }

    pub fn pop(&mut self) {
        if self.content.len() > 1 {
            self.content.pop();
        }
    }

    pub fn peek(&self) -> &Vec<&'s String> {
        self.content.last().unwrap()
    }

    pub fn last_size(&self) -> usize {
        self.peek().len()
    }
}

impl<'s> Search<'s> {
    pub fn blank(choices: &'s Vec<String>,
                 initial_search: Option<String>,
                 visible_limit: usize) -> Search<'s> {
        let query = initial_search.unwrap_or("".to_string());

        let choice_stack = ChoiceStack::new(&choices);

        let result = choices.iter().take(visible_limit).enumerate().map(|(i, x)| Match::with_empty_range(x,i)).collect();

        Search::new(query, choice_stack, result, 0, visible_limit, false)
    }

    fn new(query: String, choice_stack: ChoiceStack<'s>, result: Vec<Match<'s>>, index: usize, visible_limit: usize, done: bool) -> Search<'s> {
        Search { current: index,
                 query: query,
                 result: result,
                 choice_stack: choice_stack,
                 visible_limit: visible_limit,
                 done: done}
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn done(self) -> Search<'s> {
        Search::new(self.query, self.choice_stack, self.result, self.current, self.visible_limit, true)
    }

    pub fn selection(&self) -> Option<String> {
        self.result.get(self.current).map( |t| t.original.clone())
    }

    fn new_for_index(self, index: usize) -> Search<'s> {
        Search::new(self.query, self.choice_stack, self.result, index,self.visible_limit, self.done)
    }

    pub fn iter_matches<F: FnMut(Match<'s>)>(query: &str, choices: &Vec<&'s String>, mut f: F) {
        let lower_query = query.to_ascii_lowercase();

        for (idx, choice) in choices.iter().enumerate() {
            match score::score(&choice, &lower_query, idx) {
                None     => continue,
                Some(m) => f(m),
            };
        }
    }

    pub fn down(self) -> Search<'s> {
        let next_index = self.next_index();
        self.new_for_index(next_index)
    }

    pub fn up(self) -> Search<'s> {
        let next_index = self.prev_index();
        self.new_for_index(next_index)
    }

    pub fn append_to_search(mut self, input: &str) -> Search<'s> {
        let mut new_query = self.query.clone();
        new_query.push_str(input.as_ref());

        let mut result = SortedResultSet::new(self.visible_limit);
        let mut filtered_choices: Vec<&String> = Vec::new();
        Search::iter_matches(new_query.as_ref(), &self.choice_stack.peek(),
                        |matching| {
                                               let quality = matching.quality.to_f32();
                                               let choice = matching.original;
                                               result.push(matching.clone(), quality);
                                               filtered_choices.push(&choice)
                                             });

        self.choice_stack.push(filtered_choices);

        Search::new(new_query, self.choice_stack, result.as_sorted_vec(), 0, self.visible_limit, self.done)
    }

    pub fn backspace(mut self) -> Search<'s> {
        let mut new_query = self.query.clone();
        new_query.pop();

        self.choice_stack.pop();

        let mut result = SortedResultSet::new(self.visible_limit);
        Search::iter_matches(new_query.as_ref(), &self.choice_stack.peek(),
                             |matching| {
                                 let quality = matching.quality.to_f32();
                                 result.push(matching, quality)
                             } );

        Search::new(new_query, self.choice_stack, result.as_sorted_vec(), 0, self.visible_limit, self.done)
    }

    fn next_index(&self) -> usize {
        // TODO: fix this!
        // current is an index -> zero based
        // num_matches is a length -> 1 based
        if self.current + 1 == self.num_matches() {
            0
        } else {
            self.current+1
        }
    }

    fn prev_index(&self) -> usize {
        if self.num_matches() == 0 {
            0
        } else if self.current == 0 {
            self.num_matches() - 1
        } else {
            self.current-1
        }
    }

    pub fn num_matches(&self) -> usize {
        self.choice_stack.last_size()
    }
}
