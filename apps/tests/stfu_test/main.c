#include <signpost_tock_firmware_update.h>


int main () {
  int ret;

  printf("started\n");

  uint8_t buffer[4] = {74, 79, 83, 72};
  ret = signpost_tock_firmware_update_write_buffer(buffer, 0x304, 4);
  printf("rename %s\n", tock_strerror(ret));

  ret = signpost_tock_firmware_update_go(0x60000, 0x30000, 1024, 0);
  printf("ok %s\n", tock_strerror(ret));
}
