use lazy_static::lazy_static;
use crate::{str_to_fat_name};

use core::fmt::Write;

use alloc::string::ToString;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use spin::Mutex;
use crate::{println, print};
use core::{fmt};

pub struct DirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub reserved: u8,
    pub creation_time_tenths: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub first_cluster_high: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_low: u16,
    pub file_size: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct BiosParameterBlock {
    pub _jmp: [u8; 3],
    pub _oem: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_table_count: u8,
    pub root_entries: u16,
    pub total_sectors_16: u16,
    pub media_descriptor: u8,
    pub fat_size_16: u16,
    pub sectors_per_track: u16,
    pub heads_on_media: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct ExtendedBootRecord32 {
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fat_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot: u16,
    pub _reserved: [u8; 12],
    pub drive_number: u8,
    pub _win_nt_flags: u8,
    pub signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_id: [u8; 8],
}

pub struct RamDisk {
    data: Vec<u8>,
}

pub struct FileSystem<'a, D: BlockDevice> {
    device: &'a mut D,
    pub bpb: BiosParameterBlock,
    pub ebr: ExtendedBootRecord32,
    pub fat_start: u32,
    cluster_heap_start: u32,
    root_dir_cluster: u32,
}

pub trait BlockDevice {
    fn read_sector(&mut self, lba: u64, buf: &mut [u8;512]) -> Result<(), ()>;
    fn write_sector(&mut self, lba: u64, buf: &[u8;512]) -> Result<(), ()>;
    fn raw_data_mut(&mut self) -> &mut [u8];
}

impl DirEntry{
    pub fn new(name: [u8; 11], cluster: u32, attr: u8) -> Self {
        DirEntry {
            name,
            attr,
            reserved: 0,
            creation_time_tenths: 0,
            creation_time: 0,
            creation_date: 0,
            last_access_date: 0,
            first_cluster_high: (cluster >> 16) as u16,
            write_time: 0,
            write_date: 0,
            first_cluster_low: (cluster & 0xFFFF) as u16,
            file_size: 0,
        }
    }

    pub fn first_cluster(&self) -> u32 {
        ((self.first_cluster_high as u32) << 16) | (self.first_cluster_low as u32)
    }

    pub fn get_name(&self) -> Result<String, core::str::Utf8Error> {
        let (name_bytes, ext_bytes) = self.name.split_at(8);

        let name_str = core::str::from_utf8(name_bytes)?.trim_end();
        let ext_str = core::str::from_utf8(ext_bytes)?.trim_end();

        if ext_str.is_empty() {
            Ok(name_str.to_string())
        } else {
            Ok(format!("{}.{}", name_str, ext_str))
        }
    }

    pub fn is_directory(&self) -> bool {
        self.attr & 0x10 != 0
    }


    pub fn serialize(&self) -> [u8; 32] {
        let mut buffer = [0u8; 32];

        buffer[0..11].copy_from_slice(&self.name);

        buffer[11] = self.attr;
        buffer[12] = self.reserved;
        buffer[13] = self.creation_time_tenths;

        buffer[14..16].copy_from_slice(&self.creation_time.to_le_bytes());
        buffer[16..18].copy_from_slice(&self.creation_date.to_le_bytes());
        buffer[18..20].copy_from_slice(&self.last_access_date.to_le_bytes());
        buffer[20..22].copy_from_slice(&self.first_cluster_high.to_le_bytes());
        buffer[22..24].copy_from_slice(&self.write_time.to_le_bytes());
        buffer[24..26].copy_from_slice(&self.write_date.to_le_bytes());
        buffer[26..28].copy_from_slice(&self.first_cluster_low.to_le_bytes());
        buffer[28..32].copy_from_slice(&self.file_size.to_le_bytes());

        buffer
    }

    pub fn deserialize(data: &[u8; 32]) -> Self {
        let mut name = [0u8; 11];
        name.copy_from_slice(&data[0..11]);

        Self {
            name,
            attr: data[11],
            reserved: data[12],
            creation_time_tenths: data[13],
            creation_time: u16::from_le_bytes(data[14..16].try_into().unwrap()),
            creation_date: u16::from_le_bytes(data[16..18].try_into().unwrap()),
            last_access_date: u16::from_le_bytes(data[18..20].try_into().unwrap()),
            first_cluster_high: u16::from_le_bytes(data[20..22].try_into().unwrap()),
            write_time: u16::from_le_bytes(data[22..24].try_into().unwrap()),
            write_date: u16::from_le_bytes(data[24..26].try_into().unwrap()),
            first_cluster_low: u16::from_le_bytes(data[26..28].try_into().unwrap()),
            file_size: u32::from_le_bytes(data[28..32].try_into().unwrap()),
        }
    }
}

