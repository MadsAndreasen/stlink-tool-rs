use std::{time::Duration, fs, thread, io::{self, Write}, string::String};
use num_enum::{TryFromPrimitive, IntoPrimitive};
use rusb::{GlobalContext, DeviceHandle};
use std::convert::TryFrom;
use aes::cipher::{block_padding::Pkcs7,BlockEncryptMut, KeyInit, generic_array};

type Aes128EcbEnc = ecb::Encryptor<aes::Aes128>;

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
    device: rusb::Device<GlobalContext>,
    encryption_key: Vec<u8>
}

#[derive(IntoPrimitive, Copy, Clone)]
#[repr(u16)]
enum DownloadType {
    Command = 0,
    Data = 2
}

impl STLink {
    const ENDPOINT_IN: u8 = 1 | rusb::constants::LIBUSB_ENDPOINT_IN;
    const ENDPOINT_OUT: u8 = 2 | rusb::constants::LIBUSB_ENDPOINT_OUT;

    pub fn new(device: rusb::Device<GlobalContext>) -> Self {
        Self {
            device,
            encryption_key: Vec::new()
        }
    }


    pub(crate) fn print_info(&mut self)  {
        match self.device.open() {
            Ok(mut handle) => {
                println!("StlinkV21 Bootloader found");
                let command: [u8; 2] = [ST_DFU_INFO, 0x80];
                handle.claim_interface(0).unwrap();
                if let Err(error) = write_bulk(&handle,STLink::ENDPOINT_OUT, &command, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let mut data: [u8; 20] = Default::default();
                if let Err(error) = read_bulk(&handle,STLink::ENDPOINT_IN, &mut data, USB_TIMEOUT) {
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
                if let Err(error) = write_bulk(&handle,STLink::ENDPOINT_OUT, &command, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let mut reply: [u8; 20] = Default::default();
                if let Err(error) = read_bulk(&handle,STLink::ENDPOINT_IN, &mut reply, USB_TIMEOUT) {
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

                let firmware_key= [&reply[..4], &reply[8..]].concat();
                let key = "I am key, wawawa".as_bytes();
                self.encryption_key = self.encrypt(key, firmware_key.as_slice());

                print!("Firmware encryption key : ");
                for digit in self.encryption_key.iter() {
                    print!("{digit:02X}");
                }
                println!();

                handle.release_interface(0);

            }
            Err(error) => println!("Unable to claim USB interface ! Please close all programs that may communicate with an ST-Link dongle - {error}"),
        }
    }

    fn encrypt(&self, key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut firmware_key_be = Vec::<u8>::new();

        for chunk in data.chunks(4) {
            let i = u32::from_le_bytes(chunk.try_into().unwrap());
            firmware_key_be.extend_from_slice(&i.to_be_bytes());
        }

        let mut key_be = Vec::<u8>::new();
        for chunk in key.chunks(4) {
            let i = u32::from_le_bytes(chunk.try_into().unwrap());
            key_be.extend_from_slice(&i.to_be_bytes());
        }

        let key_as_array =  generic_array::GenericArray::from_slice(key_be.as_slice());
        let enc = Aes128EcbEnc::new(key_as_array);

        let encrypted = enc.encrypt_padded_vec_mut::<Pkcs7>(&firmware_key_be);
        let mut encrypted_data = Vec::<u8>::new();
        for chunk in encrypted[..firmware_key_be.len()].chunks(4) {
            let i = u32::from_le_bytes(chunk.try_into().unwrap());
            encrypted_data.extend_from_slice(&i.to_be_bytes());
        }

        encrypted_data
    }

    pub(crate) fn get_current_mode(&self) -> u16 {
        match self.device.open() {
            Ok(mut handle) => {
                let command = [0xF5];
                handle.claim_interface(0).unwrap();
                if let Err(error) = write_bulk(&handle,STLink::ENDPOINT_OUT, &command, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let mut data: [u8; 20] = Default::default();
                if let Err(error) = read_bulk(&handle,STLink::ENDPOINT_IN, &mut data, USB_TIMEOUT) {
                    println!(" stlink_read_info out transfer failure {error}");
                }
                let mode = u16::from(data[0]) << 8 | u16::from(data[1]);
                println!("Current mode : {mode}");
                handle.release_interface(0);
                mode
            },
            Err(error) => panic!("Unable to claim USB interface ! Please close all programs that may communicate with an ST-Link dongle - {error}"),
        }
    }

    pub(crate) fn flash(&self, file: std::path::PathBuf)  {
        const CHUNK_SIZE: usize = 1 << 10;
        const BASE_OFFSET: u32 = 0x08004000;
        let contents = fs::read(file).expect("Problem accessing {file}");
        let mut address = BASE_OFFSET;
        for chunk in contents.chunks(CHUNK_SIZE) {
            self.erase(address);
            self.set_address(address);
            if let Err(error) = self.dfu_download(chunk, &DownloadType::Data) {
                println!("{error:?}");
            }
            print!(".");
            io::stdout().flush().unwrap();
            address += chunk.len() as u32;
        }
        println!();
    }

    fn dfu_download(&self, data: &[u8], download_type: &DownloadType) -> Result<(), String> {
        match self.device.open() {
            Ok(mut handle) => {
                handle.claim_interface(0).unwrap();

                const DFU_DOWNLOAD: u8 = 0x01;
                let data_len: u16 = data.len().try_into().unwrap();
                let mut download_request: [u8; 16] = Default::default();
                download_request[0] = ST_DFU_MAGIC;
                download_request[1] = DFU_DOWNLOAD;

                download_request[2..4].clone_from_slice(u16::from(*download_type).to_le_bytes().as_slice());
                download_request[4..6].clone_from_slice(checksum(data).to_le_bytes().as_slice());
                download_request[6..8].clone_from_slice(data_len.to_le_bytes().as_slice());

                if let Err(error) = write_bulk(&handle,STLink::ENDPOINT_OUT, &download_request, USB_TIMEOUT) {
                    return Err(format!("dfu request transfer failure {error}"));
                }

                let encrypted_data = match download_type {
                    DownloadType::Data => self.encrypt(&self.encryption_key, data),
                    _ => data.to_vec()

                };
                if let Err(error) = write_bulk(&handle,STLink::ENDPOINT_OUT, &encrypted_data, USB_TIMEOUT) {
                    return Err(format!("dfu data transfer failure {error}"));
                }

                match self.dfu_status(&handle) {
                    Err(error) => {
                        return Err(format!("dfu status failure {error}"));
                    }
                    Ok(status) => {
                        if status.state != DeviceState::DfuDnbusy || status.status != DeviceStatus::Ok {
                            return Err("Unexpected DFU status".to_string());
                        }
                        thread::sleep(status.poll_timeout);
                    },
                }

                match self.dfu_status(&handle) {
                    Ok(status) => {
                        if status.state != DeviceState::DfuDnloadIdle {
                            if status.status == DeviceStatus::ErrVendor {
                                return Err("Read-only protection active".to_string());
                            } else if status.status == DeviceStatus::ErrTarget {
                                return Err("Invalid address error".to_string());
                            } else {
                                return Err(format!("Unknown error : {:?}", status.status));
                            }
                        }
                    },
                    Err(error) => return Err(format!("dfu status failure {error}")),
                }
                Ok(())
            },
            Err(error) => panic!("Unable to claim USB interface ! Please close all programs that may communicate with an ST-Link dongle - {error}"),
        }
    }

    fn erase(&self, address: u32) -> Result<(), String> {
        const ERASE_CMD: u8 = 0x41;
        let mut command: Vec<u8> = Vec::with_capacity(5);
        command.push(ERASE_CMD);
        command.extend(address.to_le_bytes());
        self.dfu_download(&command, &DownloadType::Command)?;
        Ok(())
    }

    fn dfu_status(&self, handle: &DeviceHandle<GlobalContext> ) -> Result<DFUStatus, rusb::Error> {
        const DFU_GET_STATUS: u8 = 0x03;
        let command = [ST_DFU_MAGIC, DFU_GET_STATUS, 0, 0, 0, 0, 0x06];
        if let Err(error) = write_bulk(handle,STLink::ENDPOINT_OUT, &command, USB_TIMEOUT) {
            println!(" stlink_read_info out transfer failure {error}");
        }
        let mut data: [u8; 20] = Default::default();
        if let Err(error) = read_bulk(handle,STLink::ENDPOINT_IN, &mut data, USB_TIMEOUT) {
            println!(" stlink_read_info out transfer failure {error}");
        }

        let milliseconds = u32::from(data[1]) | u32::from(data[2]) << 8 | u32::from(data[3]) <<16;
        let status = DFUStatus {
            status: DeviceStatus::try_from(data[0]).unwrap(),
            state: DeviceState::try_from(data[4]).unwrap(),
            poll_timeout: Duration::from_millis(u64::from(milliseconds)),
        };
        Ok(status)
}

    fn set_address(&self, address: u32) -> Result<(), String> {
        const SET_ADDRESS_POINTER_COMMAND: u8 = 0x21;
        let mut command: Vec<u8> = Vec::with_capacity(5);
        command.push(SET_ADDRESS_POINTER_COMMAND);
        command.extend(address.to_le_bytes());
        self.dfu_download(&command, &DownloadType::Command)?;
        Ok(())
    }

}

#[derive(Default, Debug)]
struct DFUStatus {
    status: DeviceStatus,
    state: DeviceState,
    poll_timeout: Duration,
}

#[derive(Default, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum DeviceStatus {
    #[default]
    Ok ,
    ErrTarget ,
    ErrFile ,
    ErrWrite ,
    ErrErase ,
    ErrCheckErased ,
    ErrProg ,
    ErrVerify ,
    ErrAddress ,
    ErrNotdone ,
    ErrFirmware ,
    ErrVendor ,
    ErrUsbr ,
    ErrPor ,
    ErrUnknown ,
    ErrStalledpkt,
}

#[derive(Default, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum DeviceState {
    #[default]
    AppIdle = 0,
    AppDetach = 1,
    DfuIdle = 2,
    DfuDnloadSync = 3,
    DfuDnbusy = 4,
    DfuDnloadIdle = 5,
    DfuManifestSync = 6,
    DfuManifest = 7,
    DfuManifestWaitReset = 8,
    DfuUploadIdle = 9,
    DfuError = 10

}

fn checksum(data: &[u8]) -> u16 {
    let mut sum: i32 =  0;
    for i in data {
        sum += *i as i32;
    }
    sum &= 0xFFFF;
    sum as u16
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

fn write_bulk(handle: &DeviceHandle<GlobalContext>, endpoint: u8, command: &[u8], timeout: Duration ) -> Result<usize, rusb::Error> {
    let log_str = bytes_as_hex(command);
    debug!("> {log_str}");
    handle.write_bulk(endpoint, command, timeout)
}

fn bytes_as_hex(bytes: &[u8]) -> String {
    let mut as_string = String::from("");
    for digit in bytes {
        as_string += &format!("{digit:02X}");
    }
    as_string
}

fn read_bulk(handle: &DeviceHandle<GlobalContext>, endpoint: u8, data: &mut [u8], timeout: Duration) -> Result<usize, rusb::Error> {
    let result = handle.read_bulk(endpoint, data, timeout);
    let log_str = bytes_as_hex(data);
    debug!("< {log_str}");
    result
}
