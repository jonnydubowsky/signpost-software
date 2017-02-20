#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <stdint.h>
#include <stdbool.h>

#include <tock.h>
#include <firestorm.h>
#include <console.h>
#include "test_arithmetic.h"
#include "erpc_client_setup.h"
#include "erpc_transport_setup.h"
#include "timer.h"
#include "gpio.h"


int main(void) {
    erpc_transport_t transport;
    transport = erpc_transport_i2c_master_slave_init(0x19,0x30);
    erpc_client_init(transport);

    for(uint8_t i = 0; i < 10; i++) {
        delay_ms(5000);
        gpio_clear(10);
        float result = 0;
        result =  add(34,25);
        if(result <= 59) {
            gpio_set(10);
        } else {

        }
    }


}
