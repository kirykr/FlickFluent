use super::CheckItem;

/// Build the user-facing alert from the missing items only.
pub fn blocked_message(missing: &[CheckItem]) -> String {
    let parts: Vec<String> = missing
        .iter()
        .map(|i| format!("{} (found {})", i.needed_label, i.found_label))
        .collect();
    let list = match parts.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [head @ .., last] => format!("{} and {}", head.join(", "), last),
    };
    format!("Your computer is not compatible with this app. You need {list} to run this app.")
}
