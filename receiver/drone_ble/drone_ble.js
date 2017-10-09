#!/usr/bin/env node

var noble = require('noble')

var signpost_service_uuid = '75e96f00b766568f7a49286d140dc25c'
var signpost_update_char_uuid = '75e96f01b766568f7a49286d140dc25c' // use this to write request to signpost
var signpost_read_char_uuid = '75e96f02b766568f7a49286d140dc25c' // use this to read data from the signpost
var signpost_notify_char_uuid = '75e96f03b766568f7a49286d140dc25c' // use this to get notifications from signpost

var signpost_service = 0
var signpost_update_char = 0
var signpost_read_char = 0
var signpost_notify_char = 0

var logname = "testtopic" // logname we are looking for

var signpost_peripheral = 0
var packets = []

noble.on('stateChange', function(state) {
  if (state === 'poweredOn') {
    noble.startScanning();
  } else {
    noble.stopScanning();
  }
})

noble.on('discover', on_discovery)

function on_discovery(peripheral) {
  var advertisement = peripheral.advertisement;

  var localName = advertisement.localName;
  var ble_address = advertisement.address;

  if (localName) {
    console.log('Found peripheral with localName ' + localName)
      if(localName == "Signpost") {
        signpost_peripheral = peripheral
        noble.stopScanning()
          explore(peripheral) // set up disconnect callback and connect to the peripheral
      }
  }
  else {
    console.log('Found peripheral with no localName')
  }
}

function explore(peripheral) {
  console.log('services and characteristics:');

  peripheral.on('disconnect', function() {
    console.log("disconnected")
    // send or consume any packets collected now!
    process.exit()
  });

  peripheral.connect(function(error) { peripheral.discoverServices([], on_discover_services); });
}

function on_discover_services(error, services) {
  for(i = 0; i < services.length; i++) {
    var service = services[i]
      console.log("Found service with uuid " + service.uuid)
      if (service.uuid == signpost_service_uuid) {
        console.log("Found signpost service")
          signpost_service = service
          service.discoverCharacteristics([], on_discover_characteristics)
          break
      }
  }
}


function on_discover_characteristics(error, characteristics) {
  console.log("Discover characteristics")
    for(i = 0; i < characteristics.length; i++) {
      var characteristic = characteristics[i]
        console.log("Found char with uuid " + characteristic.uuid)
        if (characteristic.uuid == signpost_update_char_uuid) {
          signpost_update_char = characteristic
        }
        else if (characteristic.uuid == signpost_notify_char_uuid) {
          signpost_notify_char = characteristic
            signpost_notify_char.notify(true, function(err) {console.log("Enabled notify on signpost update char")})
            signpost_notify_char.on('data', on_signpost_notify_data) // Callback for when we get new data
        }
        else if (characteristic.uuid == signpost_read_char_uuid) {
          signpost_read_char = characteristic
        }
    }
  console.log("Time to request some data")
    var buffer = Buffer.from(logname) // put whatever string you need here

    signpost_update_char.write(buffer, false, function() {console.log("Wrote request to signpost_u")})
}

// data is a nodejs buffer.
// you can use functions like data.readUInt8 to get data from the node.
function on_signpost_notify_data(data, isNotify) {
    console.log("Got notify from the signpost")
    if(data.readUInt32LE() == 1) {
      signpost_peripheral.disconnect();
    }

    signpost_read_char.read(function(error, data) {
      console.log("Got some data from the signpost")

      console.log(data)
      packets.push(data)

      // Write ack back
      var buffer = Buffer.from(logname) // put whatever string you need here
      signpost_update_char.write(buffer)

    });
}

