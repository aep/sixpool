use nix;
error_chain! {
    foreign_links {
        Nix(nix::Error);
        Io(::std::io::Error);
    }
}

