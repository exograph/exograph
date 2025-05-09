// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Test assertion to check SQL statements and parameters.

/// Assert that the given parameters match the expected ones.
///
/// # Usage:
/// ```no_run
/// assert_params!(actual_params, expected_param1, expected_param2, ...);`
/// ```
///
/// # Example:
/// ```
/// let actual_params = vec![Box::new(1), Box::new("hello")];
/// assert_params!(actual_params, 1, "hello"); // will pass
/// ```
///
/// ```
/// let actual_params = vec![];
/// assert_params!(actual_params); // will pass
/// ```
///
/// ```should_panic
/// let actual_params = vec![Box::new(1), Box::new("hello")];
/// assert_params!(actual_params, 1, "world"); // will fail (parameter mismatch)
/// ```
///
/// ```should_panic
/// let actual_params = vec![Box::new(1), Box::new("hello")];
/// assert_params!(actual_params, 1); // will fail (too few parameters)
/// ```
///
/// ```should_panic
/// let actual_params = vec![Box::new(1), Box::new("hello")];
/// assert_params!(actual_params, 1, "hello", 2); // will fail (too many parameters)
/// ```
macro_rules! assert_params {
    // TODO: Do we really need three cases here (the middle one looks unnecessary)
    ($actual_params:expr) => {
        assert!($actual_params.is_empty(), "Extra actual parameters");
    };
    ($actual_params:expr, $expected_param:expr) => {
        match $actual_params.split_first() {
            Some((actual_head, actual_tail)) => {
                let actual_boxed_head = actual_head.param.as_any().downcast_ref::<$crate::sql::SQLParamContainer>();
                match actual_boxed_head {
                    Some(actual_boxed_head) => {
                        let actual_head = actual_boxed_head.param();
                        let actual_head = actual_head.param.as_ref();
                        assert_eq!(
                            &actual_head,
                            &(&$expected_param as &dyn $crate::sql::SQLParam),
                            "Parameter mismatch"
                        );
                    },
                    None => {
                        assert_eq!(actual_head.param.as_ref(), (&$expected_param as &dyn $crate::sql::SQLParam), "Parameter mismatch");
                    }
                }
                assert_eq!(actual_tail.len(), 0, "Extra actual parameters")
            },
            None => assert!(false)
        }
    };
    ($actual_params:expr, $expected_param:expr, $($rest:expr), *) => {
        match $actual_params.split_first() {
            Some((actual_head, actual_tail)) => {
                let actual_boxed_head = actual_head.param.as_any().downcast_ref::<$crate::sql::SQLParamContainer>();
                match actual_boxed_head {
                    Some(actual_boxed_head) => {
                        let actual_head = actual_boxed_head.param();
                        let actual_head = actual_head.param.as_ref();
                        assert_eq!(
                            &actual_head,
                            &(&$expected_param as &dyn $crate::sql::SQLParam),
                            "Parameter mismatch"
                        );
                    },
                    None => {
                        assert_eq!(
                            &actual_head.param.as_ref(),
                            &(&$expected_param as &dyn $crate::sql::SQLParam),
                            "Parameter mismatch"
                        );
                    }
                }
                assert_params!(actual_tail, $($rest), *);
            },
            None => assert!(false)
        }
    };
}

macro_rules! assert_binding {
    ($actual:expr, $expected_stmt:expr) => {
        let (actual_stmt, actual_params) = $actual;
        assert_eq!(actual_stmt, $expected_stmt);
        assert_params!(actual_params);
    };
    ($actual:expr, $expected_stmt:expr, $($rest:expr), *) => {
        let (actual_stmt, actual_params) = $actual;
        assert_eq!(actual_stmt, $expected_stmt);
        assert_params!(actual_params, $($rest), *);
    };
}
