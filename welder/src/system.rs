use std::fs::{File, create_dir_all};
use nix::mount::{mount, MsFlags};
use filesystem;

pub struct System {
    pub cdir: String
}

impl System {
    fn make_cdir(&self) {
        create_dir_all(&*self.cdir).unwrap();
        for m in filesystem::mounts().unwrap() {
            if m.target == self.cdir {
                return
            }
        }
        mount(None as Option<&str>, &*self.cdir, Some("tmpfs"), MsFlags::empty(), None as Option<&str>)
            .expect("cannot mount tmpfs to /mnt/sixpool");
    }

    pub fn new () -> System{
        println!("~~~system init for android");

        let s = System {
            cdir: String::from("/mnt/sixpool"),
        };
        s.make_cdir();
        return s;
    }
}
