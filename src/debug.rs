use spin::{Mutex};
use x86_64::instructions::hlt;

use crate::interrupts;
use crate::AtomicBool;
use crate::vga_buffer::WRITER;
use crate::{print, println};

pub static DEBUG_FS: AtomicBool = AtomicBool::new(false);


use crate::DEBUG_MODE;
use core::sync::atomic::Ordering::SeqCst;

const DEBUG_BUF_SIZE: usize = 1024;

const VGA_BUFFER_ADDR: usize = 0xb8000;
const BUFFER_SIZE: usize = 80 * 25 * 2;

static SAVED_SCREEN: Mutex<[u8; BUFFER_SIZE]> = Mutex::new([0; BUFFER_SIZE]);

static DEBUG_BUFFER: Mutex<DebugBuffer> = Mutex::new(DebugBuffer::new());

pub struct DebugBuffer {
    data: [u8; DEBUG_BUF_SIZE],
    len: usize,
}

impl DebugBuffer {
    pub const fn new() -> Self {
        Self {
            data: [0; DEBUG_BUF_SIZE],
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn write_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let space = DEBUG_BUF_SIZE.saturating_sub(self.len);
        let to_copy = bytes.len().min(space);
        self.data[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.data[..self.len]).unwrap_or("[Invalid UTF-8]")
    }
}


pub fn debug_log(s: &str) {
    {
        let mut buf = DEBUG_BUFFER.lock();
        buf.clear();
        buf.write_str(s);
    }

    trigger_debug();

    restore_screen_buffer();
}

pub fn trigger_debug() {
    let mut saved = SAVED_SCREEN.lock();

    unsafe {
        core::ptr::copy_nonoverlapping(
            VGA_BUFFER_ADDR as *const u8,
            saved.as_mut_ptr(),
            BUFFER_SIZE,
        );
    }

    interrupts::unmask_irq0();
    DEBUG_MODE.store(true, SeqCst);
    draw_debug_buffer();

    loop {
        if !DEBUG_MODE.load(SeqCst) {
            break;
        }
        hlt();
    }
}

pub fn draw_debug_buffer() {
    let buf = DEBUG_BUFFER.lock();
    for _ in 0..50 {
        println!("");
    }
    print!("{}", buf.as_str());
}

pub fn restore_screen_buffer() {
    let saved = SAVED_SCREEN.lock();

    unsafe {
        core::ptr::copy_nonoverlapping(
            saved.as_ptr(),
            VGA_BUFFER_ADDR as *mut u8,
            BUFFER_SIZE,
        );
    }

    let mut writer = WRITER.lock();
    writer.cursor_position(2);

    interrupts::mask_irq0();
}

