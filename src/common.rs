pub fn same(d1: &[u8], d2: &[u8]) -> bool {
    if d1.len() != d2.len() {
        return false;
    }
    for i in 0..d1.len() {
        if d1[i] != d2[i] {
            return false;
        }
    }
    true
}