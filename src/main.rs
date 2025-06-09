#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(ups::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use ups::fs::fat32::{BLOCK_DEVICE, FileSystem};
use ups::vga_buffer::disable_hardware_cursor;
use ups::shell;
use ups::interrupts;

use core::panic::PanicInfo;
use ups::{allocator, println};

use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static BootInfo) -> ! {

    interrupts::mask_irq1();
        
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

    println!("Heap size: {} MB", allocator::HEAP_SIZE / 1024 / 1024);

    {
        let mut block_device = BLOCK_DEVICE.lock();

        // Create the filesystem using the locked &mut RamDisk
        let mut fs = FileSystem::new(&mut *block_device).expect("Failed to create FS");

        println!("FAT starts at sector {}", fs.fat_start);

        let occupied = fs.count_occupied_clusters();
        println!("Occupied clusters: {}", occupied);

        fs.init_fats();
        println!("FAT was just set up");

        let occupied = fs.count_occupied_clusters();
        println!("Occupied clusters: {}", occupied);

        fs.create_root_dir().unwrap();

        let occupied = fs.count_occupied_clusters();
        println!("Occupied clusters: {}", occupied);


        fs.create_file(2u32,&"Hellowo.rld").unwrap();
        fs.create_dir(2u32,&"Hellodir").unwrap();

        let occupied = fs.count_occupied_clusters();
        println!("Occupied clusters: {}", occupied);
    }

    disable_hardware_cursor();


    shell::init();

    interrupts::unmask_irq1();

    #[cfg(test)]
    test_main();

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