impl<'a, D: BlockDevice> FileSystem<'a, D> {
    fn init_fat_helper(fat_data: &mut [u8], media_descriptor: u8) {
        fat_data.fill(0);

        // Entry 0: 0x0FFF_FF0 | media_descriptor
        let entry0 = 0x0FFFFFF0u32 | (media_descriptor as u32);
        fat_data[0..4].copy_from_slice(&entry0.to_le_bytes());

        // Entry 1: reserved cluster (0xFFFF_FFFF)
        fat_data[4..8].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    }

    pub fn init_fats(&mut self) {
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };

        let media_descriptor = self.bpb.media_descriptor;

        let reserved = self.bpb.reserved_sectors as usize;
        let fat_size_bytes = fat_size_sectors * bytes_per_sector;

        let data = self.device.raw_data_mut();

        let mut byte_offset = reserved * bytes_per_sector;

        for i in 0..self.bpb.fat_table_count as usize {
            let end = byte_offset + fat_size_bytes;
            assert!(
                end <= data.len(),
                "FAT copy {} out of disk bounds (offset {} len {})",
                i,
                byte_offset,
                fat_size_bytes
            );

            let fat_slice = &mut data[byte_offset..end];
            Self::init_fat_helper(fat_slice, media_descriptor);

            byte_offset += fat_size_bytes;
        }
    }

    pub fn count_occupied_clusters(&mut self) -> usize {
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };
        let fat_bytes_len = fat_size_sectors * bytes_per_sector;

        let disk_bytes: &mut [u8] = self.device.raw_data_mut();

        let fat_start_byte = reserved * bytes_per_sector;

        let fat_slice = &disk_bytes[fat_start_byte .. fat_start_byte + fat_bytes_len];

        let mut count = 0;
        for idx in (0 .. fat_slice.len()).step_by(4) {
            let entry = u32::from_le_bytes(
                fat_slice[idx .. idx + 4]
                    .try_into()
                    .unwrap()
            );
            if entry != 0 {
                count += 1;
            }
        }

        count
    }

    fn write_fat_entry(&mut self, cluster_idx: u32, value: u32) {
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let fat_table_count = self.bpb.fat_table_count;
        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };

        let fat_bytes_len = fat_size_sectors * bytes_per_sector;

        let data: &mut [u8] = self.device.raw_data_mut();
        let first_fat_offset = reserved * bytes_per_sector;

        let bytes = value.to_le_bytes();

        for i in 0..fat_table_count {
            let fat_i_offset = first_fat_offset + (i as usize * fat_bytes_len);
            let entry_offset = fat_i_offset + (cluster_idx as usize * 4);
            data[entry_offset..entry_offset + 4].copy_from_slice(&bytes);
        }
    }

    pub fn allocate_cluster(&mut self) -> Option<u32> {
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };
    
        

        let fat_bytes_len = fat_size_sectors * bytes_per_sector;
        let data: &mut [u8] = self.device.raw_data_mut();

        let first_fat_offset = reserved * bytes_per_sector;
        let num_fat_entries = fat_bytes_len / 4;

        for cluster_idx in 2..num_fat_entries {
            let byte_offset = first_fat_offset + (cluster_idx * 4);
            let fat_entry = &mut data[byte_offset..byte_offset + 4];
            let entry = u32::from_le_bytes(
                fat_entry.try_into().unwrap()
            );
            if entry == 0 {
                fat_entry.copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
                return Some(cluster_idx as u32);
            }
        }

        None
    }

    pub fn free_cluster_chain(&mut self, start_cluster: u32) -> Result<(), ()> {
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };

        let first_fat_offset = reserved * bytes_per_sector;

        let fat_bytes_len = fat_size_sectors * bytes_per_sector;
        let fat_table_count = self.bpb.fat_table_count;
        let data = self.device.raw_data_mut();


        let mut current_cluster = start_cluster;

        while current_cluster < 0x0FFF_FFF8 {
            let next_cluster = {
                let fat_offset = first_fat_offset + (current_cluster as usize * 4);
                let entry = u32::from_le_bytes(
                    data[fat_offset..fat_offset+4].try_into().unwrap()
                );
                entry
            };

            for i in 0..fat_table_count {
                let fat_offset = first_fat_offset + (i as usize * fat_bytes_len);
                let entry_offset = fat_offset + (current_cluster as usize * 4);
                data[entry_offset..entry_offset+4].copy_from_slice(&0u32.to_le_bytes());
            }

            current_cluster = next_cluster;
        }
        Ok(())
    }

    pub fn zero_cluster_data(&mut self, cluster_idx: usize) {
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let sectors_per_cluster = self.bpb.sectors_per_cluster as usize;
        let fat_table_count = self.bpb.fat_table_count as usize;

        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };

        let data_start = (reserved + fat_table_count * fat_size_sectors) * bytes_per_sector;
        let cluster_offset = (cluster_idx - 2) * sectors_per_cluster * bytes_per_sector;

        let cluster_size = sectors_per_cluster * bytes_per_sector;

        let start_cluster = data_start + cluster_offset;
        let end_cluster = start_cluster + cluster_size;

        let data = self.device.raw_data_mut();

        for byte in &mut data[start_cluster..end_cluster] {
            *byte = 0;
        }
    }

    pub fn return_tree(&mut self, cluster_idx: u32, depth: usize) -> String{
        let entries = self.read_dir_entries(cluster_idx as usize);

        let mut out = String::new();

        for entry in entries {
            for _ in 0..depth {
                out.push_str("\\ ");
            }

            if entry.is_directory() {
                let _ = writeln!(out, "* {}", entry.get_name().unwrap().trim_end());
                let name = entry.get_name().unwrap();

                if name != "." && name != ".." {
                    let subtree = self.return_tree(entry.first_cluster(), depth + 1);
                    out.push_str(&subtree);
                }

            } else {
                let _ = writeln!(out, "* {}", entry.get_name().unwrap());
            }        
        }
        out
    }

    pub fn print_tree(&mut self, cluster_idx: u32, depth: usize) {
        let entries = self.read_dir_entries(cluster_idx as usize);

        for entry in entries {
            for _ in 0..depth {
                print!("\\ ");
            }

            if entry.is_directory() {
                println!("* {}", entry.get_name().unwrap().trim_end());

                if entry.get_name().unwrap().trim_end() != "." && entry.get_name().unwrap().trim_end() != ".." {
                    self.print_tree(entry.first_cluster(), depth + 1);
                }

            } else {
                println!("* {}", entry.get_name().unwrap());
            }
        }
    }

    pub fn allocate_dir_entry(&mut self, new_entry: DirEntry, cluster_idx: usize) -> Option<u32>{
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;

        let sectors_per_cluster = self.bpb.sectors_per_cluster as usize;
        let fat_table_count = self.bpb.fat_table_count as usize;

        let fat_size_sectors = match self.bpb.fat_size_16 {
            0 => self.ebr.fat_size_32 as usize,
            n => n as usize,
        };

        let data_start = (reserved + fat_table_count * fat_size_sectors) * bytes_per_sector;
        let cluster_offset = (cluster_idx - 2) * sectors_per_cluster * bytes_per_sector;

        let cluster_size = sectors_per_cluster * bytes_per_sector;
        let entries_per_cluster = cluster_size / 32;

        let start_cluster = data_start + cluster_offset;

        let data = self.device.raw_data_mut();

        for entry_idx in 0..entries_per_cluster {
            let byte_offset = start_cluster + (entry_idx * 32);

            let dir_slice = &mut data[byte_offset..byte_offset + 32];
            let first_byte = dir_slice[0];
            if first_byte == 0x00 || first_byte == 0xE5 {
                dir_slice.copy_from_slice(&new_entry.serialize());
                return Some(entry_idx as u32);
            }
        }

        None
    }
    pub fn create_dir(&mut self, parent_dir_cluster: u32, filename: &str) -> Result<(), ()> {
        let cluster = self.allocate_cluster().unwrap();

        let entry: DirEntry = DirEntry::new(str_to_fat_name(filename), cluster, 0x10);

        self.zero_cluster_data(cluster as usize);


        let dot = DirEntry::new(*b".          ", cluster as u32, 0x10);
        let dot2 = DirEntry::new(*b"..         ", parent_dir_cluster as u32, 0x10);

        self.allocate_dir_entry(dot, cluster as usize).unwrap();
        self.allocate_dir_entry(dot2, cluster as usize).unwrap();

        self.allocate_dir_entry(entry, parent_dir_cluster as usize);
        Ok(())
    }

    pub fn create_root_dir(&mut self) -> Result<(), ()> {
        let root_cluster = self.ebr.root_cluster as usize;

        self.zero_cluster_data(root_cluster);

        let dot = DirEntry::new(*b".          ", root_cluster as u32, 0x10);
        let dot2 = DirEntry::new(*b"..         ", root_cluster as u32, 0x10);

        self.allocate_dir_entry(dot, root_cluster).unwrap();
        self.allocate_dir_entry(dot2, root_cluster).unwrap();


        self.write_fat_entry(root_cluster as u32, 0x0FFF_FFFFu32);

        Ok(())
    }

    pub fn find_dir_in(&mut self, cluster: u32, name: &str) -> Option<u32> {
        let entries = self.read_dir_entries(cluster as usize);
        for entry in entries {
            if entry.is_directory() {
                if let Ok(entry_name) = entry.get_name() {
                    if entry_name.trim_end() == name {
                        return Some(entry.first_cluster());
                    }
                }
            }
        }
        None
    }

    pub fn read_dir_entries(&mut self, cluster_idx: usize) -> Vec<DirEntry> {
        let reserved = self.bpb.reserved_sectors as usize;
        let bytes_per_sector = self.bpb.bytes_per_sector as usize;
        let sectors_per_cluster = self.bpb.sectors_per_cluster as usize;
        let fat_table_count = self.bpb.fat_table_count as usize;

        let fat_size_sectors = if self.bpb.fat_size_16 == 0 {
            self.ebr.fat_size_32 as usize
        } else {
            self.bpb.fat_size_16 as usize
        };

        let data_start = (reserved + fat_table_count * fat_size_sectors) * bytes_per_sector;

        let cluster_offset = (cluster_idx - 2) * sectors_per_cluster * bytes_per_sector;

        let start_cluster = data_start + cluster_offset;
        let cluster_size = sectors_per_cluster * bytes_per_sector;

        let data = self.device.raw_data_mut();

        let entries_per_cluster = cluster_size / 32;

        let mut entries = Vec::new();

        for entry_idx in 0..entries_per_cluster {
            let byte_offset = start_cluster + (entry_idx * 32);

            let dir_slice = &data[byte_offset..byte_offset + 32];

            let first_byte = dir_slice[0];

            if first_byte == 0x00 {
                break; // no more entries
            }
            if first_byte == 0xE5 {
                continue; // deleted entry, skip
            }

            let entry = DirEntry::deserialize(dir_slice.try_into().unwrap());
            entries.push(entry);
        }
        entries
    }

    pub fn create_file(&mut self, parent_dir_cluster: u32, filename: &str) -> Result<(), ()> {
        // Allocate cluster(s) for file
        let cluster = self.allocate_cluster().unwrap();
        let entry: DirEntry = DirEntry::new(str_to_fat_name(filename), cluster, 0x20);
        // Create directory entry in parent_dir_cluster
        self.zero_cluster_data(cluster as usize);

        self.allocate_dir_entry(entry, parent_dir_cluster as usize);
        // Initialize file entry fields
        Ok(())
    }

    pub fn new(device: &'a mut D) -> Result<Self, ()> {     
        let mut sector = [0u8; 512];
        device.read_sector(0, &mut sector)?;

        let bpb = BiosParameterBlock::from_bytes(&sector[0..36])?;

        let ebr = ExtendedBootRecord32::from_bytes(&sector[36..90])?;


        let fat_start = bpb.reserved_sectors as u32;
        let cluster_heap_start = fat_start + (ebr.fat_size_32 * bpb.fat_table_count as u32);
        let root_dir_cluster = ebr.root_cluster;

        Ok(Self {
            device,
            bpb,
            ebr,
            fat_start,
            cluster_heap_start,
            root_dir_cluster,
        })
    }
}

