// note: we're going with UUIDV4 until we can get a RTC set up

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

mod external;
mod misc;

use embedded_graphics::geometry::Point;
use embedded_graphics::text::Baseline;
use embedded_hal_bus::spi::AtomicDevice;
use embedded_hal_bus::util::AtomicCell;
use embedded_sdmmc::sdcard::spi::AcquireOpts;
use embedded_sdmmc::{Mode, SdCard, VolumeIdx, VolumeManager};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Output, OutputConfig, OutputPin};
use esp_hal::i2c::master::{Config, I2c, Instance};
use esp_hal::{main, spi};
use esp_hal::spi::master::Spi;
use esp_hal::time::{Duration, Instant};
use heapless::{String, format};
use log::info;


use crate::external::display::SSD1306Display;
use crate::external::sd_card::{self, FakeTimeSource, PolllutionStatus};
use crate::external::water_quality_sensor::WaterQualitySensor;

#[derive(PartialEq,Debug)]
pub enum SDCardStatus {
    Ok,
    Err(String<64>)
}


// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]

#[main]
fn main() -> ! {
    // generator version: 1.4.0
    // generator parameters: -o esp32 -o esp32-wroom-32e -o log -o esp-backtrace

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let delay = Delay::new();
    let rng = esp_hal::rng::Rng::new();

    

    // The following pins are used to bootstrap the chip. They are available
    // for use, but check the datasheet of the module for more information on them.
    // - GPIO0
    // - GPIO2
    // - GPIO5
    // - GPIO12
    // - GPIO15
    // These GPIO pins are in use by some feature of the module and should not be used.
    let _gpio6 = peripherals.GPIO6;
    let _gpio7 = peripherals.GPIO7;
    let _gpio8 = peripherals.GPIO8;
    let _gpio9 = peripherals.GPIO9;
    let _gpio10 = peripherals.GPIO10;
    let _gpio11 = peripherals.GPIO11;
    let _gpio16 = peripherals.GPIO16;
    let _gpio20 = peripherals.GPIO20;
    let i2c = I2c::new(
        peripherals.I2C0,
        Config::default(),
    )
    .unwrap()
    .with_scl(peripherals.GPIO22)
    .with_sda(peripherals.GPIO21);
    let spi: AtomicCell<Spi<'static, esp_hal::Blocking>> = AtomicCell::new(
        Spi::new(
            peripherals.SPI2,
            spi::master::Config::default()
        ).expect("Cannot continue if we cannot initialize SPI")
        .with_miso(peripherals.GPIO19)
        .with_mosi(peripherals.GPIO23)
        .with_sck(peripherals.GPIO18)
    );
    let mut timesource = FakeTimeSource::new(&rng);

    let sd_card_cs_pin: Output<'static> = Output::new(peripherals.GPIO17, esp_hal::gpio::Level::Low, OutputConfig::default());

    // initialize the display
    let mut display = SSD1306Display::new(i2c);
    display.clear();
    display.flush();

    let mut water_quality = WaterQualitySensor::new(peripherals.GPIO34, peripherals.ADC1);
    
    // intialize the sd card 
    let atm_device = AtomicDevice::new(&spi, sd_card_cs_pin, delay.clone()).unwrap();
    // let mut sd_card_interface= SdCard::new(atm_device, delay);
    let mut sd_card_interface = SdCard::new_with_options(atm_device, delay, AcquireOpts { use_crc: true, acquire_retries: 0 });
    
    let mut sd_card_status = SDCardStatus::Ok;
    let hash_str:String<10> = format!("{:X}",timesource.hash).unwrap();
    let oxhash_str:String<64> = format!("0x{hash_str}").unwrap();
    let path:String<8> = format!("{hash_str}").unwrap();

    {
        let volume_mgr = VolumeManager::new(sd_card_interface, timesource);
         
        if let Ok(volume0) = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)) {
            if let Ok(mut d) = volume0.open_root_dir() {
                if d.make_dir_in_dir(path.as_str()).is_err() || d.change_dir(path.as_str()).is_err() {
                    display.clear();
                    display.write_text("Please Reinsert SD Card", Point::zero(), Baseline::Top);
                    display.write_text("and restart", Point { x: 0, y: 10 }, Baseline::Top);
                    display.flush();
                    loop {
                        // infinite loop because we cant go on after this point 
                    }
                }
    
                let f= d.open_file_in_dir("0s.csv", Mode::ReadWriteCreateOrAppend).unwrap();
    
                if f.write(sd_card::csv_header().as_bytes()).is_err() || f.flush().is_err() || f.close().is_err() || d.close().is_err() {
                    display.clear();
                    display.write_text("Please Reinsert SD Card", Point::zero(), Baseline::Top);
                    display.write_text("and restart", Point { x: 0, y: 10 }, Baseline::Top);
                    display.flush();
                    loop {
                        // infinite loop because we cant go on after this point 
                    }
                }
            }
            else {
                sd_card_status = SDCardStatus::Err(format!("NO SD CARD").unwrap());
            }
            volume0.close().unwrap();
        }
        
        (sd_card_interface, timesource) = volume_mgr.free();
    }
    
    match sd_card_interface.get_card_type() {
        Some(x) => {
            info!("{x:?}")
        },
        None => {
            sd_card_status = SDCardStatus::Err(format!("Cannot get SDcard type").unwrap());
            info!("No SD Card Detected")
        },
    }

    display_sequence(&mut display, 0x00, &sd_card_status, &timesource);
     
    let mut last_epoch = Instant::now();
    let mut current_file_name:String<32> = format!("0s.csv").unwrap();
    loop {
        let volume_mgr = VolumeManager::new(sd_card_interface, timesource);
        
        water_quality.read_to_serial();
        let value = water_quality.read_into_voltage(false);
        info!("{value:?}");
        

        // we initialize two instances because "delay_start" should not take into account the time it takes for other
        // lines to run 
        let t = Instant::now();

      

        display.clear();
        display.write_text(&oxhash_str, Point::zero(), embedded_graphics::text::Baseline::Top);
        if 1.0 > value {
            display.write_text("Water OK", Point { x: 0, y: 10 }, Baseline::Top);
        }
        else {
            display.write_text("Polluted, Check water", Point { x: 0, y: 10 }, Baseline::Top);
        }

        // display.write_sensor_voltage(value, Point { x: 0, y: 10 }, embedded_graphics::text::Baseline::Top);
        if sd_card_status == SDCardStatus::Ok {
            display.write_text("SDC: OK", Point { x: 0, y: 20 }, embedded_graphics::text::Baseline::Top);
        }
        else {
            display.write_text("SDC: ERR RESTART MCU", Point { x: 0, y: 20 }, embedded_graphics::text::Baseline::Top);
        }
        display.flush();

        let str = sd_card::format_to_csv(t.duration_since_epoch().as_secs(), PolllutionStatus::Safe, value);
        let mut caught_err = false;
        
        if let Ok(volume0) = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)) {
            match volume0.open_root_dir() {
                Ok(mut dir) => {
                    if dir.change_dir(path.as_str()).is_err() {
                        // just try to make the dir in case its not there
                        _ = dir.make_dir_in_dir(path.as_str());
                        let err_str = format!("Cannot write file. ln{}",line!()).unwrap();
                        info!("{err_str:?}");
                        sd_card_status = SDCardStatus::Err(err_str);
                        caught_err = true;
                    }
    
                    if last_epoch.elapsed().as_hours() >= 24 {
                        last_epoch = t;
                        let new_file_name:String<32> = format!("{}s.csv",last_epoch.duration_since_epoch().as_secs()).unwrap();
                        let file_for_header = dir.open_file_in_dir(new_file_name.as_str(), Mode::ReadWriteCreateOrAppend);
                        if let Ok(f) = file_for_header {
                            if f.write(sd_card::csv_header().as_bytes()).is_ok() && f.close().is_ok() {
                                current_file_name = new_file_name;
                            }

                        }
                    }
    
                    if let Ok(f) = dir.open_file_in_dir(current_file_name.as_str(), Mode::ReadWriteCreateOrAppend) {
                        if f.write(str.as_bytes()).is_err() || f.flush().is_err() || f.close().is_err() {
                            let err_str = format!("Cannot write file. ln{}",line!()).unwrap();
                            info!("{err_str:?}");
                            sd_card_status = SDCardStatus::Err(err_str);
                            caught_err = true;
                        }
                    }
    
    
                    if let SDCardStatus::Err(e) = &sd_card_status {
                        if let Ok(err_file) = dir.open_file_in_dir("err.txt", Mode::ReadWriteCreateOrAppend) {
                            let message:String<128> = format!("Recovered from Error at {}s\nErr:{}\n",t.duration_since_epoch().as_secs(),e.as_str()).unwrap();
                            err_file.write(message.as_bytes()).unwrap();
                        }
                    }
                    
                    if caught_err != true {
                        sd_card_status = SDCardStatus::Ok;
                    }
                },
                Err(e) => {
                    let err_str = format!("{e:?}").unwrap();
                    sd_card_status = SDCardStatus::Err(err_str);
                },
            }
            if volume0.close().is_err() {
                let err_str = format!("COULD NOT CLOSE VOLUME0").unwrap();
                sd_card_status = SDCardStatus::Err(err_str);        
            }
        }
        else {
            let err_str = format!("NO SD CARD FOUND").unwrap();
            sd_card_status = SDCardStatus::Err(err_str);
        }



        (sd_card_interface, timesource) = volume_mgr.free();
        let delay_start = Instant::now(); 
        while delay_start.elapsed() < Duration::from_millis(1250) {}
    }
    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.2.2/examples
}


