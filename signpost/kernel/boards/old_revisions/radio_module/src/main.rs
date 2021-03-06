#![crate_name = "radio_module"]
#![no_std]
#![no_main]
#![feature(const_fn,lang_items)]

extern crate cortexm4;
extern crate capsules;
#[macro_use(debug,static_init)]
extern crate kernel;
extern crate sam4l;

extern crate signpost_drivers;
extern crate signpost_hil;

use capsules::console::{self, Console};
use capsules::nrf51822_serialization::{self, Nrf51822Serialization};
use capsules::timer::TimerDriver;
use capsules::virtual_alarm::{MuxAlarm, VirtualMuxAlarm};
use kernel::hil;
use kernel::hil::Controller;
use kernel::{Chip, Platform};
use kernel::mpu::MPU;
use sam4l::usart;

// For panic!()
#[macro_use]
pub mod io;
pub mod version;


unsafe fn load_processes() -> &'static mut [Option<kernel::process::Process<'static>>] {
    extern "C" {
        /// Beginning of the ROM region containing app images.
        static _sapps: u8;
    }

    const NUM_PROCS: usize = 2;

    // how should the kernel respond when a process faults
    const FAULT_RESPONSE: kernel::process::FaultResponse = kernel::process::FaultResponse::Panic;
    #[link_section = ".app_memory"]
    static mut APP_MEMORY: [u8; 16384*2] = [0; 16384*2];

    static mut PROCESSES: [Option<kernel::process::Process<'static>>; NUM_PROCS] = [None, None];

    let mut apps_in_flash_ptr = &_sapps as *const u8;
    let mut app_memory_ptr = APP_MEMORY.as_mut_ptr();
    let mut app_memory_size = APP_MEMORY.len();
    for i in 0..NUM_PROCS {
        let (process, flash_offset, memory_offset) =
            kernel::process::Process::create(apps_in_flash_ptr,
                                             app_memory_ptr,
                                             app_memory_size,
                                             FAULT_RESPONSE);

        if process.is_none() {
            break;
        }

        PROCESSES[i] = process;
        apps_in_flash_ptr = apps_in_flash_ptr.offset(flash_offset as isize);
        app_memory_ptr = app_memory_ptr.offset(memory_offset as isize);
        app_memory_size -= memory_offset;
    }

    &mut PROCESSES
}

/*******************************************************************************
 * Setup this platform
 ******************************************************************************/

struct RadioModule {
    console: &'static Console<'static, usart::USART>,
    gpio: &'static capsules::gpio::GPIO<'static, sam4l::gpio::GPIOPin>,
    timer: &'static TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast<'static>>>,
    i2c_master_slave: &'static capsules::i2c_master_slave_driver::I2CMasterSlaveDriver<'static>,
    nrf51822: &'static Nrf51822Serialization<'static, usart::USART>,
    app_watchdog: &'static signpost_drivers::app_watchdog::AppWatchdog<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast<'static>>>,
    rng: &'static capsules::rng::SimpleRng<'static, sam4l::trng::Trng<'static>>,
    ipc: kernel::ipc::IPC,
}

impl Platform for RadioModule {
    fn with_driver<F, R>(&self, driver_num: usize, f: F) -> R
        where F: FnOnce(Option<&kernel::Driver>) -> R
    {

        match driver_num {
            0 => f(Some(self.console)),
            1 => f(Some(self.gpio)),
            3 => f(Some(self.timer)),
            5 => f(Some(self.nrf51822)),
            13 => f(Some(self.i2c_master_slave)),
            14 => f(Some(self.rng)),

            108 => f(Some(self.app_watchdog)),

            0xff => f(Some(&self.ipc)),
            _ => f(None)
        }
    }
}


unsafe fn set_pin_primary_functions() {
    use sam4l::gpio::{PA,PB};
    use sam4l::gpio::PeripheralFunction::{A, C};

    //backplane communication
    PB[00].configure(Some(A)); //SDA
    PB[01].configure(Some(A)); //SCL
    PB[04].configure(None); //PPS
    PB[03].configure(None); //MOD_OUT
    PB[05].configure(None); //MOD_IN
    PA[25].configure(Some(A)); //USB
    PA[26].configure(Some(A)); //USB


    //Nucleum Signals
    PB[02].configure(None); //Nucleum Reset
    PB[06].configure(None); //RTS
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
    PB[14].configure(Some(C)); //sda for smbus
    PB[15].configure(Some(C)); //scl for smbus
}

/*******************************************************************************
 * Main init function
 ******************************************************************************/

#[no_mangle]
pub unsafe fn reset_handler() {
    sam4l::init();

    sam4l::pm::setup_system_clock(sam4l::pm::SystemClockSource::ExternalOscillator, 16000000);

    // Source 32Khz and 1Khz clocks from RC23K (SAM4L Datasheet 11.6.8)
    sam4l::bpm::set_ck32source(sam4l::bpm::CK32Source::RC32K);

    set_pin_primary_functions();

    //
    // UART console
    //
    let console = static_init!(
        Console<usart::USART>,
        Console::new(&usart::USART2,
                     115200,
                     &mut console::WRITE_BUF,
                     kernel::Container::create()),
        224/8);
    hil::uart::UART::set_client(&usart::USART2, console);

    let nrf_serialization = static_init!(
        Nrf51822Serialization<usart::USART>,
        Nrf51822Serialization::new(&usart::USART3,
                &mut nrf51822_serialization::WRITE_BUF,
                &mut nrf51822_serialization::READ_BUF),
        608/8);
    hil::uart::UART::set_client(&usart::USART3, nrf_serialization);

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

    // Setup RNG
    let rng = static_init!(
            capsules::rng::SimpleRng<'static, sam4l::trng::Trng>,
            capsules::rng::SimpleRng::new(&sam4l::trng::TRNG, kernel::Container::create()),
            96/8);
    sam4l::trng::TRNG.set_client(rng);

    //
    // I2C Buses
    //
    let i2c_modules = static_init!(
        capsules::i2c_master_slave_driver::I2CMasterSlaveDriver<'static>,
        capsules::i2c_master_slave_driver::I2CMasterSlaveDriver::new(&sam4l::i2c::I2C1,
            &mut capsules::i2c_master_slave_driver::BUFFER1,
            &mut capsules::i2c_master_slave_driver::BUFFER2,
            &mut capsules::i2c_master_slave_driver::BUFFER3),
        864/8);
    sam4l::i2c::I2C1.set_master_client(i2c_modules);
    sam4l::i2c::I2C1.set_slave_client(i2c_modules);

    hil::i2c::I2CSlave::set_address(&sam4l::i2c::I2C1, 0x22);

    //
    // Remaining GPIO pins
    //
    let gpio_pins = static_init!(
        [&'static sam4l::gpio::GPIOPin; 15],
        [&sam4l::gpio::PB[03], //MOD_OUT
         &sam4l::gpio::PB[05], //MOD_IN
         &sam4l::gpio::PB[04], //PPS
         &sam4l::gpio::PB[02], //NUCLEUM RESET
         &sam4l::gpio::PB[08], //NUCLEUM BOOT
         &sam4l::gpio::PB[11], //NUCLEUM POWERGATE
         &sam4l::gpio::PA[04], //LORA INT1
         &sam4l::gpio::PA[05], //LORA INT2
         &sam4l::gpio::PA[06], //LORA POWERGATE
         &sam4l::gpio::PA[17], //LORA RESET
         &sam4l::gpio::PA[18], //LORA BOOT
         &sam4l::gpio::PA[07], //GSM POWERGATE
         &sam4l::gpio::PA[10], //GSM GPIO
         &sam4l::gpio::PA[13], //GSM RESET
         &sam4l::gpio::PA[14]],//GSM POWER
        15 * 4
    );
    let gpio = static_init!(
        capsules::gpio::GPIO<'static, sam4l::gpio::GPIOPin>,
        capsules::gpio::GPIO::new(gpio_pins),
        224/8);
    for pin in gpio_pins.iter() {
        pin.set_client(gpio);
    }

    //
    // App Watchdog
    //
    let app_timeout_alarm = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm),
        24);
    let kernel_timeout_alarm = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm),
        24);
    let app_timeout = static_init!(
        signpost_drivers::app_watchdog::Timeout<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::app_watchdog::Timeout::new(app_timeout_alarm, signpost_drivers::app_watchdog::TimeoutMode::App, 1000, cortexm4::scb::reset),
        128/8);
    app_timeout_alarm.set_client(app_timeout);
    let kernel_timeout = static_init!(
        signpost_drivers::app_watchdog::Timeout<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::app_watchdog::Timeout::new(kernel_timeout_alarm, signpost_drivers::app_watchdog::TimeoutMode::Kernel, 5000, cortexm4::scb::reset),
        128/8);
    kernel_timeout_alarm.set_client(kernel_timeout);
    let app_watchdog = static_init!(
        signpost_drivers::app_watchdog::AppWatchdog<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::app_watchdog::AppWatchdog::new(app_timeout, kernel_timeout),
        64/8);

    //
    // Kernel Watchdog
    //
    let watchdog_alarm = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm),
        24);
    let watchdog = static_init!(
        signpost_drivers::watchdog_kernel::WatchdogKernel<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::watchdog_kernel::WatchdogKernel::new(watchdog_alarm, &sam4l::wdt::WDT, 1200),
        128/8);
    watchdog_alarm.set_client(watchdog);

    //
    // Actual platform object
    //
    let radio_module = RadioModule {
        console: console,
        gpio: gpio,
        timer: timer,
        i2c_master_slave: i2c_modules,
        nrf51822:nrf_serialization,
        app_watchdog: app_watchdog,
        rng: rng,
        ipc: kernel::ipc::IPC::new(),
    };

    let kc = static_init!(
        capsules::console::App,
        capsules::console::App::default(),
        480/8);
    kernel::debug::assign_console_driver(Some(radio_module.console), kc);
    watchdog.start();

    //fix the rst line
    sam4l::gpio::PB[06].enable();
    sam4l::gpio::PB[06].enable_output();
    sam4l::gpio::PB[06].clear();

    //turn off gsm power
    sam4l::gpio::PA[07].enable();
    sam4l::gpio::PA[07].enable_output();
    sam4l::gpio::PA[07].set();

    radio_module.console.initialize();
    radio_module.nrf51822.initialize();
    watchdog.start();

    let mut chip = sam4l::chip::Sam4l::new();
    chip.mpu().enable_mpu();

    debug!("Running {} Version {} from git {}",
           env!("CARGO_PKG_NAME"),
           env!("CARGO_PKG_VERSION"),
           version::GIT_VERSION,
           );
    kernel::main(&radio_module, &mut chip, load_processes(), &radio_module.ipc);
}
