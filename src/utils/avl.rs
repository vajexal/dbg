use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct AVLTree<T> {
    root: Option<Box<Node<T>>>,
}

#[derive(Debug, Clone)]
struct Node<T> {
    value: T,
    height: i32,
    left: Option<Box<Node<T>>>,
    right: Option<Box<Node<T>>>,
}

impl<T: PartialOrd> AVLTree<T> {
    pub fn new() -> Self {
        AVLTree { root: None }
    }

    pub fn insert(&mut self, value: T) {
        self.root = Self::insert_into(self.root.take(), value);
    }

    fn insert_into(node: Option<Box<Node<T>>>, value: T) -> Option<Box<Node<T>>> {
        match node {
            Some(mut n) => {
                if value < n.value {
                    n.left = Self::insert_into(n.left.take(), value);
                } else if value > n.value {
                    n.right = Self::insert_into(n.right.take(), value);
                } else {
                    // value is already in the tree, no duplicates allowed
                    return Some(n);
                }

                n.height = 1 + std::cmp::max(Self::height(&n.left), Self::height(&n.right));

                Some(Self::rebalance(n))
            }
            None => Some(Box::new(Node {
                value,
                height: 1,
                left: None,
                right: None,
            })),
        }
    }

    fn rebalance(mut node: Box<Node<T>>) -> Box<Node<T>> {
        let balance_factor = Self::balance_factor(&node);

        if balance_factor > 1 {
            // left heavy
            if Self::balance_factor(&node.left.as_ref().unwrap()) < 0 {
                // left-right case, need to rotate left then right
                node.left = Some(Self::rotate_left(node.left.take().unwrap()));
            }
            return Self::rotate_right(node);
        }

        if balance_factor < -1 {
            // right heavy
            if Self::balance_factor(&node.right.as_ref().unwrap()) > 0 {
                // right-left case, need to rotate right then left
                node.right = Some(Self::rotate_right(node.right.take().unwrap()));
            }
            return Self::rotate_left(node);
        }

        node
    }

    fn rotate_left(mut node: Box<Node<T>>) -> Box<Node<T>> {
        let mut new_root = node.right.take().unwrap();
        node.right = new_root.left.take();
        new_root.left = Some(node);
        new_root.left.as_mut().unwrap().height = 1 + std::cmp::max(Self::height(&new_root.left), Self::height(&new_root.right));
        new_root.height = 1 + std::cmp::max(Self::height(&new_root.left), Self::height(&new_root.right));
        new_root
    }

    fn rotate_right(mut node: Box<Node<T>>) -> Box<Node<T>> {
        let mut new_root = node.left.take().unwrap();
        node.left = new_root.right.take();
        new_root.right = Some(node);
        new_root.right.as_mut().unwrap().height = 1 + std::cmp::max(Self::height(&new_root.left), Self::height(&new_root.right));
        new_root.height = 1 + std::cmp::max(Self::height(&new_root.left), Self::height(&new_root.right));
        new_root
    }

    fn balance_factor(node: &Box<Node<T>>) -> i32 {
        Self::height(&node.left) - Self::height(&node.right)
    }

    fn height(node: &Option<Box<Node<T>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> AVLTreeIterator<T> {
        AVLTreeIterator {
            stack: Vec::new(),
            current_node: self.root.as_ref(),
        }
    }

    pub fn get_by<F>(&self, cmp: F) -> Option<&T>
    where
        F: Fn(&T) -> Ordering,
    {
        Self::get_node_by(&self.root, cmp)
    }

    fn get_node_by<F>(node: &Option<Box<Node<T>>>, cmp: F) -> Option<&T>
    where
        F: Fn(&T) -> Ordering,
    {
        match node {
            Some(n) => match cmp(&n.value) {
                Ordering::Less => Self::get_node_by(&n.left, cmp),
                Ordering::Equal => Some(&n.value),
                Ordering::Greater => Self::get_node_by(&n.right, cmp),
            },
            None => None,
        }
    }
}

pub struct AVLTreeIterator<'a, T> {
    stack: Vec<&'a Box<Node<T>>>,           // stack to simulate the recursion
    current_node: Option<&'a Box<Node<T>>>, // the current node we're visiting
}

