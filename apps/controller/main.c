#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <stdint.h>
#include <stdbool.h>

#include "firestorm.h"
#include "tock.h"
#include "tock_str.h"
#include "signpost_energy.h"
#include "controller.h"



int _length;
int _go = 0;

uint8_t slave_read_buf[256];
uint8_t slave_write_buf[256];
uint8_t master_read_buf[256];
uint8_t master_write_buf[256];

uint8_t fm25cl_read_buf[256];
uint8_t fm25cl_write_buf[256];

static void gpio_async_callback (
        int callback_type __attribute__ ((unused)),
        int pin_value __attribute__ ((unused)),
        int unused __attribute__ ((unused)),
        void* callback_args __attribute__ ((unused))
        ) {
}

static void i2c_master_slave_callback (
        int callback_type,
        int length,
        int unused __attribute__ ((unused)),
        void* callback_args __attribute__ ((unused))
        ) {

  if (callback_type == 3) {
    _length = length;

    _go = 1;
  }
}

static void fm25cl_callback (
        int callback_type __attribute__ ((unused)),
        int pin_value __attribute__ ((unused)),
        int unused __attribute__ ((unused)),
        void* callback_args __attribute__ ((unused))
        ) {
}

static void timer_callback (
        int callback_type __attribute__ ((unused)),
        int pin_value __attribute__ ((unused)),
        int unused __attribute__ ((unused)),
        void* callback_args __attribute__ ((unused))
        ) {
  get_energy();
}

static void bonus_timer_callback (
        int callback_type __attribute__ ((unused)),
        int pin_value __attribute__ ((unused)),
        int unused __attribute__ ((unused)),
        void* callback_args __attribute__ ((unused))
        ) {
  putstr("BONUS TIMER!!!!!!!!!!!!!!!!!\n");
}

void print_data () {
  char buf[64];

  // Need at least two bytes to be a valid signpost message.
  if (_length < 2) {
    return;
  }

  // First byte is the sender
  int sender_address = slave_write_buf[0];
  // Second byte is the packet type
  int message_type = slave_write_buf[1];

  // Handle each type of message.
  switch (sender_address) {
    case 0x31: { // 802.15.4 scanner
      if (message_type == 1 && _length == (16+2)) {
        // Got valid message from 15.4 scanner
        putstr("Message type 1 from Scanner15.4\n");
        for (int channel=11; channel<27; channel++) {
          sprintf(buf, "  Channel %i RSSI: %i\n", channel, (int) ((int8_t) slave_write_buf[2+(channel-11)]));
          putstr(buf);
        }
      }

      break;
    }
    case 0x32: { // Ambient
      if (message_type == 1 && _length == (8+2)) {
        // Got valid message from ambient
        putstr("Message type 1 from Ambient\n");
        int temp = (int) ((int16_t) ((((uint16_t) slave_write_buf[2]) << 8) | ((uint16_t) slave_write_buf[3])));
        int humi = (int) ((int16_t) ((((uint16_t) slave_write_buf[4]) << 8) | ((uint16_t) slave_write_buf[5])));
        int ligh = (int) ((int16_t) ((((uint16_t) slave_write_buf[6]) << 8) | ((uint16_t) slave_write_buf[7])));
        int pres = (int) ((int16_t) ((((uint16_t) slave_write_buf[8]) << 8) | ((uint16_t) slave_write_buf[9])));
        sprintf(buf, "  Temperature: %i 1/100 degrees C\n", temp);
        putstr(buf);
        sprintf(buf, "  Humidity: %i 0.01%%\n", humi);
        putstr(buf);
        sprintf(buf, "  Light: %i Lux\n", ligh);
        putstr(buf);
        sprintf(buf, "  Pressure: %i ubar\n", pres);
        putstr(buf);
      }

      break;
    }
    default: {
      sprintf(buf, "Different message? %i\n  ", sender_address);
      putstr(buf);
      for (int i=0; i<_length; i++) {
        sprintf(buf, "0x%02x ", slave_write_buf[i]);
        putstr(buf);
      }
      sprintf(buf, "\n");
      putstr(buf);
    }
  }

}




#define MAGIC 0x49A80003

typedef struct {
  uint32_t magic;
  uint32_t energy_controller;
  uint32_t energy_linux;
  uint32_t energy_module0;
  uint32_t energy_module1;
  uint32_t energy_module2;
  uint32_t energy_module5;
  uint32_t energy_module6;
  uint32_t energy_module7;
} controller_fram_t;

// Keep track of the last time we got data from the ltc chips
// so we can do diffs when reading the energy.
// uint32_t energy_controller_last_reading = 0;
// uint32_t energy_linux_last_reading = 0;
// uint32_t energy_module0_last_reading = 0;
// uint32_t energy_module1_last_reading = 0;
// uint32_t energy_module2_last_reading = 0;
// uint32_t energy_module5_last_reading = 0;
// uint32_t energy_module6_last_reading = 0;
// uint32_t energy_module7_last_reading = 0;
uint32_t energy_last_readings[128] = {0};

