/// This module *will contain* the logic for running the program through a GUI

#[cfg(not(feature = "gui"))]
pub fn gui() {
    println!("GUI is not supported (this executable has been compiled without GUI support)!");
}

#[cfg(feature = "gui")]
pub fn gui() {
    todo!("GUI is not yet implemented");
}
