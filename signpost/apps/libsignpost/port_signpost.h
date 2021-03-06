#pragma once

/* This is a port interface for the signpost API and networking stack!
The goal is to export the minimal interface such that platforms
can integrate with the signpost.

From a high level you will need to provide:
    - Blocking I2C write function (currently at least 255 bytes at a time)
    - Asynchronous I2C slave function (listen on an address and send
                                        back written bytes asynchronously)
    - Set and clear of the MOD_OUT pin
    - Asynchronous callback for falling edge of the Mod-in pin
    - Currently the signpost libraries assume sprintf and malloc functionality
        - This is assumed to be provided through <stdio.h> and <stdlib.h>
    - A way to wait on a variable reference
        -This could be as simple as looping until it changes or
        -Could be more complex like going to sleep
        -It is assumed the variable will change due to an interrupt context

To provide this functionality please implement the following interface
for each platform*/

#define PORT_I2C_MAX_LEN    255
#define PORT_PRINT_MAX_LEN  80
#define PORT_SAVE_MAX_LEN   1024

//Error code definitions
#define PORT_SUCCESS       0
#define PORT_FAIL         -1
#define PORT_EBUSY        -2
#define PORT_EINVAL       -6
#define PORT_ESIZE        -7
#define PORT_ENOMEM       -9
#define PORT_ENOSUPPORT   -10
#define PORT_ENOACK       -13
#define PORT_EI2C_WRITE   -100
#define PORT_ECRYPT       -101
#define PORT_ENOSAT       -102

//These are the callback definitions
#ifdef __cplusplus
extern "C" {
#endif
//These are the callback definitions
//This is the typedef of the callback you should call when you get an i2c slave write
//You can either pass in the (positive) length or an error code
typedef void (*port_signpost_callback)(int len_or_rc);

//This function is called upon signpost initialization
//You should use it to set up the i2c interface
int port_signpost_init(uint8_t i2c_address);

//This function is a blocking i2c send call
//it should return the length of the message successfully sent on the bus
//If the bus returns an error, use the appropriate error code
//defined in this file
int port_signpost_i2c_master_write(uint8_t addr, uint8_t* buf, size_t len);

//This function is sets up the asynchronous i2c receive interface
//When this function is called start listening on the i2c bus for
//The address specified in init
//Place data in the buffer no longer than the max_len
int port_signpost_i2c_slave_listen(port_signpost_callback cb, uint8_t* buf, size_t max_len);

//This function prepares a slave read
//len bytes from buf will be read by a master read
int port_signpost_i2c_slave_read_setup(uint8_t* buf, size_t len);

//These functions are used to control mod_out/mod_in gpio
int port_signpost_mod_out_set(void);
int port_signpost_mod_out_clear(void);
int port_signpost_mod_in_read(void);
int port_signpost_pps_read(void);

//This function is used to setup a gpio interrupt
//interrupt assumes pulled up, falling edge
int port_signpost_mod_in_enable_interrupt_falling(port_signpost_callback cb);
int port_signpost_mod_in_enable_interrupt_rising(port_signpost_callback cb);
int port_signpost_mod_in_disable_interrupt(void);

//This is a way to wait on a variable in a platform specific way
void port_signpost_wait_for(void* wait_on_true);

//This is a way to wait on a variable with a timeout.
//Returns PORT_SUCCESS on success, PORT_FAIL on timeout
int port_signpost_wait_for_with_timeout(void* wait_on_true, uint32_t ms);

void port_signpost_delay_ms(unsigned ms);

//An optional debug led
int port_signpost_debug_led_on(void);
int port_signpost_debug_led_off(void);


/*  port_rng_init
 *  Sets up rng
 *  returns PORT_SUCCESS on success, PORT_FAIL on failure.
 */
int port_rng_init(void);

/*  port_rng_sync
 *  Synchronous RNG request.
 *    buf: user defined buffer.
 *    len: length of buffer.
 *    num: number of random bytes requested.
 *  returns number of random bytes acquired on success, PORT_FAIL on failure.
 */
int port_rng_sync(uint8_t* buf, uint32_t len, uint32_t num);

/* port_printf
 * Platform specific implementation of printf
 * This is usually implemented using vprintf and stdarg.h
 */
int port_printf(const char *fmt, ...) __attribute__ ((format (gnu_printf, 1, 2)));

/* port_signpost_save_state
 * Save a struct to a nonvolatile storage
 *  returns PORT_SUCCESS on success, PORT_FAIL on failure.
 * */
int port_signpost_save_state(uint8_t* state, uint16_t state_len);

/* port_signpost_load_state
 * Load a struct from nonvolatile storage
 *  returns PORT_SUCCESS on success, PORT_FAIL on failure.
 * */
int port_signpost_load_state(uint8_t* state, uint16_t state_len);

#ifdef __cplusplus
}
#endif
