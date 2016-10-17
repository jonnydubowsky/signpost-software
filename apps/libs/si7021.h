#pragma once

#include "tock.h"

#ifdef __cplusplus
extern "C" {
#endif

#define DRIVER_NUM_SI7021 107

int si7021_set_callback (subscribe_cb callback, void* callback_args);
int si7021_get_temperature_humidity ();

int si7021_get_temperature_humidity_sync (int* temperature, int* humidity);

#ifdef __cplusplus
}
#endif
