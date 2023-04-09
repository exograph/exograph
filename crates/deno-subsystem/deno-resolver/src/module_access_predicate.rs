pub enum ModuleAccessPredicate {
    True,
    False,
}

impl From<bool> for ModuleAccessPredicate {
    fn from(b: bool) -> Self {
        if b {
            ModuleAccessPredicate::True
        } else {
            ModuleAccessPredicate::False
        }
    }
}

impl From<ModuleAccessPredicate> for bool {
    fn from(predicate: ModuleAccessPredicate) -> Self {
        match predicate {
            ModuleAccessPredicate::True => true,
            ModuleAccessPredicate::False => false,
        }
    }
}

impl std::ops::Not for ModuleAccessPredicate {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            ModuleAccessPredicate::True => ModuleAccessPredicate::False,
            ModuleAccessPredicate::False => ModuleAccessPredicate::True,
        }
    }
}
