#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <stdint.h>
#include <stdbool.h>

#include <tock.h>
#include <firestorm.h>
#include <ltc2941.h>
#include <i2c_selector.h>
#include <smbus_interrupt.h>

#define MOD0_GPIOA_PORT_NUM 0
#define MOD1_GPIOA_PORT_NUM 1
#define MOD2_GPIOA_PORT_NUM 2
#define MOD5_GPIOA_PORT_NUM 3
#define MOD6_GPIOA_PORT_NUM 4
#define MOD7_GPIOA_PORT_NUM 5

#define UNUSED_PARAMETER(x) (void)(x)

// Global store
int _data = 5;
int _data2 = 6;

// Callback when the pressure reading is ready
void callback (int callback_type, int data, int data2, void* callback_args) {
  UNUSED_PARAMETER(callback_args);

  _data = data;
  _data2 = data2;

}

void print_data (int i) {
  char buf[64];
  sprintf(buf, "\tGot something from counter %i: 0x%02x  | 0x%02x\n\n",i, _data, _data2);
  putstr(buf);
}

int main () {
  putstr("Welcome to Tock...lets wait for an interrupt!!\n");


  // Pass a callback function to the kernel
  smbus_interrupt_set_callback(callback, NULL);
  i2c_selector_set_callback(callback, NULL);
  ltc2941_set_callback(callback, NULL);
    
  // Read from first selector, channel 3 (1000)
  i2c_selector_select_channels(0x08);
  yield();
  
  // Reset charge
  ltc2941_reset_charge();
  yield();
  // And tell it any interrupt that may have existed has been handled
  smbus_interrupt_issue_alert_response();
  yield();
  
  // Read status, first byte should be 0x00
  ltc2941_read_status();
  yield();
  print_data(0);

  // Set high threshold really low so we get a fast interrupt
  ltc2941_set_high_threshold(0x0000);
  yield();
  
  // Open all channels so any gauge can interrupt and respond to SMBUS Alert Response
  // Even though all gauges share same address, and multiple may be interrupting
  // they will all respond with the same address
  i2c_selector_select_channels(0xFF);

  // Wait for interrupt, then print address of who interrupted
  // Should be 0xc8 (gauge address with a 1 tacked on the end)
  putstr("Waiting...\n");
  yield();
  print_data(0);
  
  putstr("Reading Interrupts\n");

  // Query the i2c_selector driver for who interrupted
  i2c_selector_read_interrupts();
  yield();
  print_data(0);

  i2c_selector_select_channels(_data2);
  yield();

  // Reset charge and handle interrupt
  ltc2941_reset_charge();
  yield();
  smbus_interrupt_issue_alert_response();
  yield();

  // Read status twice
  ltc2941_read_status();
  yield();
  print_data(0);
  ltc2941_read_status();
  yield();
  print_data(0);

  while(1) {
    yield();
  }
}
