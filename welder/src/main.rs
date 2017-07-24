extern crate nix;
#[macro_use]
extern crate error_chain;

mod errors;
mod filesystem;
mod container;
mod system;

fn main() {
    let system = system::System::new();
    container::Container::main(&system);
}