impl<'a, T> Iterator for AVLTreeIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        // traverse the leftmost path first
        while let Some(node) = self.current_node {
            self.stack.push(node);
            self.current_node = node.left.as_ref();
        }

        // if stack is empty, there's no more nodes to visit
        if let Some(node) = self.stack.pop() {
            // now, we are visiting a node, so we need to check the right side
            self.current_node = node.right.as_ref();
            return Some(&node.value);
        }

        // no more elements
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let tree: AVLTree<i32> = AVLTree::new();
        let expected: Vec<_> = tree.iter().collect();
        assert_eq!(expected.len(), 0);
    }

    #[test]
    fn test_single_insertion() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&10]);
    }

    #[test]
    fn test_multiple_insertions() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(20);
        tree.insert(5);
        tree.insert(6);
        tree.insert(15);

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&5, &6, &10, &15, &20]);
    }

    #[test]
    fn test_insert_duplicates() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(10); // duplicate insertion
        tree.insert(20);

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&10, &20]);
    }

    #[test]
    fn test_balanced_tree_after_insertions() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(20);
        tree.insert(5);
        tree.insert(6); // this will trigger a rotation (Left-Right case)

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&5, &6, &10, &20]);
    }

    #[test]
    fn test_left_rotation() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(20);
        tree.insert(30); // this will trigger a left rotation

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&10, &20, &30]);
    }

    #[test]
    fn test_right_rotation() {
        let mut tree = AVLTree::new();
        tree.insert(30);
        tree.insert(20);
        tree.insert(10); // this will trigger a right rotation

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&10, &20, &30]);
    }

    #[test]
    fn test_left_right_rotation() {
        let mut tree = AVLTree::new();
        tree.insert(30);
        tree.insert(10);
        tree.insert(20); // this will trigger a left-right rotation

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&10, &20, &30]);
    }

    #[test]
    fn test_right_left_rotation() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(30);
        tree.insert(20); // this will trigger a right-left rotation

        let result: Vec<_> = tree.iter().collect();
        assert_eq!(result, vec![&10, &20, &30]);
    }

    #[test]
    fn test_insertion_order_does_not_affect_result() {
        let mut tree1 = AVLTree::new();
        tree1.insert(10);
        tree1.insert(20);
        tree1.insert(5);

        let mut tree2 = AVLTree::new();
        tree2.insert(5);
        tree2.insert(20);
        tree2.insert(10);

        // both trees should have the same structure (sorted result after traversal)
        let result1: Vec<_> = tree1.iter().collect();
        let result2: Vec<_> = tree2.iter().collect();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_large_input() {
        let mut tree = AVLTree::new();

        for i in 1..=1000 {
            tree.insert(i);
        }

        let result: Vec<_> = tree.iter().map(|&i| i).collect();
        let expected: Vec<_> = (1..=1000).collect();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_get_existing_value() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(20);
        tree.insert(5);
        tree.insert(15);

        assert_eq!(tree.get_by(|x| 10.cmp(x)), Some(&10));
        assert_eq!(tree.get_by(|x| 5.cmp(x)), Some(&5));
        assert_eq!(tree.get_by(|x| 20.cmp(x)), Some(&20));
        assert_eq!(tree.get_by(|x| 15.cmp(x)), Some(&15));
    }

    #[test]
    fn test_get_non_existing_value() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(20);
        tree.insert(5);

        assert_eq!(tree.get_by(|x| 15.cmp(x)), None);
        assert_eq!(tree.get_by(|x| 30.cmp(x)), None);
        assert_eq!(tree.get_by(|x| 100.cmp(x)), None);
    }

    #[test]
    fn test_get_empty_tree() {
        let tree: AVLTree<i32> = AVLTree::new();

        assert_eq!(tree.get_by(|x| 10.cmp(x)), None);
        assert_eq!(tree.get_by(|x| 100.cmp(x)), None);
    }

    #[test]
    fn test_get_after_insertions_and_rotations() {
        let mut tree = AVLTree::new();
        tree.insert(10);
        tree.insert(20);
        tree.insert(30); // this insertion should trigger a rotation

        assert_eq!(tree.get_by(|x| 10.cmp(x)), Some(&10));
        assert_eq!(tree.get_by(|x| 20.cmp(x)), Some(&20));
        assert_eq!(tree.get_by(|x| 30.cmp(x)), Some(&30));
    }
}
