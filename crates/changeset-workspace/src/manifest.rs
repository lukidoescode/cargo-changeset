use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CargoManifest {
    pub package: Option<Package>,
    pub workspace: Option<WorkspaceSection>,
}

#[derive(Debug, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Option<VersionField>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum VersionField {
    Literal(String),
    Inherited(InheritedVersion),
}

#[derive(Debug, Deserialize)]
pub struct InheritedVersion {
    pub workspace: bool,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceSection {
    pub members: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub package: Option<WorkspacePackage>,
}

#[derive(Debug, Deserialize)]
pub struct WorkspacePackage {
    pub version: Option<String>,
}
