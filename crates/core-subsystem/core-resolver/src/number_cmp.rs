use serde_json::Number;

#[derive(Debug, PartialEq, Eq)]
pub struct NumberWrapper(pub Number);

/// Partial ordering for `serde_json::Number` to allow us to compare numbers of different types.
impl PartialOrd for NumberWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let left = &self.0;
        let right = &other.0;

        match left.as_i64() {
            Some(left) => match right.as_i64() {
                Some(right) => Some(left.cmp(&right)),
                None => match right.as_u64() {
                    Some(right) => compare_i64_u64(left, right),
                    None => compare_i64_f64(left, right.as_f64().unwrap()),
                },
            },
            None => match left.as_u64() {
                Some(left) => match right.as_u64() {
                    Some(right) => Some(left.cmp(&right)),
                    None => match right.as_i64() {
                        Some(right) => compare_u64_i64(left, right),
                        None => compare_u64_f64(left, right.as_f64().unwrap()),
                    },
                },
                None => {
                    let left = left.as_f64().unwrap();
                    match right.as_f64() {
                        Some(right) => left.partial_cmp(&right),
                        None => match right.as_i64() {
                            Some(right) => compare_f64_i64(left, right),
                            None => compare_f64_u64(left, right.as_u64().unwrap()),
                        },
                    }
                }
            },
        }
    }
}

fn compare_i64_u64(left: i64, right: u64) -> Option<std::cmp::Ordering> {
    if left < 0 {
        Some(std::cmp::Ordering::Less)
    } else {
        (left as u64).partial_cmp(&right)
    }
}

fn compare_i64_f64(left: i64, right: f64) -> Option<std::cmp::Ordering> {
    (left as f64).partial_cmp(&right)
}

fn compare_u64_i64(left: u64, right: i64) -> Option<std::cmp::Ordering> {
    if right < 0 {
        Some(std::cmp::Ordering::Greater)
    } else {
        left.partial_cmp(&(right as u64))
    }
}

fn compare_u64_f64(left: u64, right: f64) -> Option<std::cmp::Ordering> {
    (left as f64).partial_cmp(&right)
}

fn compare_f64_i64(left: f64, right: i64) -> Option<std::cmp::Ordering> {
    left.partial_cmp(&(right as f64))
}

fn compare_f64_u64(left: f64, right: u64) -> Option<std::cmp::Ordering> {
    left.partial_cmp(&(right as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_eq() {
        let one_u64: Number = Number::from(1u64);
        let one_i64: Number = Number::from(1i64);
        let one_f64: Number = Number::from_f64(1.0).unwrap();

        let ones = vec![one_u64, one_i64, one_f64];

        for left in &ones {
            for right in &ones {
                assert!(
                    NumberWrapper(left.clone()).partial_cmp(&NumberWrapper(right.clone()))
                        == Some(std::cmp::Ordering::Equal)
                )
            }
        }
    }

    #[test]
    fn test_number_lt() {
        let min_u64 = Number::from(u64::MIN);
        let min_i64 = Number::from(i64::MIN);
        let min_f64 = Number::from_f64(f64::MIN).unwrap();

        let max_u64 = Number::from(u64::MAX);
        let max_i64 = Number::from(i64::MAX);
        let max_f64 = Number::from_f64(f64::MAX).unwrap();

        let mins = vec![min_u64, min_i64, min_f64];
        let maxs = vec![max_u64, max_i64, max_f64];

        // any min is less than any max
        for left in &mins {
            for right in &maxs {
                assert!(
                    NumberWrapper(left.clone()).partial_cmp(&NumberWrapper(right.clone()))
                        == Some(std::cmp::Ordering::Less)
                );
                assert!(
                    NumberWrapper(right.clone()).partial_cmp(&NumberWrapper(left.clone()))
                        == Some(std::cmp::Ordering::Greater)
                )
            }
        }
    }
}
