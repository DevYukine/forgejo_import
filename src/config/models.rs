use serde_derive::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgejoImportConfig {
    pub forgejo_url: Option<String>,
    pub forgejo_token: Option<String>,
    pub github_token: Option<String>,
    pub migrate_wiki: Option<bool>,
    pub migrate_lfs: Option<bool>,
}
