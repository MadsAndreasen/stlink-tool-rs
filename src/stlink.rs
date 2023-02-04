use std::time::Duration;

use rusb::{GlobalContext};

const STLINK_VID: u16 = 0x0483;
const STLINK_PID: u16 = 0x3748;
// const STLINK_PIDV21: u16 = 0x374b;
// const STLINK_PIDV21_MSD: u16 = 0x3752;
// const STLINK_PIDV3_MSD: u16 = 0x374e;
// const STLINK_PIDV3: u16 = 0x374f;
// const STLINK_PIDV3_BL: u16 = 0x374d;

const ST_DFU_INFO: u8 =  0xF1;
const ST_DFU_MAGIC: u8 =  0xF3;
const USB_TIMEOUT: Duration = Duration::from_secs(5);

pub struct STLink {
    device: rusb::Device<GlobalContext>
}

impl STLink {
    const ENDPOINT_IN: u8 = 1 | rusb::constants::LIBUSB_ENDPOINT_IN;
    const ENDPOINT_OUT: u8 = 2 | rusb::constants::LIBUSB_ENDPOINT_OUT;

    pub fn new(device: rusb::Device<GlobalContext>) -> Self {
        Self {
            device
        }
    }

    pub(crate) fn print_info(&self)  {
        match self.device.open() {
            Ok(mut handle) => {
                println!("StlinkV21 Bootloader found");
                let command: [u8; 2] = [ST_DFU_INFO, 0x80];
                handle.claim_interface(0).unwrap();
                if let Err(error) = handle.write_bulk(STLink::ENDPOINT_OUT, &command, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let mut data: [u8; 20] = Default::default();
                if let Err(error) = handle.read_bulk(STLink::ENDPOINT_IN, &mut data, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let stlink_version = data[0] >> 4;
                if stlink_version >= 3 {
                    panic!("St linkversion  greater or equal to 3 - Not supported");
                }
                let jtag_version = (data[0] & 0x0F) << 2 | (data[1] & 0xC0) >> 6;
                let swim_version = data[1] & 0x3F;
                let loader_version = u16::from(data[5]) << 8 | u16::from(data[4]);
                println!("Firmware version : V{stlink_version}J{jtag_version}S{swim_version}");
                println!("Loader version : {loader_version}");

                let command: [u8; 2] = [ST_DFU_MAGIC, 0x08];
                handle.claim_interface(0).unwrap();
                if let Err(error) = handle.write_bulk(STLink::ENDPOINT_OUT, &command, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let mut reply: [u8; 20] = Default::default();
                if let Err(error) = handle.read_bulk(STLink::ENDPOINT_IN, &mut reply, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let id = &reply[8..];
                print!("ST-Link ID : ");
                for chunk in id.chunks(4) {
                    for digit in chunk.iter().rev() {
                        print!("{digit:02X}");
                    }
                }
                println!();
            }
            Err(error) => println!("Unable to claim USB interface ! Please close all programs that may communicate with an ST-Link dongle - {error}"),
        }
    }
}

pub fn find_devices() -> Vec<STLink> {
    let mut ret_val = Vec::new();
    for device in rusb::devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();
        if device_desc.vendor_id() == STLINK_VID && device_desc.product_id() == STLINK_PID {
            ret_val.push(STLink::new(device));
        }
    }
    ret_val
}
