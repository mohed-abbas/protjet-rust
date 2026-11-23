//! A stack with a fixed capacity, stored inline (journal D-008).

/// The stack is full: the push was refused and the stack is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Full;

/// A stack of at most `N` values that never allocates.
///
/// The data stack (64) and the call stack (16) are both one of these. Keeping them in
/// fixed arrays is what lets a turn run without touching the allocator (README §3).
#[derive(Debug, Clone)]
pub(crate) struct BoundedStack<T, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> BoundedStack<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            items: [T::default(); N],
            len: 0,
        }
    }

    pub(crate) fn is_full(&self) -> bool {
        self.len == N
    }

    pub(crate) fn push(&mut self, value: T) -> Result<(), Full> {
        let slot = self.items.get_mut(self.len).ok_or(Full)?;
        *slot = value;
        self.len += 1;
        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        let top = self.len.checked_sub(1)?;
        let value = *self.items.get(top)?;
        self.len = top;
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_in_first_out() {
        let mut stack = BoundedStack::<i64, 3>::new();
        assert_eq!(stack.pop(), None);
        stack.push(1).unwrap();
        stack.push(2).unwrap();
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn refuses_a_push_when_full() {
        let mut stack = BoundedStack::<i64, 2>::new();
        stack.push(1).unwrap();
        stack.push(2).unwrap();
        assert!(stack.is_full());
        assert_eq!(stack.push(3), Err(Full));
        assert_eq!(stack.pop(), Some(2));
    }
}
