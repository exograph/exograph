#![cfg(test)]

// TODO: Do we really need three cases here (the middle one looks unnecessary)
macro_rules! assert_params {
    ($actual_params:expr) => {
        assert!($actual_params.is_empty(), "Extra actual parameters");
    };
    ($actual_params:expr, $expected_param:expr) => {
        match $actual_params.split_first() {
            Some((actual_head, actual_tail)) => {
                let actual_boxed_head = actual_head.as_any().downcast_ref::<$crate::sql::SQLParamContainer>();
                match actual_boxed_head {
                    Some(actual_boxed_head) => {
                        let actual_head = actual_boxed_head.as_ref();
                        assert_eq!(
                            &actual_head,
                            &(&$expected_param as &dyn $crate::sql::SQLParam),
                            "Parameter mismatch"
                        );
                    },
                    None => {
                        assert_eq!(&actual_head.as_ref(), &(&$expected_param as &dyn $crate::sql::SQLParam), "Parameter mismatch");
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
                let actual_boxed_head = actual_head.as_any().downcast_ref::<$crate::sql::SQLParamContainer>();
                match actual_boxed_head {
                    Some(actual_boxed_head) => {
                        let actual_head = actual_boxed_head.as_ref();
                        assert_eq!(
                            &actual_head,
                            &(&$expected_param as &dyn $crate::sql::SQLParam),
                            "Parameter mismatch"
                        );
                    },
                    None => {
                        assert_eq!(&actual_head.as_ref(), &(&$expected_param as &dyn $crate::sql::SQLParam), "Parameter mismatch");
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
        let (actual_stmt, actual_params) = $actual.string_expression();
        assert_eq!(actual_stmt, $expected_stmt);
        assert_params!(actual_params);
    };
    ($actual:expr, $expected_stmt:expr, $($rest:expr), *) => {
        let (actual_stmt, actual_params) = $actual.string_expression();
        assert_eq!(actual_stmt, $expected_stmt);
        assert_params!(actual_params, $($rest), *);
    };
}
