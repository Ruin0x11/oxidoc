use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::clone::Clone;

pub struct SortedResultSet<T:Clone> {
    results: BinaryHeap<ScoreResult<T>>,
    size: usize,
}

impl<T:Clone> SortedResultSet<T> {
    pub fn new(size: usize) -> SortedResultSet<T> {
        SortedResultSet {
            results:  BinaryHeap::with_capacity(size + 1),
            size:     size}
    }

    pub fn push(&mut self, choice: T, quality: f32) {
        let result = ScoreResult { quality: quality, choice: choice};

        if self.is_full() {
            self.push_pop(result);
        } else {
            self.results.push(result);
        }
    }

    fn is_full(&self) -> bool {
        self.results.len() >= self.size
    }

    pub fn as_sorted_vec(self) -> Vec<T> {
        self.results.into_sorted_vec().iter().map(|score_result| score_result.choice.clone()).collect()
    }

    fn push_pop(&mut self, result: ScoreResult<T>) {
        self.results.pop();
        self.results.push(result);
    }
}


pub struct ScoreResult<T> {
    pub quality: f32,
    pub choice: T,
}

impl<T> Ord for ScoreResult<T> {
    fn cmp(&self, other: &ScoreResult<T>) -> Ordering {
        // Reverses ordering to make the binary max heap a min heap in Search::filter.
        self.quality.partial_cmp(&other.quality).unwrap_or(Ordering::Equal).reverse()
    }
}

impl<T> PartialOrd for ScoreResult<T> {
    fn partial_cmp(&self, other: &ScoreResult<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Eq for ScoreResult<T> {
}

impl<T> PartialEq for ScoreResult<T> {
    fn eq(&self, other: &ScoreResult<T>) -> bool {
       self.quality == other.quality
    }
}
