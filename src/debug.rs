use spin::{Mutex};
use crate::vga_buffer::WRITER;
use core::fmt::{self,Write};
use crate::{print, println};

use crate::DEBUG_MODE;
use core::sync::atomic::Ordering::SeqCst;

const DEBUG_BUF_SIZE: usize = 1024;

const VGA_BUFFER_ADDR: usize = 0xb8000;
const BUFFER_SIZE: usize = 80 * 25 * 2;

const VGA_ATTRIBUTE_BYTE: u8 = 0x0E;
const SCREEN_WIDTH: usize = 80;
const SCREEN_HEIGHT: usize = 25;

static SAVED_SCREEN: Mutex<[u8; BUFFER_SIZE]> = Mutex::new([0; BUFFER_SIZE]);

static DEBUG_BUFFER: Mutex<DebugBuffer> = Mutex::new(DebugBuffer::new());

pub struct SavedScreenWriter;

impl Write for SavedScreenWriter {

    fn write_str(&mut self, _s: &str) -> fmt::Result {
        let mut buffer = SAVED_SCREEN.lock();

        for row in 1..SCREEN_HEIGHT {
            for col in 0..SCREEN_WIDTH {
                let from = 2 * (row * SCREEN_WIDTH + col);
                let to = 2 * ((row - 1) * SCREEN_WIDTH + col);
                buffer[to] = buffer[from];
                buffer[to + 1] = buffer[from + 1];
            }
        }

        let last_row = SCREEN_HEIGHT - 1;
        for col in 0..SCREEN_WIDTH {
            let idx = 2 * (last_row * SCREEN_WIDTH + col);
            buffer[idx] = b' ';
            buffer[idx + 1] = VGA_ATTRIBUTE_BYTE;
        }

        let mut col = 0;
        for byte in _s.bytes() {
            if col >= SCREEN_WIDTH {
                break;
            }
            let b = match byte {
                0x20..=0x7e => byte,
                b'\n' | b'\r' => continue,
                _ => 0xfe,
            };

            let idx = 2 * (last_row * SCREEN_WIDTH + col);
            buffer[idx] = b;
            buffer[idx + 1] = VGA_ATTRIBUTE_BYTE;
            col += 1;
        }

        Ok(())
    }

}

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

    unsafe {
        core::arch::asm!("int3");
    }
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

    DEBUG_MODE.store(true, SeqCst);
    draw_debug_buffer();
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

    DEBUG_MODE.store(false, SeqCst);
}

