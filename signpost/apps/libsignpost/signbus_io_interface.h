#pragma once

#include <stdio.h>
#include <stdint.h>
#include "signbus_app_layer.h"
#include "port_signpost.h"

#ifdef __cplusplus
extern "C" {
#endif

// Debugging macros

// Get just the filename, no path
#define __FILENAME__ (strrchr(__FILE__, '/') ? strrchr(__FILE__, '/') + 1 : __FILE__)

#define SIGNBUS_DEBUG(...)
/*
#define SIGNBUS_DEBUG(...) do {\
    port_printf("SBDBG %24s:%30s: %04d: ", __FILENAME__, __func__, __LINE__);\
    port_printf(__VA_ARGS__);\
   } while (0)
*/
#define SIGNBUS_DEBUG_DUMP_BUF(_buf, _buflen)
/*
#define SIGNBUS_DEBUG_DUMP_BUF(_buf, _buflen) do {\
    port_printf("SBDBG %24s:%04d %s(%d,%x)=", __FILENAME__, __LINE__, #_buf, _buflen, _buflen);\
    for (size_t _i = 0; _i < _buflen; _i++) {\
        port_printf("%02x", (unsigned)*(_buf+_i));\
    }\
    port_printf("\n");\
} while (0)
*/


//this is the first i2c messaging library!
//
//The MTU of the i2c bus is 256Bytes.
//We need packets
//Here's the proposed packet structure
//1:ver,2:total len, 3bit flag, remaind fragment offset, src

// Defines for testing. These are arbitrary addresses.
#define SIGNBUS_TEST_SENDER_I2C_ADDRESS   0x32
#define SIGNBUS_TEST_RECEIVER_I2C_ADDRESS 0x18

/// initialize the library
/// MUST be called before any other methods.
void signbus_io_init(
    uint8_t address                   // Address to listen on / send from
    );

/// synchronous send
/// Returns number of bytes sent or < 0 on error.
__attribute__((warn_unused_result))
int signbus_io_send(
    uint8_t dest,                     // Address to send to
    bool encrypted,                   // Is buffer encrypted?
    uint8_t* data,                    // Buffer to send from
    size_t len                        // Number of bytes to send
    );

/// synchronous receive
/// Returns number of bytes recieved or < 0 on error.
__attribute__((warn_unused_result))
int signbus_io_recv(
    size_t   recv_buflen,             // Buffer length
    uint8_t* recv_buf,                // Buffer to receive into
    bool*    encrypted,               // Is buffer encrypted?
    uint8_t* src_address              // Address received from
    );

/// async callback
/// len_or_rc is number of bytes received of < 0 on error.
typedef void (*signbus_io_callback_t)(int len_or_rc);

/// async receive
/// Returns < 0 on error.
__attribute__((warn_unused_result))
int signbus_io_recv_async(
    signbus_io_callback_t callback,   // Called when recv operation completes
    size_t   recv_buflen,             // Buffer length
    uint8_t* recv_buf,                // Buffer to receive into
    bool*    encrypted,               // Is buffer encrypted?
    uint8_t* src                      // Address received from
    );

/// API for slave reads

//set the read buffer
__attribute__((warn_unused_result))
int signbus_io_set_read_buffer(uint8_t* data, uint32_t len);

void signbus_io_set_read_callback(signbus_app_callback_t* callback);

#ifdef __cplusplus
}
#endif
