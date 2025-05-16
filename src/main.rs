#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(ups::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use ups::println;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", '!');

    ups::init();

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
