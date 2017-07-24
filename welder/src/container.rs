use nix::unistd::{fork, ForkResult, Pid, execvp};
use nix::sys::wait::{waitpid, WaitStatus, WNOHANG};
use std::ffi::CString;
use ::filesystem::{Filesystem};
use system::System;

pub struct Container<'s> {
    fs: Filesystem,
    system: &'s System,
}

impl<'s> Container<'s> {
    pub fn main(system: &System) {
        let c = Container{
            system: system,
            fs: Filesystem::new(system.cdir.clone() + "/containera/root"),
        };
        c.run();
    }
    fn run(&self) {
        match fork() {
            Ok(ForkResult::Parent { child, .. }) => self.parent(child),
            Ok(ForkResult::Child) => self.child(),
            Err(e) => panic!(e),
        }
    }
    fn parent(&self, cpid : Pid) {
        match waitpid(cpid, None) {
            Err(e) => panic!(e),
            Ok(s) => {
            }
        };
    }
    fn child(&self) {
        println!("in container");

        self.fs.clear().unwrap();
        //self.fs.mount(Some("tmpfs"), String::from(""),     Some("tmpfs"));
        self.fs.bind("/system", String::from("/system"));
        self.fs.bind("/dev",    String::from("/dev"));
        self.fs.mount(None,     String::from("/proc"), Some("proc"));

        self.fs.pivot().expect("cannot pivot_root");

        execvp(&CString::new("sh").unwrap(), &[CString::new("-li").unwrap()]).unwrap();
    }
}





