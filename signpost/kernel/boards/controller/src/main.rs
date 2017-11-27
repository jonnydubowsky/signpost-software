#![crate_name = "controller"]
#![no_std]
#![no_main]
#![feature(asm,compiler_builtins_lib,const_fn,drop_types_in_const,lang_items)]

extern crate capsules;
extern crate compiler_builtins;
extern crate cortexm4;
#[macro_use(debug,static_init)]
extern crate kernel;
extern crate sam4l;

extern crate signpost_drivers;
extern crate signpost_hil;

use signpost_drivers::gps_console;
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

// Number of concurrent processes this platform supports.
const NUM_PROCS: usize = 2;

// How should the kernel respond when a process faults.
const FAULT_RESPONSE: kernel::process::FaultResponse = kernel::process::FaultResponse::Panic;

#[link_section = ".app_memory"]
static mut APP_MEMORY: [u8; 16384*2] = [0; 16384*2];

// Actual memory for holding the active process structures.
static mut PROCESSES: [Option<kernel::Process<'static>>; NUM_PROCS] = [None, None];

/*******************************************************************************
 * Setup this platform
 ******************************************************************************/

struct SignpostController {
    console: &'static capsules::console::Console<'static, usart::USART>,
    gps_console: &'static signpost_drivers::gps_console::Console<'static, usart::USART>,
    gpio: &'static capsules::gpio::GPIO<'static, sam4l::gpio::GPIOPin>,
    led: &'static capsules::led::LED<'static, sam4l::gpio::GPIOPin>,
    timer: &'static TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast<'static>>>,
    bonus_timer: &'static TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast<'static>>>,
    smbus_interrupt: &'static signpost_drivers::smbus_interrupt::SMBUSIntDriver<'static>,
    gpio_async: &'static capsules::gpio_async::GPIOAsync<'static, capsules::mcp23008::MCP23008<'static>>,
    coulomb_counter_i2c_mux_0: &'static capsules::pca9544a::PCA9544A<'static>,
    coulomb_counter_i2c_mux_1: &'static capsules::pca9544a::PCA9544A<'static>,
    coulomb_counter_i2c_mux_2: &'static capsules::pca9544a::PCA9544A<'static>,
    coulomb_counter_generic: &'static capsules::ltc294x::LTC294XDriver<'static>,
    battery_monitor: &'static signpost_drivers::max17205::MAX17205Driver<'static>,
    nonvolatile_storage: &'static capsules::nonvolatile_storage_driver::NonvolatileStorage<'static>,
    i2c_master_slave: &'static capsules::i2c_master_slave_driver::I2CMasterSlaveDriver<'static>,
    app_watchdog: &'static signpost_drivers::app_watchdog::AppWatchdog<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast<'static>>>,
    rng: &'static capsules::rng::SimpleRng<'static, sam4l::trng::Trng<'static>>,
    app_flash: &'static capsules::app_flash_driver::AppFlash<'static>,
    stfu: &'static signpost_drivers::signpost_tock_firmware_update::SignpostTockFirmwareUpdate<'static,
        capsules::virtual_flash::FlashUser<'static, sam4l::flashcalw::FLASHCALW>>,
    stfu_holding: &'static capsules::nonvolatile_storage_driver::NonvolatileStorage<'static>,
    ipc: kernel::ipc::IPC,
}

impl Platform for SignpostController {
    fn with_driver<F, R>(&self, driver_num: usize, f: F) -> R
        where F: FnOnce(Option<&kernel::Driver>) -> R
    {

        match driver_num {
            0 => f(Some(self.console)),
            1 => f(Some(self.gpio)),
            3 => f(Some(self.timer)),
            8 => f(Some(self.led)),
            13 => f(Some(self.i2c_master_slave)),
            14 => f(Some(self.rng)),
            18 => f(Some(self.coulomb_counter_generic)),
            20 => f(Some(self.gpio_async)),
            27 => f(Some(self.nonvolatile_storage)),
            30 => f(Some(self.app_flash)),

            1001 => f(Some(self.coulomb_counter_i2c_mux_0)),
            1002 => f(Some(self.coulomb_counter_i2c_mux_1)),
            1003 => f(Some(self.coulomb_counter_i2c_mux_2)),
            110 => f(Some(self.battery_monitor)),
            104 => f(Some(self.smbus_interrupt)),
            108 => f(Some(self.app_watchdog)),
            109 => f(Some(self.gps_console)),

            120 => f(Some(self.stfu)),
            121 => f(Some(self.stfu_holding)),

            203 => f(Some(self.bonus_timer)),

            0xff => f(Some(&self.ipc)),
            _ => f(None)
        }
    }
}


unsafe fn set_pin_primary_functions() {
    use sam4l::gpio::{PA, PB};
    use sam4l::gpio::PeripheralFunction::{A, B, E};

    // GPIO: signal from modules
    PA[04].configure(None); // MOD0_IN
    PA[05].configure(None); // MOD1_IN
    PA[06].configure(None); // MOD2_IN
    PB[09].configure(None); // STORAGE_IN
    PA[07].configure(None); // MOD5_IN
    PA[08].configure(None); // MOD6_IN
    PA[09].configure(None); // MOD7_IN

    // GPIO: signal to modules
    PA[13].configure(None); // MOD0_OUT
    PA[14].configure(None); // MOD1_OUT
    PA[15].configure(None); // MOD2_OUT
    PB[10].configure(None); // STORAGE_OUT
    PA[16].configure(None); // MOD5_OUT
    PA[17].configure(None); // MOD6_OUT
    PA[18].configure(None); // MOD7_OUT

    // SPI: Master of Storage Master & FRAM - USART0
    PA[10].configure(Some(A)); // MEMORY_SCLK
    PA[11].configure(Some(A)); // MEMORY_MISO
    PA[12].configure(Some(A)); // MEMORY_MOSI
    PB[13].configure(None); // !STORAGE_CS
    PA[13].enable();
    PA[13].set();
    PA[13].enable_output();
    PA[25].configure(None); // !FRAM_CS
    PA[25].enable();
    PA[25].set();
    PA[25].enable_output();

    // UART: GPS - USART2
    PA[19].configure(Some(A)); // GPS_OUT_TX
    PA[20].configure(Some(A)); // GPS_IN_RX
    PB[07].configure(None); // GPS_ENABLE_POWER, Turn on GPS to start
    PB[07].enable();
    PB[07].set();
    PB[07].enable_output();

    // SMBus: Power / Backplane - TWIM2
    PA[21].configure(Some(E)); // SMBDATA
    PA[22].configure(Some(E)); // SMBCLK
    PA[26].configure(None); // !SMBALERT

    // I2C: Modules - TWIMS0
    PA[23].configure(Some(B)); // MODULES_SDA
    PA[24].configure(Some(B)); // MODULES_SCL

    // UART: Debug - USART1
    PB[04].configure(Some(B)); // CONTROLLER_DEBUG_RX
    PB[05].configure(Some(B)); // CONTROLLER_DEBUG_TX

    // GPIO: Debug
    PB[11].configure(None); // !CONTROLLER_LED
    PB[14].configure(None); // CONTROLLER_DEBUG_GPIO1
    PB[15].configure(None); // CONTROLLER_DEBUG_GPIO2
}

/*******************************************************************************
 * Main init function
 ******************************************************************************/

#[no_mangle]
pub unsafe fn reset_handler() {
    sam4l::init();

    // Setup clock
    sam4l::pm::PM.setup_system_clock(sam4l::pm::SystemClockSource::PllExternalOscillatorAt48MHz {
        frequency: sam4l::pm::OscillatorFrequency::Frequency16MHz,
        startup_mode: sam4l::pm::OscillatorStartup::SlowStart,
    });

    // Source 32Khz and 1Khz clocks from RC23K (SAM4L Datasheet 11.6.8)
    sam4l::bpm::set_ck32source(sam4l::bpm::CK32Source::RC32K);

    set_pin_primary_functions();

    // UART console
    let console = static_init!(
        capsules::console::Console<usart::USART>,
        capsules::console::Console::new(&usart::USART1,
                     115200,
                     &mut capsules::console::WRITE_BUF,
                     kernel::Container::create()));
    hil::uart::UART::set_client(&usart::USART1, console);

    //
    // GPS console
    //
    let gps_console = static_init!(
        signpost_drivers::gps_console::Console<usart::USART>,
        signpost_drivers::gps_console::Console::new(&usart::USART2,
                     9600,
                     &mut gps_console::WRITE_BUF,
                     &mut gps_console::READ_BUF,
                     kernel::Container::create()));
    hil::uart::UART::set_client(&usart::USART2, gps_console);

    //
    // Timer
    //
    let ast = &sam4l::ast::AST;

    let mux_alarm = static_init!(
        MuxAlarm<'static, sam4l::ast::Ast>,
        MuxAlarm::new(&sam4l::ast::AST));
    ast.configure(mux_alarm);

    let virtual_alarm1 = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm),
        24);
    let timer = static_init!(
        TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        TimerDriver::new(virtual_alarm1, kernel::Container::create()));
    virtual_alarm1.set_client(timer);


    let virtual_alarm2 = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm));
    let bonus_timer = static_init!(
        TimerDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        TimerDriver::new(virtual_alarm2, kernel::Container::create()));
    virtual_alarm2.set_client(bonus_timer);

    // Setup RNG
    let rng = static_init!(
            capsules::rng::SimpleRng<'static, sam4l::trng::Trng>,
            capsules::rng::SimpleRng::new(&sam4l::trng::TRNG, kernel::Container::create()));
    sam4l::trng::TRNG.set_client(rng);

    // Nonvolatile Pages
    pub static mut PAGEBUFFER: sam4l::flashcalw::Sam4lPage = sam4l::flashcalw::Sam4lPage::new();
    let nv_to_page = static_init!(
        capsules::nonvolatile_to_pages::NonvolatileToPages<'static, sam4l::flashcalw::FLASHCALW>,
        capsules::nonvolatile_to_pages::NonvolatileToPages::new(
            &mut sam4l::flashcalw::FLASH_CONTROLLER,
            &mut PAGEBUFFER));
    hil::flash::HasClient::set_client(&sam4l::flashcalw::FLASH_CONTROLLER, nv_to_page);

    // App Flash
    pub static mut APP_FLASH_BUFFER: [u8; 512] = [0; 512];
    let app_flash = static_init!(
        capsules::app_flash_driver::AppFlash<'static>,
        capsules::app_flash_driver::AppFlash::new(nv_to_page,
            kernel::Container::create(), &mut APP_FLASH_BUFFER));
    hil::nonvolatile_storage::NonvolatileStorage::set_client(nv_to_page, app_flash);
    sam4l::flashcalw::FLASH_CONTROLLER.configure();

    //
    // I2C Buses
    //
    let i2c_modules = static_init!(
        capsules::i2c_master_slave_driver::I2CMasterSlaveDriver<'static>,
        capsules::i2c_master_slave_driver::I2CMasterSlaveDriver::new(&sam4l::i2c::I2C0,
            &mut capsules::i2c_master_slave_driver::BUFFER1,
            &mut capsules::i2c_master_slave_driver::BUFFER2,
            &mut capsules::i2c_master_slave_driver::BUFFER3));
    sam4l::i2c::I2C0.set_master_client(i2c_modules);
    sam4l::i2c::I2C0.set_slave_client(i2c_modules);

    // Set I2C slave address here, because it is board specific and not app
    // specific. It can be overridden in the app, of course.
    hil::i2c::I2CSlave::set_address(&sam4l::i2c::I2C0, 0x20);

    let i2c_mux_smbus = static_init!(
        capsules::virtual_i2c::MuxI2C<'static>,
        capsules::virtual_i2c::MuxI2C::new(&sam4l::i2c::I2C2));
    sam4l::i2c::I2C2.set_master_client(i2c_mux_smbus);

    //
    // SMBUS INTERRUPT
    //

    let smbusint_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x0C));

    let smbusint = static_init!(
        signpost_drivers::smbus_interrupt::SMBUSInterrupt<'static>,
        // Make sure to replace "None" below with gpio used as SMBUS Alert
        // Some(&sam4l::gpio::PA[16]) for instance
        signpost_drivers::smbus_interrupt::SMBUSInterrupt::new(smbusint_i2c, None, &mut signpost_drivers::smbus_interrupt::BUFFER));

    smbusint_i2c.set_client(smbusint);
    // Make sure to set smbusint as client for chosen gpio for SMBUS Alert
    // &sam4l::gpio::PA[16].set_client(smbusint); for instance

    let smbusint_driver = static_init!(
        signpost_drivers::smbus_interrupt::SMBUSIntDriver<'static>,
        signpost_drivers::smbus_interrupt::SMBUSIntDriver::new(smbusint));
    smbusint.set_client(smbusint_driver);

    //
    // GPIO EXTENDERS
    //

    // Configure the MCP23008_0. Device address 0x20
    let mcp23008_0_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x20));
    let mcp23008_0 = static_init!(
        capsules::mcp23008::MCP23008<'static>,
        capsules::mcp23008::MCP23008::new(mcp23008_0_i2c, None, &mut capsules::mcp23008::BUFFER));
    mcp23008_0_i2c.set_client(mcp23008_0);

    // Configure the MCP23008_1. Device address 0x21
    let mcp23008_1_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x21));
    let mcp23008_1 = static_init!(
        capsules::mcp23008::MCP23008<'static>,
        capsules::mcp23008::MCP23008::new(mcp23008_1_i2c, None, &mut capsules::mcp23008::BUFFER));
    mcp23008_1_i2c.set_client(mcp23008_1);

    // Configure the MCP23008_2. Device address 0x22
    let mcp23008_2_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x22));
    let mcp23008_2 = static_init!(
        capsules::mcp23008::MCP23008<'static>,
        capsules::mcp23008::MCP23008::new(mcp23008_2_i2c, None, &mut capsules::mcp23008::BUFFER));
    mcp23008_2_i2c.set_client(mcp23008_2);

    // Configure the MCP23008_5. Device address 0x25
    let mcp23008_5_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x25));
    let mcp23008_5 = static_init!(
        capsules::mcp23008::MCP23008<'static>,
        capsules::mcp23008::MCP23008::new(mcp23008_5_i2c, None, &mut capsules::mcp23008::BUFFER));
    mcp23008_5_i2c.set_client(mcp23008_5);

    // Configure the MCP23008_6. Device address 0x26
    let mcp23008_6_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x26));
    let mcp23008_6 = static_init!(
        capsules::mcp23008::MCP23008<'static>,
        capsules::mcp23008::MCP23008::new(mcp23008_6_i2c, None, &mut capsules::mcp23008::BUFFER));
    mcp23008_6_i2c.set_client(mcp23008_6);

    // Configure the MCP23008_7. Device address 0x27
    let mcp23008_7_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x27));
    let mcp23008_7 = static_init!(
        capsules::mcp23008::MCP23008<'static>,
        capsules::mcp23008::MCP23008::new(mcp23008_7_i2c, None, &mut capsules::mcp23008::BUFFER));
    mcp23008_7_i2c.set_client(mcp23008_7);

    // Create an array of the GPIO extenders so we can pass them to an
    // administrative layer that provides a single interface to them all.
    let async_gpio_ports = static_init!(
        [&'static capsules::mcp23008::MCP23008; 6],
        [mcp23008_0, // Port 0
         mcp23008_1, // Port 1
         mcp23008_2, // Port 2
         mcp23008_5, // Port 3
         mcp23008_6, // Port 4
         mcp23008_7] // Port 5
    );

    // `gpio_async` is the object that manages all of the extenders
    let gpio_async = static_init!(
        capsules::gpio_async::GPIOAsync<'static, capsules::mcp23008::MCP23008<'static>>,
        capsules::gpio_async::GPIOAsync::new(async_gpio_ports));
    for port in async_gpio_ports.iter() {
        port.set_client(gpio_async);
    }

    //
    // I2C Selectors.
    //
    let pca9544a_0_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x70));
    let pca9544a_0 = static_init!(
        capsules::pca9544a::PCA9544A<'static>,
        capsules::pca9544a::PCA9544A::new(pca9544a_0_i2c, &mut capsules::pca9544a::BUFFER));
    pca9544a_0_i2c.set_client(pca9544a_0);

    let pca9544a_1_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x71));
    let pca9544a_1 = static_init!(
        capsules::pca9544a::PCA9544A<'static>,
        capsules::pca9544a::PCA9544A::new(pca9544a_1_i2c, &mut capsules::pca9544a::BUFFER));
    pca9544a_1_i2c.set_client(pca9544a_1);

    let pca9544a_2_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x72));
    let pca9544a_2 = static_init!(
        capsules::pca9544a::PCA9544A<'static>,
        capsules::pca9544a::PCA9544A::new(pca9544a_2_i2c, &mut capsules::pca9544a::BUFFER));
    pca9544a_2_i2c.set_client(pca9544a_2);


    //
    // Coulomb counter
    //

    // Setup the driver for the coulomb counter. We only use one because
    // they all share the same address, so one driver can be used for any
    // of them based on which port is selected on the i2c selector.
    let ltc294x_i2c = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x64));
    let ltc294x = static_init!(
        capsules::ltc294x::LTC294X<'static>,
        capsules::ltc294x::LTC294X::new(ltc294x_i2c, None, &mut capsules::ltc294x::BUFFER));
    ltc294x_i2c.set_client(ltc294x);

    // Create the object that provides an interface for the coulomb counter
    // for applications.
    let ltc294x_driver = static_init!(
        capsules::ltc294x::LTC294XDriver<'static>,
        capsules::ltc294x::LTC294XDriver::new(ltc294x));
    ltc294x.set_client(ltc294x_driver);

    //
    // Battery Monitor
    //

    // Setup driver for battery monitor
    // We use two i2c addresses in order to address the full range of the
    // monitor's memory
    let max17205_i2c0 = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x36));
    let max17205_i2c1 = static_init!(
        capsules::virtual_i2c::I2CDevice,
        capsules::virtual_i2c::I2CDevice::new(i2c_mux_smbus, 0x0B));
    let max17205 = static_init!(
        signpost_drivers::max17205::MAX17205<'static>,
        signpost_drivers::max17205::MAX17205::new(max17205_i2c0, max17205_i2c1, &mut signpost_drivers::max17205::BUFFER));
    max17205_i2c0.set_client(max17205);
    max17205_i2c1.set_client(max17205);

    // Create the object that provides an interface for the battery monitor
    // for applications.
    let max17205_driver = static_init!(
        signpost_drivers::max17205::MAX17205Driver<'static>,
        signpost_drivers::max17205::MAX17205Driver::new(max17205));
    max17205.set_client(max17205_driver);

    //
    // SPI - Shared by FRAM and Storage Master
    //
    let mux_spi = static_init!(
        capsules::virtual_spi::MuxSpiMaster<'static, usart::USART>,
        capsules::virtual_spi::MuxSpiMaster::new(&sam4l::usart::USART0));
    // sam4l::spi::SPI.set_client(mux_spi);
    // sam4l::spi::SPI.init();
    hil::spi::SpiMaster::set_client(&sam4l::usart::USART0, mux_spi);
    hil::spi::SpiMaster::init(&sam4l::usart::USART0);

    //
    // FRAM
    //
    let fm25cl_spi = static_init!(
        capsules::virtual_spi::VirtualSpiMasterDevice<'static, usart::USART>,
        // capsules::virtual_spi::VirtualSpiMasterDevice::new(mux_spi, k),
        capsules::virtual_spi::VirtualSpiMasterDevice::new(mux_spi, Some(&sam4l::gpio::PA[25])));
    let fm25cl = static_init!(
        capsules::fm25cl::FM25CL<'static, capsules::virtual_spi::VirtualSpiMasterDevice<'static, usart::USART>>,
        capsules::fm25cl::FM25CL::new(fm25cl_spi, &mut capsules::fm25cl::TXBUFFER, &mut capsules::fm25cl::RXBUFFER));
    fm25cl_spi.set_client(fm25cl);
    // Interface for applications
    let nonvolatile_storage = static_init!(
        capsules::nonvolatile_storage_driver::NonvolatileStorage<'static>,
        capsules::nonvolatile_storage_driver::NonvolatileStorage::new(
            fm25cl, kernel::Container::create(),
            0,      // Start address for userspace accessible region
            8*1024, // Length of userspace accessible region
            8*1024, // Start address of kernel accessible region
            0,      // Length of kernel accessible region
            &mut capsules::nonvolatile_storage_driver::BUFFER));
    hil::nonvolatile_storage::NonvolatileStorage::set_client(fm25cl, nonvolatile_storage);

    //
    // App Watchdog
    //
    let app_timeout_alarm = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm));
    let kernel_timeout_alarm = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm));
    let app_timeout = static_init!(
        signpost_drivers::app_watchdog::Timeout<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::app_watchdog::Timeout::new(app_timeout_alarm, signpost_drivers::app_watchdog::TimeoutMode::App, 1000, cortexm4::scb::reset));
    app_timeout_alarm.set_client(app_timeout);
    let kernel_timeout = static_init!(
        signpost_drivers::app_watchdog::Timeout<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::app_watchdog::Timeout::new(kernel_timeout_alarm, signpost_drivers::app_watchdog::TimeoutMode::Kernel, 5000, cortexm4::scb::reset));
    kernel_timeout_alarm.set_client(kernel_timeout);
    let app_watchdog = static_init!(
        signpost_drivers::app_watchdog::AppWatchdog<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::app_watchdog::AppWatchdog::new(app_timeout, kernel_timeout));

    //
    // Kernel Watchdog
    //
    let watchdog_alarm = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm),
        24);
    let watchdog = static_init!(
        signpost_drivers::watchdog_kernel::WatchdogKernel<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        signpost_drivers::watchdog_kernel::WatchdogKernel::new(watchdog_alarm, &sam4l::wdt::WDT, 1200));
    watchdog_alarm.set_client(watchdog);

    //
    // Remaining GPIO pins
    //
    let gpio_pins = static_init!(
        [&'static sam4l::gpio::GPIOPin; 15],
         [&sam4l::gpio::PA[04], // MOD0_IN
         &sam4l::gpio::PA[05],  // MOD1_IN
         &sam4l::gpio::PA[06],  // MOD2_IN
         &sam4l::gpio::PB[09],  // STORAGE_IN
         &sam4l::gpio::PA[07],  // MOD5_IN
         &sam4l::gpio::PA[08],  // MOD6_IN
         &sam4l::gpio::PA[09],  // MOD7_IN
         &sam4l::gpio::PA[13],  // MOD0_OUT
         &sam4l::gpio::PA[14],  // MOD1_OUT
         &sam4l::gpio::PA[15],  // MOD2_OUT
         &sam4l::gpio::PB[10],  // STORAGE_OUT
         &sam4l::gpio::PA[16],  // MOD5_OUT
         &sam4l::gpio::PA[17],  // MOD6_OUT
         &sam4l::gpio::PA[18],  // MOD7_OUT
         &sam4l::gpio::PA[26]]  // !SMBALERT
    );
    let gpio = static_init!(
        capsules::gpio::GPIO<'static, sam4l::gpio::GPIOPin>,
        capsules::gpio::GPIO::new(gpio_pins));
    for pin in gpio_pins.iter() {
        pin.set_client(gpio);
    }

    //
    // LEDs
    //
    let led_pins = static_init!(
        [(&'static sam4l::gpio::GPIOPin, capsules::led::ActivationMode); 3],
        [(&sam4l::gpio::PB[14], capsules::led::ActivationMode::ActiveHigh), // CONTROLLER_DEBUG_GPIO1
         (&sam4l::gpio::PB[15], capsules::led::ActivationMode::ActiveHigh), // CONTROLLER_DEBUG_GPIO2
         (&sam4l::gpio::PB[11], capsules::led::ActivationMode::ActiveLow)]  // !CONTROLLER_LED
        );
    let led = static_init!(
        capsules::led::LED<'static, sam4l::gpio::GPIOPin>,
        capsules::led::LED::new(led_pins));

    // configure initial state for debug LEDs
    sam4l::gpio::PB[14].clear(); // red LED off
    sam4l::gpio::PB[15].set();   // green LED on

    //
    // Flash
    //

    let mux_flash = static_init!(
        capsules::virtual_flash::MuxFlash<'static, sam4l::flashcalw::FLASHCALW>,
        capsules::virtual_flash::MuxFlash::new(&sam4l::flashcalw::FLASH_CONTROLLER));
    hil::flash::HasClient::set_client(&sam4l::flashcalw::FLASH_CONTROLLER, mux_flash);

    //
    // Firmware Update
    //
    let virtual_flash_stfu_holding = static_init!(
        capsules::virtual_flash::FlashUser<'static, sam4l::flashcalw::FLASHCALW>,
        capsules::virtual_flash::FlashUser::new(mux_flash));
    pub static mut STFU_HOLDING_PAGEBUFFER: sam4l::flashcalw::Sam4lPage = sam4l::flashcalw::Sam4lPage::new();

    let stfu_holding_nv_to_page = static_init!(
        capsules::nonvolatile_to_pages::NonvolatileToPages<'static,
            capsules::virtual_flash::FlashUser<'static, sam4l::flashcalw::FLASHCALW>>,
        capsules::nonvolatile_to_pages::NonvolatileToPages::new(
            virtual_flash_stfu_holding,
            &mut STFU_HOLDING_PAGEBUFFER));
    hil::flash::HasClient::set_client(virtual_flash_stfu_holding, stfu_holding_nv_to_page);

    pub static mut STFU_HOLDING_BUFFER: [u8; 512] = [0; 512];
    let stfu_holding = static_init!(
        capsules::nonvolatile_storage_driver::NonvolatileStorage<'static>,
        capsules::nonvolatile_storage_driver::NonvolatileStorage::new(
            stfu_holding_nv_to_page, kernel::Container::create(),
            0x60000, // Start address for userspace accessible region
            0x20000, // Length of userspace accessible region
            0,       // Start address of kernel accessible region
            0,       // Length of kernel accessible region
            &mut STFU_HOLDING_BUFFER));
    hil::nonvolatile_storage::NonvolatileStorage::set_client(stfu_holding_nv_to_page, stfu_holding);


    let virtual_flash_btldrflags = static_init!(
        capsules::virtual_flash::FlashUser<'static, sam4l::flashcalw::FLASHCALW>,
        capsules::virtual_flash::FlashUser::new(mux_flash));
    pub static mut BTLDRPAGEBUFFER: sam4l::flashcalw::Sam4lPage = sam4l::flashcalw::Sam4lPage::new();

    let stfu = static_init!(
        signpost_drivers::signpost_tock_firmware_update::SignpostTockFirmwareUpdate<'static,
            capsules::virtual_flash::FlashUser<'static, sam4l::flashcalw::FLASHCALW>>,
        signpost_drivers::signpost_tock_firmware_update::SignpostTockFirmwareUpdate::new(
            virtual_flash_btldrflags,
            &mut BTLDRPAGEBUFFER));
    hil::flash::HasClient::set_client(virtual_flash_btldrflags, stfu);


    //
    // Actual platform object
    //
    let signpost_controller = SignpostController {
        console: console,
        gps_console: gps_console,
        gpio: gpio,
        led: led,
        timer: timer,
        bonus_timer: bonus_timer,
        gpio_async: gpio_async,
        coulomb_counter_i2c_mux_0: pca9544a_0,
        coulomb_counter_i2c_mux_1: pca9544a_1,
        coulomb_counter_i2c_mux_2: pca9544a_2,
        coulomb_counter_generic: ltc294x_driver,
        battery_monitor: max17205_driver,
        smbus_interrupt: smbusint_driver,
        nonvolatile_storage: nonvolatile_storage,
        i2c_master_slave: i2c_modules,
        app_watchdog: app_watchdog,
        rng: rng,
        app_flash: app_flash,
        stfu: stfu,
        stfu_holding: stfu_holding,
        ipc: kernel::ipc::IPC::new(),
    };

    signpost_controller.console.initialize();
    signpost_controller.gps_console.initialize();
    //watchdog.start();

    // Attach the kernel debug interface to this console
    let kc = static_init!(
        capsules::console::App,
        capsules::console::App::default());
    kernel::debug::assign_console_driver(Some(signpost_controller.console), kc);

    let mut chip = sam4l::chip::Sam4l::new();
    chip.mpu().enable_mpu();

    debug!("Running {} Version {} from git {}",
           env!("CARGO_PKG_NAME"),
           env!("CARGO_PKG_VERSION"),
           version::GIT_VERSION,
           );

    extern "C" {
        /// Beginning of the ROM region containing app images.
        static _sapps: u8;
    }
    kernel::process::load_processes(&_sapps as *const u8,
                                    &mut APP_MEMORY,
                                    &mut PROCESSES,
                                    FAULT_RESPONSE);

    kernel::main(&signpost_controller, &mut chip, &mut PROCESSES, &signpost_controller.ipc);
}