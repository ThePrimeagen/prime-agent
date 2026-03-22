//! Rolling buffer of the last N complete lines of stdout.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct StdoutTail {
    cap: usize,
    lines: VecDeque<String>,
}

impl StdoutTail {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            lines: VecDeque::new(),
        }
    }

    pub fn push_line(&mut self, line: String) {
        if self.lines.len() >= self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn lines(&self) -> impl Iterator<Item = &str> + '_ {
        self.lines.iter().map(String::as_str)
    }

    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_empty() {
        let t = StdoutTail::new(3);
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn tail_exact_capacity() {
        let mut t = StdoutTail::new(3);
        t.push_line("a".to_string());
        t.push_line("b".to_string());
        t.push_line("c".to_string());
        let v: Vec<_> = t.lines().collect();
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn tail_drops_oldest_when_exceeds() {
        let mut t = StdoutTail::new(3);
        for i in 0..10 {
            t.push_line(format!("line{i}"));
        }
        let v: Vec<_> = t.lines().collect();
        assert_eq!(v, vec!["line7", "line8", "line9"]);
    }

    #[test]
    fn tail_cap_one() {
        let mut t = StdoutTail::new(1);
        t.push_line("x".to_string());
        t.push_line("y".to_string());
        let v: Vec<_> = t.lines().collect();
        assert_eq!(v, vec!["y"]);
    }
}
