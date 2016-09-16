
use common::{List, ListLink, ListNode};
use common::take_cell::TakeCell;
use core::cell::Cell;
use hil;
use signpost_hil;

pub struct MuxSPIMaster<'a> {
    spi: &'a hil::spi::SpiMaster,
    devices: List<'a, SPIMasterDevice<'a>>,
    // enabled: Cell<usize>,
    inflight: TakeCell<&'a SPIMasterDevice<'a>>,
}

impl<'a> hil::spi::SpiMasterClient for MuxSPIMaster<'a> {
    fn read_write_done(&self, write_buffer: Option<&'static mut [u8]>, read_buffer: Option<&'static mut [u8]>, len: usize) {
        self.inflight.take().map(move |device| {
            // write_buffer.map(move |txbuf| {
            //     panic!("virtual cb tx {}", txbuf[0]);
            // });
            // match read_buffer {
            //     Some(x) => {
            //         panic!("virtual cb {} {} {}", x[0], x[1], x[2]);
            //     },
            //     None => {}
            // }
            // match write_buffer {
            //     Some(x) => {
            //         panic!("virtual cbtx {}", x[0]);
            //     },
            //     None => {}
            // }

            device.read_write_done(write_buffer, read_buffer, len);
        });
        self.do_next_op();
    }
}

impl<'a> MuxSPIMaster<'a> {
    pub const fn new(spi: &'a hil::spi::SpiMaster) -> MuxSPIMaster<'a> {
        MuxSPIMaster {
            spi: spi,
            devices: List::new(),
            // enabled: Cell::new(0),
            inflight: TakeCell::empty(),
        }
    }

    // fn enable(&self) {
    //     let enabled = self.enabled.get();
    //     self.enabled.set(enabled + 1);
    //     if enabled == 0 {
    //         self.i2c.enable();
    //     }
    // }

    // fn disable(&self) {
    //     let enabled = self.enabled.get();
    //     self.enabled.set(enabled - 1);
    //     if enabled == 1 {
    //         self.i2c.disable();
    //     }
    // }

    fn do_next_op(&self) {
        if self.inflight.is_none() {
            let mnode = self.devices.iter().find(|node| node.operation.get() != Op::Idle);
            mnode.map(|node| {

                    match node.operation.get() {
                        Op::Configure(cpol, cpal, rate) => {


                            match node.chip_select {
                                Some(x) => {
                                    self.spi.set_chip_select(x);
                                },
                                None => {}
                            }

                            // In theory, the SPI interface should support
                            // using a GPIO in lieu of a hardware CS line.
                            // This is particularly important for the SAM4L
                            // if using a USART, but might be relevant
                            // for other platforms as well.
                            // TODO: make this do something if given GPIO pin
                            match node.chip_select_gpio {
                                Some(x) => { },
                                None => {}
                            }

                            self.spi.set_clock(cpol);
                            self.spi.set_phase(cpal);
                            self.spi.set_rate(rate);


                        },
                          //  self.i2c.write(node.addr, buf, len),
                        Op::ReadWriteBytes(len) => {


                            node.txbuffer.take().map(|txbuffer| {
                                node.rxbuffer.take().map(|rxbuffer| {
                                    self.spi.read_write_bytes(txbuffer, rxbuffer, len);
                                });
                            });

                            // Only async operations want to block by setting the devices
                            // as inflight.
                            self.inflight.replace(node);
                        },
                        // Op::Read(len) => self.i2c.read(node.addr, buf, len),
                        // Op::WriteRead(wlen, rlen) => {
                        //     self.i2c.write_read(node.addr, buf, wlen, rlen)
                        // }
                        Op::Idle => {} // Can't get here...
                    }
                // });
                node.operation.set(Op::Idle);
                // self.inflight.replace(node);
            });
        }
    }
}

#[derive(Copy, Clone,PartialEq)]
enum Op {
    Idle,
    Configure(hil::spi::ClockPolarity, hil::spi::ClockPhase, u32),
    ReadWriteBytes(usize),
    // Write(u8),
    // Read(u8),
    // WriteRead(u8, u8),
}

pub struct SPIMasterDevice<'a> {
    mux: &'a MuxSPIMaster<'a>,
    chip_select: Option<u8>,
    chip_select_gpio: Option<&'static hil::gpio::GPIOPin>,
    // enabled: Cell<bool>,
    txbuffer: TakeCell<Option<&'static mut [u8]>>,
    rxbuffer: TakeCell<Option<&'static mut [u8]>>,
    operation: Cell<Op>,
    next: ListLink<'a, SPIMasterDevice<'a>>,
    client: Cell<Option<&'a hil::spi::SpiMasterClient>>,
}

