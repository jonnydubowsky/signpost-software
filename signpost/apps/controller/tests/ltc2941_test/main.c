#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <timer.h>
#include <tock.h>

#include "ltc294x.h"


static void print_data (int charge) {
  printf("\tCharge: 0x%02x\n\n", charge);
}

static void print_status (int status) {
  printf("\tStatus: 0x%02x\n\n", status);
}

int main (void) {
  printf("[LTC2941] Test\n");

  int status = ltc294x_read_status_sync();
  print_status(status);

  ltc294x_reset_charge_sync();
  ltc294x_set_high_threshold_sync(0x0010);

  while (1) {
    int charge = ltc294x_get_charge_sync();
    print_data(charge);
    delay_ms(1000);
  }
}
