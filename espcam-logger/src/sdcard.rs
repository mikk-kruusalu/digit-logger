use anyhow::Result;
use chrono::{Datelike, Timelike};
use embedded_sdmmc::{BlockDevice, SdCard, VolumeIdx, VolumeManager};
use embedded_sdmmc::{TimeSource, Timestamp};
use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::spi;
use std::borrow::Borrow;

struct SDTimeSource;

impl TimeSource for SDTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        let now = chrono::Local::now();
        Timestamp {
            year_since_1970: now
                .years_since(
                    chrono::DateTime::from_timestamp(0, 0)
                        .unwrap()
                        .with_timezone(&chrono::Local),
                )
                .unwrap() as u8,
            zero_indexed_month: now.month0() as u8,
            zero_indexed_day: now.day0() as u8,
            hours: now.hour() as u8,
            minutes: now.minute() as u8,
            seconds: now.second() as u8,
        }
    }
}

pub struct SDCard<
    'd,
    T: Borrow<spi::SpiDriver<'d>> + 'd,
    SPI: embedded_hal::spi::SpiDevice<u8>,
    DELAYER: embedded_hal::delay::DelayNs,
    D: BlockDevice,
    C: TimeSource,
> {
    spi_driver: spi::SpiDeviceDriver<'d, T>,
    sd_card: SdCard<SPI, DELAYER>,
    volume_mgr: VolumeManager<D, C, 4, 4>,
    volume: VolumeIdx,
}

impl<'d, T, SPI, DELAYER, D, C> SDCard<'d, T, SPI, DELAYER, D, C>
where
    T: Borrow<spi::SpiDriver<'d>> + 'd,
    SPI: embedded_hal::spi::SpiDevice<u8>,
    DELAYER: embedded_hal::delay::DelayNs,
    D: BlockDevice,
    C: TimeSource,
{
    pub fn new(peripherals: Peripherals, volume: VolumeIdx) -> Result<Self> {
        log::info!("Initialising SD card...");
        let spi_driver = spi::SpiDeviceDriver::new_single(
            peripherals.spi2,
            peripherals.pins.gpio14,
            peripherals.pins.gpio15,
            Some(peripherals.pins.gpio2),
            Some(peripherals.pins.gpio13),
            &spi::SpiDriverConfig::default(),
            &spi::SpiConfig::default().baudrate(Hertz(200_000)),
        )?;

        let sd_card = SdCard::new(spi_driver, Delay::new(1000));
        let sd_size = match sd_card.num_bytes() {
            Ok(size) => size,
            Err(err) => return Err(anyhow::Error::msg("Failed to initialise SD card")),
        };
        log::info!("SD card initialised");
        log::info!("SD card size is {} bytes", sd_size);
        log::info!("SD card type {:?}", sd_card.get_card_type().unwrap());

        let volume_mgr = VolumeManager::new(sd_card, SDTimeSource {});

        Ok(Self {
            spi_driver,
            sd_card,
            volume_mgr,
            volume,
        })
    }
}
