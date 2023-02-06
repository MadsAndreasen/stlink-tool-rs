use core::time;
use std::{path::PathBuf, time::Duration, thread};

use clap::Parser;
mod stlink;


/// A tool to flash chinese ST-link dongles
/// Application is started when called without argument or after firmware load
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Probe the ST-Link adapter
    #[clap(short, long)]
    probe: bool,

    file: Option<PathBuf>,
}


const OPENMOKO_VID: u16 = 0x1d50;
const BMP_APPL_PID: u16 = 0x6018;

const BMP_DFU_IF: u8 = 4;

fn main() {
    let args = Args::parse();

    if find_and_reboot_black_magic_probes() > 0 {
        thread::sleep(time::Duration::from_secs(2));
    }

    let devices = stlink::find_devices();
    if devices.is_empty() {
        println!("No ST-LINK in DFU mode found. Replug ST-Link to flash");
        std::process::exit(1);
    }

    for device in devices.iter() {
        device.print_info();
        device.get_current_mode();

        if let Some(file) = args.file.clone() {
            device.flash(file);
        }

    }

    if !args.probe {

    }
}


fn find_and_reboot_black_magic_probes() -> isize {
    let mut count: isize = 0;
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
                        u16::from(BMP_DFU_IF),
                        &buf,
                        Duration::from_millis(5000)).unwrap();
                    handle.release_interface(BMP_DFU_IF).unwrap();
                    count += 1;
                },
                Err(error) => println!("... Failed with error: {error}"),
            }
        }
    }
    count

}