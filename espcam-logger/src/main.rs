use embedded_svc::io::Write;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::http::{client::EspHttpConnection, Method};
use esp_idf_svc::nvs;
use esp_idf_svc::sntp;
use esp_idf_sys::{esp_deep_sleep_start, esp_sleep_enable_timer_wakeup};
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

    let mut led = PinDriver::output(peripherals.pins.gpio4).unwrap();

    led.set_high().unwrap();
    camera.get_framebuffer();
    // take two frames to get a fresh one
    let framebuffer = camera.get_framebuffer();

    if let Some(framebuffer) = framebuffer {
        let data = framebuffer.data();

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let headers: [(&str, &str); 2] = [
            (
                "Content-Type",
                &format!("multipart/form-data; boundary={}", boundary),
            ),
            ("Content-Length", &data.len().to_string()),
        ];
        let mut http_conn = EspHttpConnection::new(&esp_idf_svc::http::client::Configuration {
            timeout: Some(Duration::from_secs(60)),
            buffer_size: Some(4096),
            buffer_size_tx: Some(4096),
            ..Default::default()
        })
        .unwrap();
        if let Err(e) =
            http_conn.initiate_request(Method::Post, "http://192.168.1.222:3000/upload", &headers)
        {
            log::warn!("Failed to initiate request: {e}");
        } else {
            let mut body = Vec::new();
            let filename = dt_now.date_naive().format("%Y-%m-%d.jpg").to_string();
            // Build multipart body
            write!(body, "--{}\r\n", boundary).unwrap();
            write!(
                body,
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                filename
            )
            .unwrap();
            write!(body, "Content-Type: image/jpeg\r\n\r\n").unwrap();
            body.extend_from_slice(data);
            write!(body, "\r\n--{}--\r\n", boundary).unwrap();

            match http_conn.write_all(&body) {
                Ok(_) => log::info!("Uploaded file {filename}"),
                Err(e) => log::error!("Failed to write data: {}", e),
            };
        }
    } else {
        log::info!("No framebuffer available");
    }
    led.set_low().unwrap();

    deep_sleep_until(dt_now + Duration::from_secs(24 * 60 * 60));
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