impl<'a> SPIMasterDevice<'a> {
    pub const fn new(mux: &'a MuxSPIMaster<'a>, chip_select: Option<u8>, chip_select_gpio: Option<&'static hil::gpio::GPIOPin>) -> SPIMasterDevice<'a> {
        SPIMasterDevice {
            mux: mux,
            chip_select: chip_select,
            chip_select_gpio: chip_select_gpio,
            // enabled: Cell::new(false),
            txbuffer: TakeCell::empty(),
            rxbuffer: TakeCell::empty(),
            operation: Cell::new(Op::Idle),
            next: ListLink::empty(),
            client: Cell::new(None),
        }
    }

    pub fn set_client(&'a self, client: &'a hil::spi::SpiMasterClient) {
        self.mux.devices.push_head(self);
        self.client.set(Some(client));
    }
}

impl<'a> hil::spi::SpiMasterClient for SPIMasterDevice<'a> {
    fn read_write_done(&self, write_buffer: Option<&'static mut [u8]>, read_buffer: Option<&'static mut [u8]>, len: usize) {
        self.client.get().map(move |client| {
            client.read_write_done(write_buffer, read_buffer, len);
        });
    }
}

impl<'a> ListNode<'a, SPIMasterDevice<'a>> for SPIMasterDevice<'a> {
    fn next(&'a self) -> &'a ListLink<'a, SPIMasterDevice<'a>> {
        &self.next
    }
}

impl<'a> signpost_hil::spi_master2::SPIMasterDevice for SPIMasterDevice<'a> {
    // fn enable(&self) {
    //     if !self.enabled.get() {
    //         self.enabled.set(true);
    //         self.mux.enable();
    //     }
    // }

    // fn disable(&self) {
    //     if self.enabled.get() {
    //         self.enabled.set(false);
    //         self.mux.disable();
    //     }
    // }

    fn configure(&self, cpol: hil::spi::ClockPolarity, cpal: hil::spi::ClockPhase, rate: u32) {
        self.operation.set(Op::Configure(cpol, cpal, rate));
        self.mux.do_next_op();
    }

    // fn write_read(&self, data: &'static mut [u8], write_len: u8, read_len: u8) {
    //     self.buffer.replace(data);
    //     self.operation.set(Op::WriteRead(write_len, read_len));
    //     self.mux.do_next_op();
    // }

    fn read_write_bytes(&self, write_buffer: Option<&'static mut [u8]>, read_buffer: Option<&'static mut [u8]>, len: usize) -> bool {
        self.txbuffer.replace(write_buffer);
        self.rxbuffer.replace(read_buffer);
        self.operation.set(Op::ReadWriteBytes(len));
        self.mux.do_next_op();

        true
    }

    // fn write(&self, data: &'static mut [u8], len: u8) {
    //     self.buffer.replace(data);
    //     self.operation.set(Op::Write(len));
    //     self.mux.do_next_op();
    // }

    // fn read(&self, buffer: &'static mut [u8], len: u8) {
    //     self.buffer.replace(buffer);
    //     self.operation.set(Op::Read(len));
    //     self.mux.do_next_op();
    // }
}