fn display_sequence(display:&mut SSD1306Display, sd_card_size:u64, sd_card_status:&SDCardStatus,timesource:&FakeTimeSource) {
    // show that all the devices are ok
    display.clear();
    display.write_text("Display: OK", Point { x: 0, y: 0 }, Baseline::Top);
    match sd_card_status {
        SDCardStatus::Ok => {
            display.write_text("SD Card: OK", Point { x: 0, y: 10 }, Baseline::Top);
        },
        SDCardStatus::Err(_) => {
            display.write_text("SD Card: ERR", Point { x: 0, y: 10 }, Baseline::Top);
            display.write_text("WAIT FOR MORE INFO", Point { x: 0, y: 20 }, Baseline::Top);
        },
    }
    
    display.flush();
    let delay_start = Instant::now();
    while delay_start.elapsed() < Duration::from_millis(5000) {};

    let s:String<21>;

    if sd_card_size != 0x00 {
        s = format!("{sd_card_size:X}").unwrap();
    }
    else {
        s = format!("UNK").unwrap();
    }
    let hash:String<21> = format!("Hash: 0x{:X}",timesource.hash).unwrap();


    display.clear();
    
    match sd_card_status {
        SDCardStatus::Ok => {
            display.write_text("SD Card size:", Point::zero(), Baseline::Top);
            display.write_text(s.as_str(), Point { x: 0, y: 10 }, Baseline::Top);
            display.write_text(hash.as_str(), Point { x: 0, y: 20 }, Baseline::Top);
        },
        SDCardStatus::Err(string_inner) => {
            display.write_text(string_inner.as_str(), Point::zero(), Baseline::Top);
            display.write_text("CHECK SD CARD", Point { x: 0, y: 10 }, Baseline::Top);
            display.write_text("CONNECTION & RESTART", Point { x: 0, y: 20 }, Baseline::Top);
                    
        },
    }

    display.flush();
    
    
    let delay_start = Instant::now();
    while delay_start.elapsed() < Duration::from_millis(5000) {}

    let year:String<21> = format!("Year: {}",timesource.fake_year).unwrap();
    let month:String<21> = format!("Month: {}",timesource.fake_month).unwrap();
    let day:String<21> = format!("Day: {}",timesource.fake_day).unwrap();
    display.clear();
    display.write_text("Fake Timesource:", Point { x: 0, y: 0 }, Baseline::Top);
    display.write_text(year.as_str(), Point { x: 0, y: 10 }, Baseline::Top);
    display.write_text(month.as_str(), Point { x: 64, y: 10 }, Baseline::Top);
    display.write_text(day.as_str(), Point { x: 0, y: 20 }, Baseline::Top);
    display.flush();

    let delay_start = Instant::now();
    while delay_start.elapsed() < Duration::from_millis(5000) {};
    display.clear();
}   

#[macro_export]
macro_rules! sd_card_interaction {
    ($func:expr, $sd_card_status:ident) => {
        match $func {
            Ok(_) => {
                // do a lot of nothing
            },
            Err(e) => {
                let err_str = format!("{e:?}").unwrap();
                info!("{err_str:?}");
                $sd_card_status = SDCardStatus::Err(err_str);
            }
        }
    };
}