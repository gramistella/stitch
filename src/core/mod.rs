use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<Node>,
    pub expanded: bool,
    pub has_children: bool,
}

mod fs;
mod rust_filters;
mod slint_filters;
mod text;
mod workspace;

pub use fs::*;
pub use rust_filters::*;
pub use slint_filters::*;
pub use text::*;
pub use workspace::*;
