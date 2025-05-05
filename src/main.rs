#![no_std]
#![no_main]

mod vga_buffer;

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", '!');

    ups::init(); 

    x86_64::instructions::interrupts::int3();

    panic!("Oh no the laptop is burning!");
    
    // loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
