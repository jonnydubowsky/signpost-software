#![crate_name = "radio_module"]
#![no_std]
#![no_main]
#![feature(const_fn,lang_items)]

extern crate cortexm4;
extern crate capsules;
#[macro_use(static_init)]
extern crate kernel;
extern crate sam4l;

extern crate signpost_drivers;
extern crate signpost_hil;

use capsules::console::{self, Console};
use capsules::timer::TimerDriver;
use capsules::virtual_alarm::{MuxAlarm, VirtualMuxAlarm};
use kernel::hil::Controller;
use kernel::hil::spi::SpiMaster;
use kernel::{Chip, MPU, Platform};
use sam4l::usart;

// For panic!()
#[macro_use]
pub mod io;


unsafe fn load_processes() -> &'static mut [Option<kernel::process::Process<'static>>] {
    extern "C" {
        /// Beginning of the ROM region containing app images.
        static _sapps: u8;
    }

    const NUM_PROCS: usize = 2;

    #[link_section = ".app_memory"]
    static mut MEMORIES: [[u8; 8192]; NUM_PROCS] = [[0; 8192]; NUM_PROCS];

    static mut processes: [Option<kernel::process::Process<'static>>; NUM_PROCS] = [None, None];

    let mut addr = &_sapps as *const u8;
    for i in 0..NUM_PROCS {
        // The first member of the LoadInfo header contains the total size of each process image. A
        // sentinel value of 0 (invalid because it's smaller than the header itself) is used to
        // mark the end of the list of processes.
        let total_size = *(addr as *const usize);
        if total_size == 0 {
            break;
        }

        let process = &mut processes[i];
        let memory = &mut MEMORIES[i];
        *process = Some(kernel::process::Process::create(addr, total_size, memory));
        // TODO: panic if loading failed?

        addr = addr.offset(total_size as isize);
    }

    if *(addr as *const usize) != 0 {
        panic!("Exceeded maximum NUM_PROCS.");
    }

    &mut processes
}

/*******************************************************************************
 * Setup this platform
 ******************************************************************************/

struct SignpostController {
    console: &'static Console<'static, usart::USART>,
    gpio: &'static capsules::gpio::GPIO<'static, sam4l::gpio::GPIOPin>,
    timer: &'static TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
    gpio_async: &'static signpost_drivers::gpio_async::GPIOAsync<'static, signpost_drivers::mcp23008::MCP23008<'static>>,
    coulomb_counter_i2c_selector: &'static signpost_drivers::i2c_selector::I2CSelector<'static, signpost_drivers::pca9544a::PCA9544A<'static>>,
    coulomb_counter_generic: &'static signpost_drivers::ltc2941::LTC2941Driver<'static>,
	//ble
	lora
}

impl Platform for SignpostController {
    fn with_driver<F, R>(&mut self, driver_num: usize, f: F) -> R
        where F: FnOnce(Option<&kernel::Driver>) -> R
    {

        match driver_num {
            0 => f(Some(self.console)),
            1 => f(Some(self.gpio)),
            3 => f(Some(self.timer)),
            100 => f(Some(self.gpio_async)),
            101 => f(Some(self.coulomb_counter_i2c_selector)),
            102 => f(Some(self.coulomb_counter_generic)),
            _ => f(None)
        }
    }
}


unsafe fn set_pin_primary_functions() {
    use sam4l::gpio::{PA,PB};
    use sam4l::gpio::PeripheralFunction::{A, B, C, D, E};
    
    //backplane communication 
    PB[00].configure(Some(A)); //SDA
    PB[01].configure(Some(A)); //SCL
    PB[04].configure(None); //MOD_IN
    PB[05].configure(None); //PPS
    PB[03].configure(None); //MOD_OUT
    PA[25].configure(Some(A)); //USB
    PA[26].configure(Some(A)); //USB
    

    //Nucleum Signals
    PB[02].configure(None); //Nucleum Reset
    PB[06].configure(Some(A)); //RTS
    PB[07].configure(Some(A)); //CTS
    PB[08].configure(None); //Boot
    PB[09].configure(Some(A)); //TX
    PB[10].configure(Some(A)); //RX
    PB[11].configure(None); //Power Gate
    PB[13].configure(Some(C)); //CS

    //LoRa Signals
    PA[04].configure(None); //Int1
    PA[05].configure(None); //Int2
    PA[06].configure(None); //Power Gate
    PA[17].configure(None); //Reset
    PA[18].configure(None); //Boot
    PA[19].configure(Some(A)); //Tx
    PA[20].configure(Some(A)); //Rx
    PA[24].configure(Some(A)); //CS

    //GSM Signals
    PA[07].configure(None); //Power Gate
    PA[08].configure(Some(A)); //RTS //can't be used, forgot crossover
    PA[09].configure(Some(A)); //CTS
    PA[10].configure(None); //GPIO
    PA[11].configure(Some(A)); //Tx
    PA[12].configure(Some(A)); //Rx
    PA[13].configure(None); //Reset
    PA[14].configure(None); //Power signal
    //gsm/multipurpose uart out
    PA[15].configure(Some(A)); //aux tx
    PA[16].configure(Some(A)); //aux rx

    //shared signals
    PA[21].configure(Some(A)); //Miso
    PA[22].configure(Some(A)); //Mosi
    PA[23].configure(Some(A)); //Sclk
    PB[12].configure(None); //smbus alert
    PB[14].configure(Some(C))); //sda for smbus
    PB[15].configure(Some(C)); //scl for smbus
}

/*******************************************************************************
 * Main init function
 ******************************************************************************/

