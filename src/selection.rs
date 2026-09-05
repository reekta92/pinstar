//! Multi-select state for canvas nodes (ported from clin's `ui::CanvasSelection`).

use std::collections::HashSet;
use std::hash::Hash;

pub struct Selection<Id: Eq + Hash + Clone> {
    pub primary: Option<Id>,
    pub extra: HashSet<Id>,
}

impl<Id: Eq + Hash + Clone> Selection<Id> {
    pub fn new() -> Self {
        Self {
            primary: None,
            extra: HashSet::new(),
        }
    }

    pub fn select_only(&mut self, id: Id) {
        self.primary = Some(id);
        self.extra.clear();
    }

    pub fn clear(&mut self) {
        self.primary = None;
        self.extra.clear();
    }

    pub fn clear_set(&mut self) {
        self.extra.clear();
    }

    pub fn replace_set(&mut self, set: HashSet<Id>, primary: Option<Id>) {
        self.extra = set;
        self.primary = primary;
    }

    pub fn add(&mut self, id: Id) {
        self.extra.insert(id);
    }

    pub fn is_selected(&self, id: &Id) -> bool {
        self.primary.as_ref().is_some_and(|p| p == id) || self.extra.contains(id)
    }

    pub fn all(&self) -> HashSet<Id> {
        let mut out = self.extra.clone();
        if let Some(p) = &self.primary {
            out.insert(p.clone());
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.primary.is_none() && self.extra.is_empty()
    }

    pub fn count(&self) -> usize {
        self.extra.len() + usize::from(self.primary.is_some())
    }
}

impl<Id: Eq + Hash + Clone> Default for Selection<Id> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_only_clears_extra() {
        let mut s: Selection<String> = Selection::new();
        s.add("a".into());
        s.select_only("b".into());
        assert!(s.extra.is_empty());
        assert_eq!(s.primary.as_deref(), Some("b"));
    }

    #[test]
    fn clear_nukes_both() {
        let mut s: Selection<String> = Selection::new();
        s.select_only("a".into());
        s.add("b".into());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn clear_set_keeps_primary() {
        let mut s: Selection<String> = Selection::new();
        s.select_only("a".into());
        s.add("b".into());
        s.clear_set();
        assert_eq!(s.primary.as_deref(), Some("a"));
        assert!(s.extra.is_empty());
    }

    #[test]
    fn is_selected_primary_and_extra() {
        let mut s: Selection<String> = Selection::new();
        s.select_only("a".into());
        s.add("b".into());
        assert!(s.is_selected(&"a".to_string()));
        assert!(s.is_selected(&"b".to_string()));
        assert!(!s.is_selected(&"c".to_string()));
    }

    #[test]
    fn all_unions() {
        let mut s: Selection<String> = Selection::new();
        s.select_only("a".into());
        s.add("b".into());
        assert_eq!(s.all().len(), 2);
    }
}
