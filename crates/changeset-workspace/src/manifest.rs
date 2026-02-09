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
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkspacePackage {
    pub(crate) version: Option<String>,
}
