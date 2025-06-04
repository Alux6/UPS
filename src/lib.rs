#![no_std]

#![feature(abi_x86_interrupt)]

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use alloc::vec::Vec;
use crate::serial::Green;
use crate::serial::Red;

extern crate alloc;

pub mod allocator;
pub mod serial;
pub mod vga_buffer;


pub mod fs;
pub mod shell;

pub mod gdt;

pub mod interrupts;

pub mod memory;

#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable(); 
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

pub fn str_to_fat_name(filename: &str) -> [u8; 11] {
    let mut fat_name = [b' '; 11];

    let parts: Vec<&str> = filename.split('.').collect();

    if let Some(name) = parts.get(0) {
        for (i, &b) in name.as_bytes().iter().take(8).enumerate() {
            fat_name[i] = b.to_ascii_uppercase();
        }
    }

    if parts.len() > 1 {
        if let Some(ext) = parts.get(1) {
            for (i, &b) in ext.as_bytes().iter().take(3).enumerate() {
                fat_name[8 + i] = b.to_ascii_uppercase();
            }
        }
    }

    fat_name
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("{}",Green("[ok]"));
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("{}", Red("[failed]\n"));
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

#[cfg(test)]
#[unsafe(no_mangle)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    init();
    test_main();
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
