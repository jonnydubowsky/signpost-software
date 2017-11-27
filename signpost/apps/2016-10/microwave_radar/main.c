#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <stdint.h>
#include <stdbool.h>

#include "alarm.h"
#include "tock.h"
#include "adc.h"
#include "console.h"
#include "timer.h"
#include "gpio.h"
#include "led.h"
#include "app_watchdog.h"
#include "i2c_master_slave.h"

#define UNUSED_PARAMETER(x) (void)(x)

// Global store
uint32_t t1;
uint32_t t2;
uint32_t sample_index;
uint32_t max_sample;
uint32_t min_sample;
bool active_mode = false;

#define RISING 2
#define FALLING 3
#define FINISH_RISING 5
#define FINISH_FALLING 6
uint32_t sample_state;

// Other global variable
const uint32_t INITIAL_VAL = 0x820;
//const uint32_t LOWER_BOUND = 0X800;
//const uint32_t UPPER_BOUND = 0x840;
const uint32_t LOWER_BOUND = 0X750;
const uint32_t UPPER_BOUND = 0x890;
//const uint32_t NOISE_OFFSET = 4;
const uint32_t NOISE_OFFSET = 10;

// Some define
#define SAMPLE_TIMES 20
#define ADC_CHANNEL 0
#define LED_PIN 0

// used to store the interval of each half period
uint32_t time_intervals[SAMPLE_TIMES];

// i2c message storage
uint8_t master_write_buf[256];

bool motion_since_last_transmit = false;
uint32_t max_speed_since_last_transmit = 0;

static bool detect_motion (uint32_t sample) {
    return (sample > UPPER_BOUND) || (sample < LOWER_BOUND);
}

static uint32_t calculate_sample_frequency (uint32_t curr_data) {
    // check if data signifies movement
    if (!active_mode && detect_motion(curr_data)) {
        // initialize active mode
        active_mode = true;
        sample_index = 0;

        // starting this period sample
        t1 = alarm_read();
        if (INITIAL_VAL < curr_data) {
            sample_state = RISING;
            max_sample = curr_data;
            min_sample = curr_data;
        } else {
            sample_state = FALLING;
            min_sample = curr_data;
            max_sample = curr_data;
        }
    }

    // sample frequencies
    if (active_mode) {

        // keep sampling until signals peaks or bottoms
        if (sample_state == RISING) {
            if (curr_data <= (max_sample - NOISE_OFFSET)) {
                // data falling again
                sample_state = FINISH_RISING;
            } else {
                if (curr_data > max_sample) {
                    max_sample = curr_data;
                }
                sample_state = RISING;
            }
        } else if (sample_state == FALLING) {
            if (curr_data >= (min_sample + NOISE_OFFSET)) {
                // data rising again
                sample_state = FINISH_FALLING;
            } else {
                if (curr_data < min_sample) {
                    min_sample = curr_data;
                }
                sample_state = FALLING;
            }
        }

        // calculate time for that period
        if (sample_state == FINISH_RISING || sample_state == FINISH_FALLING) {
            t2 = alarm_read();
            time_intervals[sample_index] = t2-t1;
            sample_index++;

            t1 = t2;
            if (sample_state == FINISH_FALLING) {
                sample_state = RISING;
                max_sample = curr_data;
                min_sample = curr_data;
            } else if (sample_state == FINISH_RISING) {
                sample_state = FALLING;
                min_sample = curr_data;
                max_sample = curr_data;
            }

            if (sample_index >= SAMPLE_TIMES) {
                // average clock ticks between pos and neg edge
                uint32_t average = 0;

                // The first value is outlier. Don't account for that
                for(int i = 1; i < SAMPLE_TIMES; i++) {
                    average += time_intervals[i]/(SAMPLE_TIMES - 1);
                }

                // Since we are using timer with 32kHz frequency and prescaler 0
                // One tick means 0.0000625 s
                uint32_t interval = average * 625 * 2;
                uint32_t frequency = 10000000/interval;
                active_mode = false;

                return frequency;
            }
        }
    }

    return 0;
}

static uint32_t calculate_radar_speed (uint32_t freq) {
    // Note: this speed is not really meaningful because it calculates
    // the corresponding speed when object moving perpendicular to sensor
    uint32_t speed_fph = (freq * 5280)/31;
    uint32_t speed_mfps = (speed_fph*1000)/3600;

    printf("Freq: %lu\tSpeed: %lu (milli-fps)\n", freq, speed_mfps);

    return speed_mfps;
}


bool _sample_done = false;

// Callback when the adc reading is done
static void adc_callback (int callback_type, int channel, int sample, void* callback_args) {
    UNUSED_PARAMETER(callback_type);
    UNUSED_PARAMETER(channel);
    UNUSED_PARAMETER(callback_args);

    bool motion = detect_motion(sample);
    if (motion) {
        led_on(LED_PIN);
        motion_since_last_transmit = true;
    } else {
        led_off(LED_PIN);
    }
    uint32_t speed_mfps = 0;

    // determine mircowave radar frequency
    uint32_t freq = calculate_sample_frequency(sample);
    if (freq != 0) {
        speed_mfps = calculate_radar_speed(freq);
        uint32_t speed_mmps = (speed_mfps*1000)/3280;

        if (speed_mmps > max_speed_since_last_transmit) {
            max_speed_since_last_transmit = speed_mmps;
        }
    }

    // get new sample
    // adc_single_sample(ADC_CHANNEL);
    _sample_done = true;
}

static void timer_callback (
        int callback_type __attribute__ ((unused)),
        int pin_value __attribute__ ((unused)),
        int unused __attribute__ ((unused)),
        void* callback_args __attribute__ ((unused))
        ) {

    //printf("TIMER motion: %d speed: %d (mm/s)\n", motion_since_last_transmit, max_speed_since_last_transmit);

    // set i2c address and service id
    master_write_buf[0] = 0x34;
    master_write_buf[1] = 0x01;

    // set data
    // boolean, motion since last transmission
    master_write_buf[2] = (motion_since_last_transmit & 0xFF);
    // uint32_t, max speed in milli-meters per second detected since last transmission
    master_write_buf[3] = ((max_speed_since_last_transmit >> 24) & 0xFF);
    master_write_buf[4] = ((max_speed_since_last_transmit >> 16) & 0xFF);
    master_write_buf[5] = ((max_speed_since_last_transmit >>  8) & 0xFF);
    master_write_buf[6] = ((max_speed_since_last_transmit)       & 0xFF);

    // write data
    int result = i2c_master_slave_write_sync(0x22, 7);

    if (result >= 0) {
        app_watchdog_tickle_kernel();
    }

    // reset variables
    motion_since_last_transmit = false;
    max_speed_since_last_transmit = 0;
}

int main (void) {
    printf("[Microwave Radar] Start\n");

    // initialize LED
    gpio_enable_output(LED_PIN);
    gpio_set(LED_PIN);

    // setup i2c
    // microwave radar's i2c address is 0x34
    i2c_master_slave_set_slave_address(0x34);
    i2c_master_slave_set_master_write_buffer(master_write_buf, 256);

    // setup timer
    // set to about two seconds, but a larger prime number so that hopefully we
    //  can avoid continually conflicting with other modules
    static tock_timer_t timer;
    timer_every(2*1039, timer_callback, NULL, &timer);

    // initialize adc
    adc_set_callback(adc_callback, NULL);

    // Setup a watchdog
    app_watchdog_set_kernel_timeout(10000);
    app_watchdog_start();

    // start getting samples
    while (1) {
        _sample_done = false;
        adc_single_sample(ADC_CHANNEL);
        yield_for(&_sample_done);
    }

}

