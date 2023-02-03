use std::{path::PathBuf, time::Duration};

use clap::Parser;

/// A tool to flash chinese ST-link dongles
/// Application is started when called without argument or after firmware load
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Probe the ST-Link adapter
    #[clap(short, long)]
    probe: Option<bool>,

    file: Option<PathBuf>,
}

// const STLINK_VID: u16 = 0x0483;
// const STLINK_PID: u16 = 0x3748;
// const STLINK_PIDV21: u16 = 0x374b;
// const STLINK_PIDV21_MSD: u16 = 0x3752;
// const STLINK_PIDV3_MSD: u16 = 0x374e;
// const STLINK_PIDV3: u16 = 0x374f;
// const STLINK_PIDV3_BL: u16 = 0x374d;

const OPENMOKO_VID: u16 = 0x1d50;
const BMP_APPL_PID: u16 = 0x6018;

const BMP_DFU_IF: u8 = 4;

fn main() {
    // let args = Args::parse();

    for device in rusb::devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        if device_desc.vendor_id() == OPENMOKO_VID && device_desc.product_id() == BMP_APPL_PID {
            println!("Found BMP. Switching to bootloader");

            match device.open() {
                Ok(mut handle) => {
                    let buf: [u8; 0] = [];
                    handle.claim_interface(BMP_DFU_IF).unwrap();
                    handle.write_control(
                        rusb::constants::LIBUSB_ENDPOINT_OUT | rusb::constants::LIBUSB_REQUEST_TYPE_CLASS | rusb::constants::LIBUSB_RECIPIENT_INTERFACE,
                        0, /*DFU_DETACH,*/
                        1000,
                        BMP_DFU_IF as u16,
                        &buf,
                        Duration::from_millis(5000)).unwrap();
                    handle.release_interface(0).unwrap();
                },
                Err(error) => println!("... Failed with error: {error}"),
            }
        }
    }
}
