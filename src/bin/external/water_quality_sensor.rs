use esp_hal::{Blocking, analog::adc::{Adc, AdcChannel, AdcConfig, AdcPin}, gpio::AnalogPin , peripherals::ADC1};
use log::info;

const MAX_VOLTAGE:f32 = 3.3;

pub struct WaterQualitySensor<'a,T: AnalogPin + AdcChannel>
{
    adc_pin:AdcPin<T, ADC1<'a>>,
    adc: Adc<'a, ADC1<'a>, Blocking>
}


impl<'a,T: AnalogPin + AdcChannel> WaterQualitySensor<'a,T> {
    /// Steals the ADC because I dont want to make this project like im not 
    /// sped
    pub fn new(
        pin: T,
        adc: ADC1<'a>
    ) -> Self 
    {   
        
        let mut adc_config = AdcConfig::new();
        let adc_pin: AdcPin<T, ADC1<'_>> = adc_config.enable_pin(pin, esp_hal::analog::adc::Attenuation::_11dB);
        let adc1: Adc<'_, ADC1<'_>, Blocking> = Adc::new(adc, adc_config);
        

        return Self {
            adc_pin,
            adc:adc1
        }
    }

    pub fn read_to_serial(&mut self) {
        // max 4.5v min 0.0v
        let value = nb::block!(self.adc.read_oneshot(&mut self.adc_pin)).unwrap();
        info!("{:?} \n Voltage: {:?}",value, value as f64 * (3.3 / 4096.0));
    }


    pub fn read(&mut self) -> u16 {
        return nb::block!(self.adc.read_oneshot(&mut self.adc_pin)).unwrap()
    } 

    /// Taken from https://dfimg.dfrobot.com/nobody/wiki/8e585d98aafe2bab22be39c5b68165c5.pdf
    /// Mandatory reference that you should probably verify this before you drink
    // tap water is around 0.3 NTU
    pub fn read_into_turbidity(&mut self) -> u16 {
        let value = self.read();
        let voltage = value as f64 * (5.0 / 4096.0);
        const SLOPE:f64 = -1572.0;
        // so basically to calculate this we did some maths to find the slope
        // of the test data provided in the datasheet. now mind you this has some guesstimation
        // but it should be good enough to ball out 
        // 	Y = -0.0006357*X + MAX VOLTAGE
        let turbidity = (SLOPE * (scale_3_3v_to_5v(voltage))) + 7099.0;
        // let turbidity = (SLOPE * voltage) + 7099.0;
        return turbidity as u16;
    }

    pub fn read_into_voltage(&mut self, convert_to_5v:bool ) -> f32 {
        let value = self.read();
        let voltage:f32;
        
        if convert_to_5v {
            voltage = value as f32 * (5.0 / 4096.0);
        }
        else {
            voltage = value as f32 * (3.3 / 4096.0);
        }

        return voltage;    
    }
}


/// Would've added this to be apart of WaterQualitySensor but you shouldn't have
/// to provide a generic & lifetime for this 
fn scale_3_3v_to_5v(input_voltage:f64) -> f64 {
    return (    
            (
                (input_voltage - 0.0) * (4.4 - 0.0)
            )
            /
            (
                3.3 - 0.0
            )
        )
        + 0.0 
}