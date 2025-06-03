#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(ups::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

mod fs;

use crate::fs::fat32::{BLOCK_DEVICE, FileSystem};

use core::panic::PanicInfo;
use ups::{allocator, println, serial_println};

use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static BootInfo) -> ! {

    use x86_64::{VirtAddr};
    use ups::memory::{self,
        BootInfoFrameAllocator,
    };

    ups::init();
    println!("Hello world!");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);    
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");


    let mut block_device = BLOCK_DEVICE.lock();

    // Create the filesystem using the locked &mut RamDisk
    let fs = FileSystem::new(&mut *block_device).expect("Failed to create FS");

    println!("FAT starts at sector {}", fs.fat_start);

    serial_println!("BPB: {:?}", fs.bpb);
    serial_println!("EBR: {:?}", fs.ebr);

    #[cfg(test)]
    test_main();

    println!("It didn't crash!");

    ups::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    ups::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    ups::test_panic_handler(info)
}
