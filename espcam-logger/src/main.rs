use anyhow::Result;
use embedded_svc::{http::client::Client, http::Method, io::Write};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::gpio::PinDriver, hal::prelude::*,
    http::client::EspHttpConnection, nvs,
};
use esp_idf_sys::{esp_deep_sleep_start, esp_sleep_enable_timer_wakeup};
use serde::{Deserialize, Serialize};
use std::time::Duration;

mod espcam;
mod network;

use espcam::Camera;

#[derive(Serialize, Deserialize)]
struct HealthRequest {
    voltage: f32,
    timestamp: String,
}

struct Config<'a> {
    wifi_ssid: &'a str,
    wifi_password: &'a str,
    server_address: &'a str,
    health_uri: &'a str,
    upload_uri: &'a str,
    wakeup_time: chrono::NaiveTime,
}

const CONFIG: Config = Config {
    wifi_ssid: "Kaneelirull",
    wifi_password: "palunW1f1t",
    server_address: "http://synology:3000",
    health_uri: "/health",
    upload_uri: "/upload",
    wakeup_time: chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
};

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    nvs::EspNvsPartition::<nvs::NvsDefault>::take()?;

    let sysloop = EspSystemEventLoop::take()?;
    // Connect to the Wi-Fi network
    let _wifi = network::wifi(
        CONFIG.wifi_ssid,
        CONFIG.wifi_password,
        peripherals.modem,
        sysloop,
    )
    .expect("Could not connect to Wi-Fi network");
    println!("Connected to Wi-Fi network!");

    network::sntp("pool.ntp.org").expect("Could not set up SNTP");
    let dt_now = chrono::Local::now();
    log::info!("Current time {dt_now}");

    send_health_data(dt_now)?;

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
    )?;

    let mut led = PinDriver::output(peripherals.pins.gpio4)?;

    led.set_high()?;
    camera.get_framebuffer();
    // take two frames to get a fresh one
    let framebuffer = camera.get_framebuffer();
    led.set_low()?;

    if let Some(framebuffer) = framebuffer {
        let image = framebuffer.data();
        let filename = dt_now.format("%Y-%m-%dT%H:%M:%S.jpg").to_string();
        send_image(&filename, image)?;
    } else {
        log::info!("No framebuffer available");
    }

    deep_sleep_until(
        (dt_now + Duration::from_secs(86400))
            .with_time(CONFIG.wakeup_time)
            .unwrap(),
    );

    Ok(())
}

fn send_post_request(uri: &str, headers: &[(&str, &str)], data: &[u8]) -> Result<()> {
    let http_conn = EspHttpConnection::new(&esp_idf_svc::http::client::Configuration {
        timeout: Some(Duration::from_secs(60)),
        buffer_size: Some(4096),
        buffer_size_tx: Some(4096),
        ..Default::default()
    })?;
    let mut client = Client::wrap(http_conn);
    let mut request = client.request(Method::Post, uri, headers)?;
    request.write_all(data)?;
    let response = request.submit()?;
    log::info!(
        "Response status: {}",
        response.status_message().unwrap_or("None")
    );
    Ok(())
}

fn deep_sleep_until(target_time: chrono::DateTime<chrono::Local>) {
    let now = chrono::Local::now();
    if target_time > now {
        let sleep_duration = target_time - now; // chrono::Duration
        let sleep_us = sleep_duration.num_microseconds().unwrap_or(0) as u64;

        log::info!(
            "Deep sleeping until {}, {} s",
            target_time,
            sleep_duration.num_seconds()
        );
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

fn send_health_data(dt: chrono::DateTime<chrono::Local>) -> Result<()> {
    let health_body = serde_json::to_string(&HealthRequest {
        voltage: 0.0,
        timestamp: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
    })?;
    log::info!("Health request body {}", health_body);
    let headers = [
        ("Content-Type", "application/json"),
        ("Content-Length", &health_body.len().to_string()),
    ];

    match send_post_request(
        &format!("{}{}", CONFIG.server_address, CONFIG.health_uri),
        &headers,
        health_body.as_bytes(),
    ) {
        Ok(_) => log::info!("Health request successful"),
        Err(e) => {
            log::error!("Failed to initiate health request: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

fn send_image(filename: &str, image: &[u8]) -> Result<()> {
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let mut body = Vec::new();
    // Build multipart body
    write!(body, "--{}\r\n", boundary)?;
    write!(
        body,
        "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
        filename
    )?;
    write!(body, "Content-Type: image/jpeg\r\n\r\n")?;
    body.extend_from_slice(image);
    write!(body, "\r\n--{}--\r\n", boundary)?;

    let headers: [(&str, &str); 2] = [
        (
            "Content-Type",
            &format!("multipart/form-data; boundary={}", boundary),
        ),
        ("Content-Length", &body.len().to_string()),
    ];

    match send_post_request(
        &format!("{}{}", CONFIG.server_address, CONFIG.upload_uri),
        &headers,
        &body,
    ) {
        Ok(_) => log::info!("Uploaded file {filename}"),
        Err(e) => {
            log::error!("Failed to upload file {filename}: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
