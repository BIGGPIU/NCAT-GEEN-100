
use heapless::String;
use log::{error,info};
use esp_hal::{Blocking, i2c::master::I2c}; 
use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder, ascii::FONT_6X10}, pixelcolor::BinaryColor, prelude::*, text::{Baseline, Text},
};
use core::fmt::Write;

use ssd1306::{I2CDisplayInterface, Ssd1306, mode::{BasicMode, BufferedGraphicsMode}, prelude::*};

use crate::external::water_quality_sensor::WaterQualitySensor;
pub struct SSD1306Display<'a> {
    display:Ssd1306<I2CInterface<I2c<'a, Blocking>>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>,
    style:MonoTextStyle<'static,BinaryColor>
}


impl<'b> SSD1306Display<'b> {
    pub fn new(
        i2c_interface:I2c<'b,Blocking>,
    ) -> Self {

        let text_style: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

        let interface = I2CDisplayInterface::new(i2c_interface);

        let mut display = Ssd1306::new(
            interface, 
            DisplaySize128x32,
            DisplayRotation::Rotate0
        ).into_buffered_graphics_mode();
        if let Err(e) = display.init() {
            error!("{e:?}");
            panic!("Could not enable ssd1306")
        }
        
        Self {
            display:display,
            style: text_style,
        }
    }

    pub fn write_text(&mut self, text:&str,point:Point,baseline:Baseline) {
        Text::with_baseline(text, point, self.style, baseline)
        .draw(&mut self.display).unwrap();
    }

    pub fn write_sensor_voltage(&mut self,value:f32, point:Point, baseline:Baseline) {
        let buf:String<32> = heapless::format!("Sensor Value {value:.2}v").unwrap();

        self.write_text(&buf, point, baseline);
    }
    
    pub fn write_water_sensor_value(&mut self, value:u16, point:Point, baseline:Baseline) {
        let buf:String<24> = heapless::format!("Sensor Value {value:?}").unwrap();

        self.write_text(&buf, point, baseline);
    }

    pub fn flush(&mut self) {
        self.display.flush().unwrap();
    }

    pub fn clear(&mut self) {
        self.display.clear_buffer();
    }
}