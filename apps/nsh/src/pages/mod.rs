#[derive(Clone, Debug, PartialEq)]
pub enum ViewerRoute {
    Demo,
    Project {
        project_id: String,
        pcb_document_id: Option<String>,
        schematic_document_id: Option<String>,
        title: String,
        initial_schematic: bool,
    },
}
pub mod bom;
pub mod discover;
pub mod project;
pub mod viewer;
