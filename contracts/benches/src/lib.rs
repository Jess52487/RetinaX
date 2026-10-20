pub fn add(a: u64, b: u64) -> u64 {
    a.saturating_add(b)
}

pub fn expensive_computation(iterations: u64) -> u64 {
    let mut sum: u64 = 0;
    for i in 0..iterations {
        sum = sum.wrapping_add(i.wrapping_mul(31));
    }
    sum
}

pub fn storage_simulation(data: &mut std::vec::Vec<u8>, val: u8) {
    data.push(val);
}
