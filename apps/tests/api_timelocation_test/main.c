#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <timer.h>
#include <tock.h>

#include "signpost_api.h"

static const uint8_t random_i2c_address = 0x51;

int main (void) {
  printf("\n\n[Test] API: Time & Location\n");

  int rc = signpost_initialization_module_init(
    random_i2c_address,
    SIGNPOST_INITIALIZATION_NO_APIS);
  if (rc < TOCK_SUCCESS) {
    printf("Signpost initialization errored: %d\n", rc);
  }

  signpost_timelocation_time_t time;
  signpost_timelocation_location_t location;

  while (true) {
    printf("Query Time\n");
    rc = signpost_timelocation_get_time(&time);
    if (rc < TOCK_SUCCESS) {
      printf("Error querying time: %d\n\n", rc);
    } else {
      printf("  Current time: %d/%d/%d %d:%02d:%02d with %d satellites\n", time.year, time.month, time.day, time.hours,
             time.minutes, time.seconds, time.satellite_count);
    }
    printf("Query Location\n");
    rc = signpost_timelocation_get_location(&location);
    if (rc < TOCK_SUCCESS) {
      printf("Error querying location: %d\n\n", rc);
    } else {
      float lat = ((float) location.latitude) / 1000000.0;
      float lon = ((float) location.longitude) / 1000000.0;
      printf("  Current location:\n");
      printf("    Latitude:  %f %lu\n", lat, location.latitude);
      printf("    Longitude: %f %lu\n", lon, location.longitude);
      printf("  With %d satellites\n",location.satellite_count);
    }

    printf("Sleeping for 5s\n\n");
    delay_ms(5000);
  }
}
