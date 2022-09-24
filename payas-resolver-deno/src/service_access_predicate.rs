pub enum ServiceAccessPredicate {
    True,
    False,
}

impl From<bool> for ServiceAccessPredicate {
    fn from(b: bool) -> Self {
        if b {
            ServiceAccessPredicate::True
        } else {
            ServiceAccessPredicate::False
        }
    }
}

impl From<ServiceAccessPredicate> for bool {
    fn from(predicate: ServiceAccessPredicate) -> Self {
        match predicate {
            ServiceAccessPredicate::True => true,
            ServiceAccessPredicate::False => false,
        }
    }
}

impl std::ops::Not for ServiceAccessPredicate {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            ServiceAccessPredicate::True => ServiceAccessPredicate::False,
            ServiceAccessPredicate::False => ServiceAccessPredicate::True,
        }
    }
}
