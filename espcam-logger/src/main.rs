use chrono::TimeZone;
use esp_idf_hal::io::EspIOError;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::http::{server::EspHttpServer, Method};
use esp_idf_svc::nvs;
use esp_idf_svc::sntp;
use esp_idf_sys::{esp_deep_sleep_start, esp_sleep_enable_timer_wakeup};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

mod espcam;
mod network;

use espcam::Camera;

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    let _logger = esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();

    nvs::EspNvsPartition::<nvs::NvsDefault>::take().unwrap();

    let sysloop = EspSystemEventLoop::take().unwrap();
    // Connect to the Wi-Fi network
    let _wifi = match network::wifi("SSID", "PASSWORD", peripherals.modem, sysloop) {
        Ok(inner) => {
            println!("Connected to Wi-Fi network!");
            inner
        }
        Err(err) => {
            // Red!
            panic!("Could not connect to Wi-Fi network: {:?}", err)
        }
    };

    let sntp = network::sntp("pool.ntp.org").expect("Could not set up SNTP");
    log::info!("Synchronising NTP...");
    while sntp.get_sync_status() != sntp::SyncStatus::Completed {
        thread::sleep(Duration::from_millis(100));
    }
    let dt_now = chrono::Local::now();
    log::info!("Current time {dt_now}");

    let camera = Camera::new(
        peripherals.pins.gpio32,
        peripherals.pins.gpio0,
        peripherals.pins.gpio5,
        peripherals.pins.gpio18,
        peripherals.pins.gpio19,
        peripherals.pins.gpio21,
        peripherals.pins.gpio36,
        peripherals.pins.gpio39,
        peripherals.pins.gpio34,
        peripherals.pins.gpio35,
        peripherals.pins.gpio25,
        peripherals.pins.gpio23,
        peripherals.pins.gpio22,
        peripherals.pins.gpio26,
        peripherals.pins.gpio27,
        esp_idf_sys::camera::pixformat_t_PIXFORMAT_JPEG,
        esp_idf_sys::camera::framesize_t_FRAMESIZE_UXGA,
    )
    .unwrap();

    let mut server =
        EspHttpServer::new(&esp_idf_svc::http::server::Configuration::default()).unwrap();

    server
        .fn_handler("/camera.jpg", Method::Get, {
            let led = Mutex::new(PinDriver::output(peripherals.pins.gpio4).unwrap());
            move |request| {
                led.lock().unwrap().set_high().unwrap();
                camera.get_framebuffer();
                // take two frames to get a fresh one
                let framebuffer = camera.get_framebuffer();

                if let Some(framebuffer) = framebuffer {
                    let data = framebuffer.data();

                    let headers = [
                        ("Content-Type", "image/jpeg"),
                        ("Content-Length", &data.len().to_string()),
                    ];
                    let mut response = request.into_response(200, Some("OK"), &headers).unwrap();
                    response.write(data)?;
                } else {
                    let mut response = request.into_ok_response()?;
                    response.write("no framebuffer".as_bytes())?;
                }
                led.lock().unwrap().set_low().unwrap();

                Ok::<(), EspIOError>(())
            }
        })
        .unwrap();

    server
        .fn_handler("/", Method::Get, |request| {
            let mut response = request.into_ok_response()?;
            response.write("ok".as_bytes())?;
            Ok::<(), EspIOError>(())
        })
        .unwrap();

    // deep_sleep_until(dt_now + Duration::from_secs(24 * 60 * 60));
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn deep_sleep_until(target_time: chrono::DateTime<chrono::Local>) {
    let now = chrono::Local::now();
    if target_time > now {
        let sleep_duration = target_time - now; // chrono::Duration
        let sleep_us = sleep_duration.num_microseconds().unwrap_or(0) as u64;

        log::info!("Deep sleeping until {}", target_time);
        unsafe {
            esp_sleep_enable_timer_wakeup(sleep_us);
            esp_deep_sleep_start();
        }
    } else {
        // Target time is in the past, handle accordingly
        // For example, sleep a minimal time or skip sleeping
        log::warn!("Target time is in the past, no deep sleep triggered.");
    }
}
