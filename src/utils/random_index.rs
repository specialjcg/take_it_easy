pub fn random_index(max: usize) -> usize {
    use rand::Rng;
    let mut rng = rand::rng();
    rng.random_range(0..max)
}