#[no_mangle]
pub unsafe fn reset_handler() {
    sam4l::init();

    // Source 32Khz and 1Khz clocks from RC23K (SAM4L Datasheet 11.6.8)
    sam4l::bpm::set_ck32source(sam4l::bpm::CK32Source::RC32K);

    set_pin_primary_functions();

    //
    // UART console
    //
    let console = static_init!(
        Console<usart::USART>,
        Console::new(&usart::USART1,
                     &mut console::WRITE_BUF,
                     kernel::Container::create()),
        24);
    usart::USART1.set_client(console);

    //
    // Timer
    //
    let ast = &sam4l::ast::AST;

    let mux_alarm = static_init!(
        MuxAlarm<'static, sam4l::ast::Ast>,
        MuxAlarm::new(&sam4l::ast::AST),
        16);
    ast.configure(mux_alarm);

    let virtual_alarm1 = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm),
        24);
    let timer = static_init!(
        TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        TimerDriver::new(virtual_alarm1, kernel::Container::create()),
        12);
    virtual_alarm1.set_client(timer);

    //
    // I2C Buses
    //
    //some declaration of an i2c slave that I don't know how to do yet

    let i2c_mux_smbus = static_init!(capsules::virtual_i2c::MuxI2C<'static>, capsules::virtual_i2c::MuxI2C::new(&sam4l::i2c::I2C3), 20);
    sam4l::i2c::I2C3.set_client(i2c_mux_smbus);

    //
    // I2C Selectors.
    //
    let pca9544a_0_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x70),
        32);
    let pca9544a_0 = static_init!(
        signpost_drivers::pca9544a::PCA9544A<'static>,
        signpost_drivers::pca9544a::PCA9544A::new(pca9544a_0_i2c, None, &mut signpost_drivers::pca9544a::BUFFER),
        320/8);
    pca9544a_0_i2c.set_client(pca9544a_0);

    // This provides the common interface to the I2C selectors
    let i2c_selector = static_init!(
        signpost_drivers::i2c_selector::I2CSelector<'static, signpost_drivers::pca9544a::PCA9544A<'static>>,
        signpost_drivers::i2c_selector::I2CSelector::new(i2c_selectors),
        228/8
    );
    for (i, selector) in i2c_selectors.iter().enumerate() {
        selector.set_client(i2c_selector, i);
    }

    //
    // Coulomb counter
    //

    // Setup the driver for the coulomb counter. We only use one because
    // they all share the same address, so one driver can be used for any
    // of them based on which port is selected on the i2c selector.
    let ltc2941_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x64),
        32);
    let ltc2941 = static_init!(
        signpost_drivers::ltc2941::LTC2941<'static>,
        signpost_drivers::ltc2941::LTC2941::new(ltc2941_i2c, None, &mut signpost_drivers::ltc2941::BUFFER),
        288/8);
    ltc2941_i2c.set_client(ltc2941);

    // Create the object that provides an interface for the coulomb counter
    // for applications.
    let ltc2941_driver = static_init!(
        signpost_drivers::ltc2941::LTC2941Driver<'static>,
        signpost_drivers::ltc2941::LTC2941Driver::new(ltc2941),
        128/8);
    ltc2941.set_client(ltc2941_driver);

    /*
    XXX: Needs to be changed to the USART SPI implementation
    //
    // SPI
    //
    let mux_spi = static_init!(
        capsules::virtual_spi::MuxSPIMaster<'static>,
        capsules::virtual_spi::MuxSPIMaster::new(&sam4l::spi::SPI),
        128/8);
    sam4l::spi::SPI.set_client(mux_spi);
    sam4l::spi::SPI.init();

    //
    // Remaining GPIO pins
    //
    let gpio_pins = static_init!(
        [&'static sam4l::gpio::GPIOPin; 14],
        [&sam4l::gpio::PA[25],  // CONTROLLER_LED
         &sam4l::gpio::PA[04],  // MOD0_IN
         &sam4l::gpio::PA[05],  // MOD1_IN
         &sam4l::gpio::PA[06],  // MOD2_IN
         &sam4l::gpio::PA[07],  // MOD5_IN
         &sam4l::gpio::PA[08],  // MOD6_IN
         &sam4l::gpio::PA[09],  // MOD7_IN
         &sam4l::gpio::PA[13],  // MOD0_OUT
         &sam4l::gpio::PA[14],  // MOD1_OUT
         &sam4l::gpio::PA[15],  // MOD2_OUT
         &sam4l::gpio::PA[16],  // MOD5_OUT
         &sam4l::gpio::PA[17],  // MOD6_OUT
         &sam4l::gpio::PA[18],  // MOD7_OUT
         &sam4l::gpio::PA[26]], // !SMBALERT
        14 * 4
    );
    let gpio = static_init!(
        capsules::gpio::GPIO<'static, sam4l::gpio::GPIOPin>,
        capsules::gpio::GPIO::new(gpio_pins),
        20);
    for pin in gpio_pins.iter() {
        pin.set_client(gpio);
    }


    //
    // Actual platform object
    //
    let signpost_controller = static_init!(
        SignpostController,
        SignpostController {
            console: console,
            gpio: gpio,
            timer: timer,
            coulomb_counter_i2c_selector: i2c_selector,
            coulomb_counter_generic: ltc2941_driver,
        },
        192/8);

    usart::USART1.configure(usart::USARTParams {
        baud_rate: 115200,
        data_bits: 8,
        parity: kernel::hil::uart::Parity::None,
        mode: kernel::hil::uart::Mode::Normal,
    });

    signpost_controller.console.initialize();

    let mut chip = sam4l::chip::Sam4l::new();
    chip.mpu().enable_mpu();

    kernel::main(signpost_controller, &mut chip, load_processes());
}
