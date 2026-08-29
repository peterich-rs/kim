pub fn build_slots(weights: &[u32]) -> Vec<usize> {
    let mut slots = Vec::new();
    for (i, w) in weights.iter().enumerate() {
        for _ in 0..*w {
            slots.push(i);
        }
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_and_empty() {
        assert_eq!(build_slots(&[2, 1]), vec![0, 0, 1]);
        assert!(build_slots(&[0, 0]).is_empty());
    }
}
