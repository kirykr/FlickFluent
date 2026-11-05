//! Language list and pair check, built from the model catalogue (feature 002, US2).

use flickfluent_core::languages::{check_pair, list, Language, PairCheck};
use flickfluent_core::models::bundled_catalogue;

/// Every language the catalogue knows, with what can be done with it. `opt_in` lists the
/// non-commercial models the user turned on.
#[tauri::command]
pub fn languages_list(opt_in: Option<Vec<String>>) -> Vec<Language> {
    list(&bundled_catalogue(), &opt_in.unwrap_or_default())
}

/// What `source` (a code or `auto`) to `target` needs, and the fallbacks when a step cannot do
/// it. The UI shows this before `dub_start`.
#[tauri::command]
pub fn languages_check_pair(
    source: String,
    target: String,
    opt_in: Option<Vec<String>>,
) -> PairCheck {
    check_pair(
        &bundled_catalogue(),
        &source,
        &target,
        &opt_in.unwrap_or_default(),
    )
}
