use embedded_hal_bus::{spi::AtomicDevice, util::AtomicCell} ;
use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use esp_hal::{Blocking, delay::Delay, gpio::{Output, OutputPin}, rng::Rng, spi::master::Spi, time::Instant};
use heapless::{String, Vec, format};
use log::info;

// use crate::external::sd_card::SdCardInterfaceError::Unknown;



// pub struct SDCardInterface<'a,T:TimeSource> {
//     pub size:u64,
//     sd_card_interface: SdCard<AtomicDevice<'a, Spi<'static, Blocking>, Output<'static>, Delay>, Delay>,
//     time: T,
//     current_volume:Option<VolumeIdx>,
// }


// #[derive(Debug)]
// pub enum SdCardInterfaceError {
//     /// We checked and theres no SD card.
//     NoSdCard,
//     /// If you tried to do something that would traditionally require you to be inside of a volume
//     /// but you're not inside of a volume
//     NotInsideVolume,

//     Unknown(String<64>)
// }

pub struct FakeTimeSource {
    pub fake_year:u8,
    pub fake_month:u8,
    pub fake_day:u8,
    pub hash:u32,
}

impl FakeTimeSource {
    pub fn new(
        rng:&Rng,
    ) -> Self {
        let current_rand = rng.random();
        let random_bytes: [u8; 4] = current_rand.to_le_bytes();

        info!("Your fake date is:\nYear: {}\nMonth: {}\nDay: {}\nHash: {:X}"
        ,random_bytes[0],random_bytes[1] % 12 ,random_bytes[2] % 30,current_rand
        );

        return Self {
            fake_year: random_bytes[0],
            fake_month: random_bytes[1] % 12,
            fake_day: random_bytes[2] % 30,
            hash: current_rand
        }
    }
}

impl TimeSource for FakeTimeSource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        return Timestamp {
            year_since_1970: self.fake_year,
            zero_indexed_month: self.fake_month,
            zero_indexed_day: self.fake_day,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }    
    }
}

impl Clone for FakeTimeSource {
    fn clone(&self) -> Self {
        Self { fake_year: self.fake_year.clone(), fake_month: self.fake_month.clone(), fake_day: self.fake_day.clone(), hash: self.hash.clone() }
    }
}

// impl<'a,T:TimeSource> SDCardInterface<'a,T> {
//     pub fn new<'spi: 'a> (
//         cs_pin:Output<'static> ,
//         spi_bus:&'spi AtomicCell<Spi<'static, esp_hal::Blocking>>,
//         mut delay:Delay,
//         timesource:T
//     ) -> Self {
        
//         let x = AtomicDevice::new(spi_bus, cs_pin, delay.clone()).unwrap();
//         let sd_card_interface= SdCard::new(x, delay);
//         // let size = sd_card_interface.num_bytes().unwrap();
//         // let volume_mgr: VolumeManager<SdCard<AtomicDevice<'a, Spi<'static, Blocking>, Output<'_>, Delay>, &mut Delay>, T> = VolumeManager::new(sd_card_interface, timesource);
//         // let size = 0;

//         return Self {
//             size: 0,
//             sd_card_interface,
//             time: timesource,
//             current_volume: None,
//         }
//     }

//     // https://docs.rs/embedded-sdmmc/latest/embedded_sdmmc/struct.SdCard.html#method.num_bytes

//     pub fn open_volume(&mut self,vol_idx:VolumeIdx) -> Result<(),SdCardInterfaceError> {
//         let volume_mgr = VolumeManager::new(self.sd_card_interface., self.time);

//         match volume_mgr.open_volume(vol_idx) {
//             Ok(_) => {
//                 self.current_volume = Some(vol_idx);
                
//             },
//             Err(e) => {
//                 let err:String<64> = format!("{e:?}").unwrap();
//                 return core::prelude::v1::Err(SdCardInterfaceError::Unknown(err))
//             },
//         }

//         Ok(())
//     }

//     pub fn write_log(&mut self,custom_name:Option<&str>) {
//         if let Some(current_dir) = &self.current_directory {
//             let x:String<64> = format!("{}",Instant::now().duration_since_epoch().as_secs()).unwrap();
            
//             let file = current_dir.open_file_in_dir(custom_name
//                 .unwrap_or_else(|| {
//                     x.as_str()
//                 }),
//                 embedded_sdmmc::Mode::ReadWriteCreate
//             );


//             todo!()
//         }
//     }
// }


pub enum PolllutionStatus {
    Polluted,
    LikelyPolluted,
    Safe
}

impl Into<&'static str> for PolllutionStatus {
    fn into(self) -> &'static str {
        match self {
            PolllutionStatus::Polluted => "Polluted",
            PolllutionStatus::LikelyPolluted => "Likely Polluted",
            PolllutionStatus::Safe => "Safe",
        }
    }
}


/// Formats the arguments into a csv row that can be added to a file. 
/// 
/// now will overflow if the device is running for 136 years (lol) 
pub fn format_to_csv(now:u64,status:PolllutionStatus, raw_voltage:f32) -> String<32> {
    return format!(
        "{},{},{:.2}\n",
        now as u32,
        Into::<&'static str>::into(status),
        raw_voltage
    ).unwrap()
}



pub const fn csv_header() -> &'static str{
    //                      (polluted, likely polluted, Safe)
    "Cycles Since Power On,Status,Raw Voltage (3.3v max)\n"
}