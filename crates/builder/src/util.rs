// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// Join strings together with commas and an optional separator before the last word.
///
/// e.g. `join_strings(vec!["a", "b", "c"], Some("or")) == "a, b, or c"`
pub fn join_strings(strs: &[impl AsRef<str>], last_sep: Option<&'static str>) -> String {
    const COMMA: &str = ", ";

    match strs.len() {
        1 => strs[0].as_ref().to_string(),
        2 => match last_sep {
            Some(last_sep) => format!("{} {} {}", strs[0].as_ref(), last_sep, strs[1].as_ref()),
            None => format!("{}{}{}", strs[0].as_ref(), COMMA, strs[1].as_ref()),
        },
        _ => {
            let mut joined = String::new();
            for i in 0..strs.len() {
                joined.push_str(strs[i].as_ref());
                if i < strs.len() - 1 {
                    joined.push_str(COMMA);
                }
                if i == strs.len() - 2
                    && let Some(last_sep) = last_sep
                {
                    joined.push_str(last_sep);
                    joined.push(' ');
                }
            }
            joined
        }
    }
}

#[cfg(test)]
mod tests {
    use super::join_strings;
    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn join_strings_no_last_sep() {
        assert!(join_strings(&["a"], None) == "a");
        assert!(join_strings(&["a", "b"], None) == "a, b");
        assert!(join_strings(&["a", "b", "c"], None) == "a, b, c");
    }

    #[multiplatform_test]
    fn join_strings_with_last_sep() {
        assert!(join_strings(&["a"], Some("or")) == "a");
        assert!(join_strings(&["a", "b"], Some("or")) == "a or b");
        assert!(join_strings(&["a", "b", "c"], Some("or")) == "a, b, or c");
    }
}
