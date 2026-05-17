use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn main() {
    for value in std::env::args().skip(1) {
        let mut hasher = DefaultHasher::new();
        value.to_lowercase().hash(&mut hasher);
        println!("{value}=profile-{:x}", hasher.finish());
    }
}
