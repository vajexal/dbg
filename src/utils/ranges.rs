use std::cmp::Ordering;

use super::avl::AVLTree;

#[derive(Debug)]
struct Range<T> {
    start: u64,
    end: u64,
    value: T,
}

impl<T> PartialOrd for Range<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.start > other.end {
            return Some(Ordering::Greater);
        }

        if self.end < other.start {
            return Some(Ordering::Less);
        }

        None
    }
}

impl<T> PartialEq for Range<T> {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end == other.end
    }
}

#[derive(Debug)]
pub struct Ranges<T> {
    tree: AVLTree<Range<T>>,
}

impl<T> Ranges<T> {
    pub fn new() -> Self {
        Self { tree: AVLTree::new() }
    }

    pub fn add(&mut self, start: u64, end: u64, value: T) {
        self.tree.insert(Range { start, end, value });
    }

    pub fn find_value(&self, pos: u64) -> Option<&T> {
        self.find_range_ref(pos).map(|range| &range.value)
    }

    pub fn find_range(&self, pos: u64) -> Option<(u64, u64)> {
        self.find_range_ref(pos).map(|range| (range.start, range.end))
    }

    fn find_range_ref(&self, pos: u64) -> Option<&Range<T>> {
        self.tree.get_by(|range| {
            if pos < range.start {
                return Ordering::Less;
            }

            if pos > range.end {
                return Ordering::Greater;
            }

            Ordering::Equal
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranges() {
        let mut ranges = Ranges::new();

        assert_eq!(ranges.find_value(10), None);
        assert_eq!(ranges.find_range(10), None);

        ranges.add(10, 20, "foo");
        ranges.add(30, 50, "bar");
        ranges.add(60, 90, "baz");

        assert_eq!(ranges.find_value(10), Some(&"foo"));
        assert_eq!(ranges.find_value(40), Some(&"bar"));
        assert_eq!(ranges.find_value(90), Some(&"baz"));

        assert_eq!(ranges.find_value(100), None);

        assert_eq!(ranges.find_range(15), Some((10, 20)));
        assert_eq!(ranges.find_range(0), None);
    }
}
