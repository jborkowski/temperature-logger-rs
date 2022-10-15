#![allow(unused_imports)]

mod wifi;

use esp_idf_hal::i2c::Master;
use esp_idf_sys as _; // If using the `libstart` feature of `esp-idf-sys`, always keep this module imported

use anyhow::{bail, Result};

use log::*;
use std::{cell::RefCell, env, sync::atomic::*, sync::Arc, thread, time::*};

use embedded_svc::mqtt::client::{
    Client, Details::Complete, Event::Received, Message, Publish, QoS,
};

use esp_idf_svc::{
    log::EspLogger,
    mqtt::client::{EspMqttClient, EspMqttMessage, MqttClientConfiguration},
};



use esp_idf_svc::nvs::EspDefaultNvs;
use esp_idf_svc::nvs::*;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::sysloop::*;

use embedded_hal::blocking::delay::DelayMs;

use esp_idf_hal::delay;
use esp_idf_hal::gpio;
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;

use dht_sensor::*;
use esp_idf_sys::{esp, EspError};
use ds1307::{DateTimeAccess, Ds1307, NaiveDate};


#[allow(dead_code)]
const WIFI_SSID: &str = env!("RUST_ESP32_WIFI_SSID");
#[allow(dead_code)]
const WIFI_PASS: &str = env!("RUST_ESP32_WIFI_PASS");
#[allow(dead_code)]
const MQTT_USER: &str = env!("RUST_ESP32_MQTT_USER");
#[allow(dead_code)]
const MQTT_PASS: &str = env!("RUST_ESP32_MQTT_PASS");
#[allow(dead_code)]
const MQTT_HOST: &str = env!("RUST_ESP32_MQTT_HOST");

#[no_mangle]
extern "C" fn rust_main() -> ! {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let mut delay = delay::Ets;

    let mut _wifi = wifi::wifi(WIFI_SSID, WIFI_PASS).unwrap();

    let mqtt_config = MqttClientConfiguration::default();

    let broker_url = format!("mqtt://{}:{}@{}", MQTT_USER, MQTT_PASS, MQTT_HOST);

    delay.delay_ms(2000 as u32);
    let mut mqtt_client = EspMqttClient::new(broker_url, &mqtt_config, move |message_event| {
        if let Ok(Received(message)) = message_event {
            process_message(message);
        }
    })
    .unwrap();

    let i2c = i2c(peripherals.i2c0, pins.gpio0, pins.gpio1);
    let mut rtc = Ds1307::new(i2c);

    // let datetime = NaiveDate::from_ymd(2022, 10, 16).and_hms(0, 29, 10);
    //  rtc.set_datetime(&datetime).unwrap();
    rtc.halt().unwrap();

    let mut data = pins.gpio4.into_input_output().unwrap();

    loop {
        match dht22::Reading::read(&mut delay, &mut data) {
            Ok(dht22::Reading {
                temperature,
                relative_humidity,
            }) => {

		info!("[{}] Temperature: {}°, Humidity {} % RHr", rtc.datetime().unwrap(), temperature, relative_humidity);
                mqtt_client
                    .publish(
                        "temperature/office",
                        QoS::AtLeastOnce,
                        false,
                        &temperature.to_be_bytes() as &[u8],
                    )
                    .unwrap();

                mqtt_client
                    .publish(
                        "humidity/office",
                        QoS::AtLeastOnce,
                        false,
                        &relative_humidity.to_be_bytes() as &[u8],
                    )
                    .unwrap();
            }
            Err(e) => println!("Error {:?}", e),
        }
        delay.delay_ms(1000 as u32);
    }
}

fn process_message(message: &EspMqttMessage) {
    match message.details() {
        Complete => {
            info!("{}", message.topic().unwrap());
            //            let message_data: &[u8] = &message.data();
        }
        _ => error!("Unsupported command."),
    }
}

fn i2c(
    i2c: i2c::I2C0,
    sda: gpio::Gpio0<gpio::Unknown>,
    scl: gpio::Gpio1<gpio::Unknown>,
) -> Master<i2c::I2C0, gpio::Gpio0<gpio::Unknown>, gpio::Gpio1<gpio::Unknown>,
> {
    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());

    Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap()
}
