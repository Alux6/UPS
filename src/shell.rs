use crate::{fs::fat32::{FileSystem, BLOCK_DEVICE}, print};

use x86_64::structures::paging::{OffsetPageTable, FrameAllocator, Size4KiB};

use core::sync::atomic::Ordering::SeqCst;

use spin::Mutex;
use crate::AtomicBool;
use lazy_static::lazy_static;
use crate::debug;
use debug::DEBUG_FS;
use crate::println;

pub static EXECUTE_COMMAND: AtomicBool = AtomicBool::new(false);

use core::fmt::Write;

use alloc::string::String;

pub struct Terminal {
    cwd: u32,
    buffer: [u8; 512],
    index: usize,
}

impl Terminal {
    pub fn new() -> Self {
        Self {
            cwd: 2,
            buffer: [0;512],
            index: 0,
        }
    }

    pub fn push_char(&mut self, c: u8) -> String {
        let mut out = String::new();
        if self.index < self.buffer.len() {
            self.buffer[self.index] = c;
            self.index += 1;
        }
        let _ = write!(out, "{}", c as char);
        out
    }

    pub fn pop_char(&mut self){
        if self.index > 0 {
            self.index -= 1;
        }
        self.buffer[self.index] = b' ';
    }

    pub fn execute_command(&mut self) -> String {

        let mut out = String::new();

        let _ = writeln!(out,"");

        let input = &self.buffer[..self.index];
        if let Ok(cmd) = core::str::from_utf8(input) {

            let trimmed = cmd.trim();
            let mut parts = trimmed.split_whitespace();

            let command = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("");

            match command {

                "ls" => {
                    let mut dev = BLOCK_DEVICE.lock();
                    let mut fs = FileSystem::new(&mut *dev)
                        .expect("failed to mount FS");
                    out.push_str(&fs.return_tree(self.cwd, 0));
                }
                "mkdir" => {
                    let mut dev = BLOCK_DEVICE.lock();
                    let mut fs = FileSystem::new(&mut *dev)
                        .expect("failed to mount FS");
                    if arg != "" {
                        let _ = fs.create_dir(self.cwd, arg);
                    }
                }
                "help" => {
                    let _ = writeln!(out, "\
Available commands:

ls
List the contents of the current directory.

mkdir <name>
Create a new directory with the given <name> in the current directory.

touch <name>
Create a new empty file with the given <name> in the current directory.

cd <dirname>
Change the current directory to <dirname>.

bk <component>
Toggle FileSystem debugging on or off.

clear
Clear the screen by printing empty lines.

help
Show this help message.");
                }

                "touch" => {
                    let mut dev = BLOCK_DEVICE.lock();
                    let mut fs = FileSystem::new(&mut *dev)
                        .expect("failed to mount FS");
                    if arg != "" {
                        let _ = fs.create_file(self.cwd, arg);
                    }
                }
                "cd" => {
                    let mut dev = BLOCK_DEVICE.lock();
                    let mut fs = FileSystem::new(&mut *dev)
                        .expect("failed to mount FS");
                    if arg != "" {

                        if let Some(cluster) = fs.find_dir_in(self.cwd, arg) {
                            self.cwd = cluster;
                            let _ = writeln!(out, "Changed directory to {}", arg);
                        } 

                        else {
                            let _ = writeln!(out, "Directory '{}' not found", arg);
                        }

                    } else {
                        let _ = writeln!(out, "Usage: cd <dirname>");
                    }
                }

                "bk" => {

                    match arg {
                        "fs" => {
                            let val = DEBUG_FS.load(SeqCst);
                            DEBUG_FS.store(!val, SeqCst);

                            if DEBUG_FS.load(SeqCst) {
                                let _ = writeln!(out, "FileSystem debugging activated");
                            }
                            else{
                                let _ = writeln!(out, "FileSystem debugging deactivated");
                            }
                        }
                        "help" => {

                    let _ = writeln!(out, "\
Available components:

fs
File system.");
                        }
                        "" => {}
                        _ => {}
                    }

                    let _ = write!(out,"");
                }

                "clear" => {
                    for _ in 0..50 {
                        let _ = writeln!(out,"");
                    }
                }

                "" => {}
                _ => {
                    let _ = writeln!(out, "Unknown command: {}", cmd);
                }
            }

        } else {
            let _ = writeln!(out, "Invalid UTF-8 input");
        }

        let _ = write!(out, "> ");

        self.index = 0;
        out
    }
}


lazy_static! {
    pub static ref TERMINAL: Mutex<Terminal> = Mutex::new(Terminal::new());
}

pub fn init() {
    lazy_static::initialize(&TERMINAL);

    crate::print!("> ");
}