lazy_static! {
    pub static ref BLOCK_DEVICE: Mutex<RamDisk> = {
        let size_in_sectors = 400;
        let sectors_per_cluster = 8;
        let reserved_sectors = 32;
        let fat_count = 2;
        let fat_size = 100;
        let root_cluster = 2;
        let fs_info_sector = 1;
        let backup_boot_sector = 6;
        let volume_id = 0x12345678;
        let volume_label = *b"NO NAME    ";

        let disk = RamDisk::new(
            size_in_sectors,
            sectors_per_cluster,
            reserved_sectors,
            fat_count,
            fat_size,
            root_cluster,
            fs_info_sector,
            backup_boot_sector,
            volume_id,
            volume_label,
        );

        Mutex::new(disk)
    };
}


impl RamDisk {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        size_in_sectors: usize,
        sectors_per_cluster: u8,
        reserved_sectors: u16,
        fat_count: u8,
        fat_size: u32,
        root_cluster: u32,
        fs_info_sector: u16,
        backup_boot_sector: u16,
        volume_id: u32,
        volume_label: [u8; 11],
    ) -> Self {

        let total_sectors = reserved_sectors as usize + (fat_count as usize * fat_size as usize);
        if total_sectors > size_in_sectors {
            panic!(
                "Provided size_in_sectors ({}) is too small; at least {} sectors are required to fit reserved + FAT + data regions.",
                size_in_sectors, total_sectors
            );        
        }
        let mut data = vec![0u8; size_in_sectors * 512];

        println!("{}", size_in_sectors * 512);

        // Basic jump and OEM
        data[0] = 0xEB;
        data[1] = 0x58;
        data[2] = 0x90;
        data[3..11].copy_from_slice(b"MSWIN4.1");

        // BIOS Parameter Block
        data[11..13].copy_from_slice(&512u16.to_le_bytes()); // bytes/sector
        data[13] = sectors_per_cluster;
        data[14..16].copy_from_slice(&reserved_sectors.to_le_bytes());
        data[16] = fat_count;
        data[17..19].copy_from_slice(&0u16.to_le_bytes()); // root entries

        data[19..21].copy_from_slice(&(if size_in_sectors < 65536 {
            size_in_sectors as u16} else {0}).to_le_bytes());

        data[21] = 0xF8; // media descriptor
        data[22..24].copy_from_slice(&0u16.to_le_bytes()); // fat_size_16
        data[24..26].copy_from_slice(&63u16.to_le_bytes()); // sectors/track
        data[26..28].copy_from_slice(&255u16.to_le_bytes()); // heads
        data[28..32].copy_from_slice(&0u32.to_le_bytes()); // hidden sectors
        data[32..36].copy_from_slice(&(if size_in_sectors >= 65536 {
            size_in_sectors as u32
        } else {
                0
            })
            .to_le_bytes());

        // Extended Boot Record
        data[36..40].copy_from_slice(&fat_size.to_le_bytes());
        data[40..42].copy_from_slice(&0u16.to_le_bytes()); // ext flags
        data[42..44].copy_from_slice(&0u16.to_le_bytes()); // fat version
        data[44..48].copy_from_slice(&root_cluster.to_le_bytes());
        data[48..50].copy_from_slice(&fs_info_sector.to_le_bytes());
        data[50..52].copy_from_slice(&backup_boot_sector.to_le_bytes());

        // Drive/boot fields
        data[64] = 0x80;
        data[65] = 0x00; // Windows NT flags
        data[66] = 0x29; // signature
        data[67..71].copy_from_slice(&volume_id.to_le_bytes());
        data[71..82].copy_from_slice(&volume_label);
        data[82..90].copy_from_slice(b"FAT32   ");

        // Boot sector signature
        data[510] = 0x55;
        data[511] = 0xAA;

        Self { data }
    }
}


