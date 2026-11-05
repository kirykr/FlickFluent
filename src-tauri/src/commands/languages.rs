//! Language list and pair check, built from the model catalogue and, once a key is verified,
//! the cloud services (features 002 US2 and 003 US5).

use flickfluent_core::languages::{check_pair_with_cloud, list_with_cloud, Language, PairCheck};
use flickfluent_core::models::bundled_catalogue;

use super::cloud::CloudState;

/// Every language the catalogue and the set-up cloud services know, with what can be done with
/// it. `opt_in` lists the non-commercial models the user turned on.
#[tauri::command]
pub fn languages_list(
    cloud: tauri::State<CloudState>,
    opt_in: Option<Vec<String>>,
) -> Vec<Language> {
    list_with_cloud(
        &bundled_catalogue(),
        &opt_in.unwrap_or_default(),
        &cloud.offers(),
    )
}

/// What `source` (a code or `auto`) to `target` needs, and the fallbacks when a step cannot do
/// it, including the cloud services. The UI shows this before `dub_start`.
#[tauri::command]
pub fn languages_check_pair(
    cloud: tauri::State<CloudState>,
    source: String,
    target: String,
    opt_in: Option<Vec<String>>,
) -> PairCheck {
    check_pair_with_cloud(
        &bundled_catalogue(),
        &source,
        &target,
        &opt_in.unwrap_or_default(),
        &cloud.offers(),
    )
}
