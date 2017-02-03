#![crate_name = "signpost_drivers"]
#![crate_type = "rlib"]
#![feature(const_fn)]
#![no_std]

extern crate kernel;
extern crate signpost_hil;


pub mod mcp23008;
pub mod pca9544a;
pub mod ltc2941;

pub mod lps331ap;

pub mod gpio_async;
pub mod i2c_selector;
pub mod smbus_interrupt;
pub mod app_watchdog;
pub mod watchdog_kernel;

pub mod gps_console;