impl BiosParameterBlock {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, ()> {
        if buf.len() < 36 {
            return Err(());
        }
        Ok(Self {
            _jmp: [buf[0], buf[1], buf[2]],
            _oem: buf[3..11].try_into().unwrap(),

            bytes_per_sector: u16::from_le_bytes(buf[11..13].try_into().unwrap()),
            sectors_per_cluster: buf[13],
            reserved_sectors: u16::from_le_bytes(buf[14..16].try_into().unwrap()),
            fat_table_count: buf[16],
            root_entries: u16::from_le_bytes(buf[17..19].try_into().unwrap()),
            total_sectors_16: u16::from_le_bytes(buf[19..21].try_into().unwrap()),
            media_descriptor: buf[21],
            fat_size_16: u16::from_le_bytes(buf[22..24].try_into().unwrap()),
            sectors_per_track: u16::from_le_bytes(buf[24..26].try_into().unwrap()),
            heads_on_media: u16::from_le_bytes(buf[26..28].try_into().unwrap()),
            hidden_sectors: u32::from_le_bytes(buf[28..32].try_into().unwrap()),
            total_sectors_32: u32::from_le_bytes(buf[32..36].try_into().unwrap()),
        })
    }
}

impl ExtendedBootRecord32 {
    pub fn from_bytes(buf: &[u8]) -> Result<Self, ()> {
        if buf.len() < 54 {
            return Err(());
        }
        Ok(Self {
            fat_size_32: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            ext_flags: u16::from_le_bytes(buf[4..6].try_into().unwrap()),
            fat_version: u16::from_le_bytes(buf[6..8].try_into().unwrap()),
            root_cluster: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
            fs_info: u16::from_le_bytes(buf[12..14].try_into().unwrap()),
            backup_boot: u16::from_le_bytes(buf[14..16].try_into().unwrap()),
            _reserved: buf[16..28].try_into().unwrap(),
            drive_number: buf[28],
            _win_nt_flags: buf[29],
            signature: buf[30],
            volume_id: u32::from_le_bytes(buf[31..35].try_into().unwrap()),
            volume_label: buf[35..46].try_into().unwrap(),
            fs_id: buf[46..54].try_into().unwrap(),
        })
    }
}

