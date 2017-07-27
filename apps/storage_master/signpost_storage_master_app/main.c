#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <i2c_master_slave.h>
#include <led.h>
#include <sdcard.h>
#include <timer.h>
#include <tock.h>

#include "app_watchdog.h"
#include "signpost_api.h"
#include "signpost_storage.h"
#include "storage_master.h"

#define DEBUG_RED_LED 0

static void storage_api_callback(uint8_t source_address,
    signbus_frame_type_t frame_type, signbus_api_type_t api_type,
    uint8_t message_type, size_t message_length, uint8_t* message) {
  int err = TOCK_SUCCESS;

  if (api_type != StorageApiType) {
    signpost_api_error_reply_repeating(source_address, api_type, message_type, true, true, 1);
    return;
  }

  if (frame_type == NotificationFrame) {
    // XXX unexpected, drop
  } else if (frame_type == CommandFrame) {
    printf("Got a command message!: len = %d\n", message_length);
    for (size_t i=0; i<message_length; i++) {
      printf("%X ", message[i]);
    }
    printf("\n");

    //XXX do some checking that the message type is right and all that jazz
    //XXX also figure out what module index this is, somehow
    int module_index = 0;

    printf("Writing data\n");

    // get initial record
    Storage_Record_Pointer_t write_record = {0};
    write_record.block = storage_status.status_records[module_index].curr.block;
    write_record.offset = storage_status.status_records[module_index].curr.offset;

    // write data to storage
    err = storage_write_record(write_record, message, message_length, &write_record);
    if (err < TOCK_SUCCESS) {
      printf("Writing error: %d\n", err);
      //XXX: send error
    }

    // update record
    storage_status.status_records[module_index].curr.block = write_record.block;
    storage_status.status_records[module_index].curr.offset = write_record.offset;
    err = storage_update_status();
    if (err < TOCK_SUCCESS) {
      printf("Updating status error: %d\n", err);
      //XXX: send error
    }
    printf("Complete. Final block: %lu offset: %lu\n", write_record.block, write_record.offset);

    // send response
    err = signpost_storage_write_reply(source_address, (uint8_t*)&write_record);
    if (err < TOCK_SUCCESS) {
      //XXX: I guess just try to send an error...
    }

  } else if (frame_type == ResponseFrame) {
    // XXX unexpected, drop
  } else if (frame_type == ErrorFrame) {
    // XXX unexpected, drop
  }
}

int main (void) {
  printf("\n[Storage Master]\n** Main App **\n");

  // set up the SD card and storage system
  int rc = storage_initialize();
  if (rc != TOCK_SUCCESS) {
    printf(" - Storage initialization failed\n");
    return rc;
  }

  // turn off Red Led
  led_off(DEBUG_RED_LED);

  // Install hooks for the signpost APIs we implement
  static api_handler_t storage_handler = {StorageApiType, storage_api_callback};
  static api_handler_t* handlers[] = {&storage_handler, NULL};
  do {
    rc = signpost_initialization_module_init(ModuleAddressStorage, handlers);
    if (rc < 0) {
      printf(" - Error initializing bus access (code: %d). Sleeping 5s\n", rc);
      delay_ms(5000);
    }
  } while (rc < 0);

  // Setup watchdog
  //app_watchdog_set_kernel_timeout(30000);
  //app_watchdog_start();

  printf("\nStorage Master initialization complete\n");
}

