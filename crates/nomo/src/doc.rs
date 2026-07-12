use crate::project::{Project, project_package_id};
use std::path::Path;

pub use nomo_doc::{
    DocError, DocItem, DocModule, DocPackage, collect_source_docs, generate_source_docs,
    generate_std_docs, render_packages_json, std_doc_package, write_doc_index,
};

pub fn generate_project_docs(project: &Project, output: &Path) -> Result<DocPackage, DocError> {
    let package_id = project_package_id(project).map_err(DocError::Message)?;
    generate_source_docs(
        &project.root,
        &project.root.join("src"),
        &package_id,
        output,
    )
}

pub fn collect_project_docs(project: &Project, package_id: &str) -> Result<DocPackage, DocError> {
    collect_source_docs(&project.root, &project.root.join("src"), package_id)
}