controller_fram_t fram;

void print_energy_data (int module, int energy) {
  char buf[64];
  if (module == 3) {
    sprintf(buf, "Controller energy: %i uAh\n", energy);
  } else if (module == 4) {
    sprintf(buf, "Linux energy: %i uAh\n", energy);
  } else {
    sprintf(buf, "Module %i energy: %i uAh\n", module, energy);
  }
  putstr(buf);
}

void get_energy () {
  putstr("\n\nUpdated Energy!\n");

  for (int i=0; i<8; i++) {
    uint32_t energy;
    uint32_t* last_reading = &energy_last_readings[i];

    if (i == 3) {
      energy = signpost_ltc_to_uAh(signpost_energy_get_controller_energy(), POWER_MODULE_RSENSE, POWER_MODULE_PRESCALER);
    } else if (i == 4) {
      energy = signpost_ltc_to_uAh(signpost_energy_get_linux_energy(), POWER_MODULE_RSENSE, POWER_MODULE_PRESCALER);
    } else {
      energy = signpost_ltc_to_uAh(signpost_energy_get_module_energy(i), POWER_MODULE_RSENSE, POWER_MODULE_PRESCALER);
    }

    uint32_t diff = energy - *last_reading;
    *last_reading = energy;



    switch (i) {
      case 0: fram.energy_module0 += diff; break;
      case 1: fram.energy_module1 += diff; break;
      case 2: fram.energy_module2 += diff; break;
      case 3: fram.energy_controller += diff; break;
      case 4: fram.energy_linux += diff; break;
      case 5: fram.energy_module5 += diff; break;
      case 6: fram.energy_module6 += diff; break;
      case 7: fram.energy_module7 += diff; break;
    }

    fm25cl_write(0, sizeof(controller_fram_t));
    yield();

    // Test print
    switch (i) {
      case 0: print_energy_data(i, fram.energy_module0); break;
      case 1: print_energy_data(i, fram.energy_module1); break;
      case 2: print_energy_data(i, fram.energy_module2); break;
      case 3: print_energy_data(i, fram.energy_controller); break;
      case 4: print_energy_data(i, fram.energy_linux); break;
      case 5: print_energy_data(i, fram.energy_module5); break;
      case 6: print_energy_data(i, fram.energy_module6); break;
      case 7: print_energy_data(i, fram.energy_module7); break;
    }
  }
}

int main () {
  putstr("[Controller] ** Main App **\n");

  // Setup backplane by enabling the modules
  gpio_async_set_callback(gpio_async_callback, NULL);
  controller_init_module_switches();
  controller_all_modules_enable_power();
  controller_all_modules_enable_i2c();
  // controller_all_modules_disable_i2c();
  // controller_module_enable_i2c(MODULE5);
  // controller_module_enable_i2c(MODULE0);

  // Configure FRAM
  fm25cl_set_callback(fm25cl_callback, NULL);
  fm25cl_set_read_buffer((uint8_t*) &fram, sizeof(controller_fram_t));
  fm25cl_set_write_buffer((uint8_t*) &fram, sizeof(controller_fram_t));

  // Read FRAM to see if anything is stored there
  fm25cl_read(0, sizeof(controller_fram_t));
  yield();
  if (fram.magic == MAGIC) {
    // Great. We have saved data.
  } else {
    // Initialize this
    fram.magic = MAGIC;
    fram.energy_controller = 0;
    fram.energy_linux = 0;
    fram.energy_module0 = 0;
    fram.energy_module1 = 0;
    fram.energy_module2 = 0;
    fram.energy_module5 = 0;
    fram.energy_module6 = 0;
    fram.energy_module7 = 0;
    fm25cl_write(0, sizeof(controller_fram_t));
    yield();
  }

  // Need to init the signpost energy library
  signpost_energy_init();

  // Reset all of the LTC2941s
  signpost_energy_reset();

  // Need a timer
  timer_subscribe(timer_callback, NULL);
  bonus_timer_subscribe(bonus_timer_callback, NULL);


  timer_start_repeating(10000);
  bonus_timer_start_repeating(27000);



  while (1) {

    yield();

    // get_energy();



  }




  // // Setup I2C listen
  // i2c_master_slave_set_callback(i2c_master_slave_callback, NULL);
  // i2c_master_slave_set_slave_address(0x20);

  // i2c_master_slave_set_master_read_buffer(master_read_buf, 256);
  // i2c_master_slave_set_master_write_buffer(master_write_buf, 256);
  // i2c_master_slave_set_slave_read_buffer(slave_read_buf, 256);
  // i2c_master_slave_set_slave_write_buffer(slave_write_buf, 256);

  // i2c_master_slave_listen();

  // while (1) {
  //   yield();

  //   if (_go == 1) {
  //     _go = 0;

  //     print_data();
  //   }
  // }
}
