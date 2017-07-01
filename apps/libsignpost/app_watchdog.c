#include <stdint.h>

#include "app_watchdog.h"
#include "tock.h"

int app_watchdog_start(void) {
  return command(DRIVER_NUM_APPWATCHDOG, 4, 0);
}

int app_watchdog_stop(void) {
  return command(DRIVER_NUM_APPWATCHDOG, 5, 0);
}

int app_watchdog_tickle_app(void) {
  return command(DRIVER_NUM_APPWATCHDOG, 0, 0);
}

int app_watchdog_tickle_kernel(void) {
  return command(DRIVER_NUM_APPWATCHDOG, 1, 0);
}

int app_watchdog_set_app_timeout(int timeout) {
  return command(DRIVER_NUM_APPWATCHDOG, 2, timeout);
}

int app_watchdog_set_kernel_timeout(int timeout) {
  return command(DRIVER_NUM_APPWATCHDOG, 3, timeout);
}

int app_watchdog_reset_app(void) {
  return command(DRIVER_NUM_APPWATCHDOG, 6, 0xDEAD);
}
