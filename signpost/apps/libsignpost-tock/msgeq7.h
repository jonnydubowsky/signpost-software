#pragma once

#include "tock.h"

#ifdef __cplusplus
extern "C" {
#endif

void msgeq7_get_values(uint16_t* samples);
void msgeq7_initialize(uint8_t strobe, uint8_t reset, uint8_t out);

#ifdef __cplusplus
}
#endif
