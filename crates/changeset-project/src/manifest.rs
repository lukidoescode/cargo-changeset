use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct CargoManifest {
    pub(crate) package: Option<Package>,
    pub(crate) workspace: Option<WorkspaceSection>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) version: Option<VersionField>,
    pub(crate) metadata: Option<PackageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum VersionField {
    Literal(String),
    Inherited(InheritedVersion),
}

#[derive(Debug, Deserialize)]
pub(crate) struct InheritedVersion {
    pub(crate) workspace: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkspaceSection {
    pub(crate) members: Option<Vec<String>>,
    pub(crate) exclude: Option<Vec<String>>,
    pub(crate) package: Option<WorkspacePackage>,
    pub(crate) metadata: Option<WorkspaceMetadata>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkspacePackage {
    pub(crate) version: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct PackageMetadata {
    pub(crate) changeset: Option<ChangesetMetadata>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct WorkspaceMetadata {
    pub(crate) changeset: Option<ChangesetMetadata>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ChangesetMetadata {
    #[serde(default, rename = "ignored-files")]
    pub(crate) ignored_files: Vec<String>,
    #[serde(default, rename = "changeset-dir")]
    pub(crate) changeset_dir: Option<String>,
}
