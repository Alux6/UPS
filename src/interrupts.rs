use core::fmt::Write;

use crate::DEBUG_MODE;
use crate::shell::EXECUTE_COMMAND;
use core::sync::atomic::Ordering::SeqCst;

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;
use crate::print;


use alloc::string::String;
use lazy_static::lazy_static;
use crate::gdt;
use crate::shell::TERMINAL;
use crate::vga_buffer::WRITER;

use pic8259::ChainedPics;
use spin;

use x86_64::structures::idt::PageFaultErrorCode;
use crate::hlt_loop;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });


#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

pub fn mask_irq0() {
    unsafe {
        let mut port = x86_64::instructions::port::Port::new(0x21);
        let mask: u8 = port.read();
        port.write(mask | 0x01);
    }
}

pub fn unmask_irq0() {
    unsafe {
        let mut port = x86_64::instructions::port::Port::new(0x21);
        let mask: u8 = port.read();
        port.write(mask & !0x01);
    }
}

pub fn mask_irq1() {
    unsafe {
        let mut port = x86_64::instructions::port::Port::new(0x21);
        let mask: u8 = port.read();
        port.write(mask | 0x02);
    }
}

pub fn unmask_irq1() {
    unsafe {
        let mut port = x86_64::instructions::port::Port::new(0x21);
        let mask: u8 = port.read();
        port.write(mask & !0x02);
    }
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
    /*
    fn as_usize(self) -> usize {
    usize::from(self.as_u8())
    }
    */
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_interrupt_handler);
        idt
    };
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(ScancodeSet1::new(),
            layouts::Us104Key, HandleControl::Ignore)
        );
    }

    let mut port = Port::new(0x60);
    let mut keyboard = KEYBOARD.lock();

    let scancode: u8 = unsafe {port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(decoded) = keyboard.process_keyevent(key_event) {
            if let DecodedKey::Unicode(character) = decoded {
                let byte = character as u8; 
                if DEBUG_MODE.load(SeqCst){
                    if character == 'q' {
                        DEBUG_MODE.store(false, SeqCst);
                    }
                }
                else {
                    let mut term = TERMINAL.lock();
                    if character == '\u{8}' {
                        term.pop_char();
                        WRITER.lock().delete_byte();
                    }
                    else{
                        let mut result: String = term.push_char(byte);
                        if character == '\n' || character == '\r' {
                            EXECUTE_COMMAND.store(true, SeqCst);
                        }
                            print!("{}", result);
                    }
                }
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{    

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}


pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    return;
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