impl BlockDevice for RamDisk {
    fn read_sector(&mut self, lba: u64, buf: &mut [u8; 512]) -> Result<(), ()> {
        let start = (lba as usize) * 512;
        let end = start + 512;
        if end > self.data.len() {
            return Err(());
        }
        buf.copy_from_slice(&self.data[start..end]);
        Ok(())
    }

    fn write_sector(&mut self, lba: u64, buf: &[u8; 512]) -> Result<(), ()> {
        let start = (lba as usize) * 512;
        let end = start + 512;
        if end > self.data.len() {
            return Err(());
        }
        self.data[start..end].copy_from_slice(buf);
        Ok(())
    }

    fn raw_data_mut(&mut self) -> &mut [u8] {
        &mut self.data[..]
    }
}

impl fmt::Display for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cluster = ((self.first_cluster_high as u32) << 16) | (self.first_cluster_low as u32);

        write!(
            f,"DirEntry {{
                Name: \"{}\", FC: {}
                }}", self.get_name().unwrap(), cluster
        )
    }
}

impl fmt::Display for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let oem_str = core::str::from_utf8(&self._oem)
            .unwrap_or("<invalid utf8>");

        write!(
            f,
            "BiosParameterBlock {{
JMP: {:02X?}
OEM: {}
Bytes per sector: {}
Sectors per cluster: {}
Reserved sectors: {}
FAT table count: {}
Root entries: {}
Total sectors 16: {}
Media descriptor: 0x{:02X}
FAT size 16: {}
Sectors per track: {}
Heads on media: {}
Hidden sectors: {}
Total sectors 32: {}
}}",
            &self._jmp,
            oem_str,
            self.bytes_per_sector,
            self.sectors_per_cluster,
            self.reserved_sectors,
            self.fat_table_count,
            self.root_entries,
            self.total_sectors_16,
            self.media_descriptor,
            self.fat_size_16,
            self.sectors_per_track,
            self.heads_on_media,
            self.hidden_sectors,
            self.total_sectors_32,
        )
    }
}

impl fmt::Display for ExtendedBootRecord32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let volume_label_str = core::str::from_utf8(&self.volume_label)
            .unwrap_or("<invalid utf8>");
        let fs_id_str = core::str::from_utf8(&self.fs_id)
            .unwrap_or("<invalid utf8>");

        write!(
            f,
            "ExtendedBootRecord32 {{
FAT size 32: {}
Ext flags: {}
FAT version: {}
Root cluster: {}
FS info sector: {}
Backup boot sector: {}
Drive number: {}
Signature: 0x{:02X}
Volume ID: 0x{:08X}
Volume Label: {}
FS ID: {}
}}",
            self.fat_size_32,
            self.ext_flags,
            self.fat_version,
            self.root_cluster,
            self.fs_info,
            self.backup_boot,
            self.drive_number,
            self.signature,
            self.volume_id,
            volume_label_str,
            fs_id_str,
        )
    }
}
