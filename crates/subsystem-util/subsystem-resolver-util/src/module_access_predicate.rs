// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_resolver::access_solver::AccessPredicate;

/// Simple boolean access predicate for module-level access control.
#[derive(Debug)]
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

impl AccessPredicate for ModuleAccessPredicate {
    fn and(self, other: Self) -> Self {
        (bool::from(self) && bool::from(other)).into()
    }

    fn or(self, other: Self) -> Self {
        (bool::from(self) || bool::from(other)).into()
    }

    fn is_true(&self) -> bool {
        matches!(self, ModuleAccessPredicate::True)
    }

    fn is_false(&self) -> bool {
        matches!(self, ModuleAccessPredicate::False)
    }
}
