use codemap::{CodeMap, Span};

pub fn null_span() -> Span {
    let mut codemap = CodeMap::new();
    let file = codemap.add_file("".to_string(), "".to_string());
    file.span
}

/// Join strings together with commas and an optional separator before the last word.
///
/// e.g. `join_strings(vec!["a", "b", "c"], Some("or")) == "a, b, or c"`
pub fn join_strings(strs: &[String], last_sep: Option<&'static str>) -> String {
    match strs.len() {
        1 => strs[0].to_string(),
        2 => match last_sep {
            Some(last_sep) => format!("{} {} {}", strs[0], last_sep, strs[1]),
            None => format!("{}, {}", strs[0], strs[1]),
        },
        _ => {
            let mut joined = String::new();
            for i in 0..strs.len() {
                joined.push_str(&strs[i]);
                if i < strs.len() - 1 {
                    joined.push_str(", ");
                }
                if i == strs.len() - 2 {
                    joined.push_str(last_sep.unwrap_or(""));
                    joined.push(' ');
                }
            }
            joined
        }
    }
}
