use lazy_static::lazy_static;
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;
use core::{fmt};

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
}

impl<'a, D: BlockDevice> FileSystem<'a, D> {
    pub fn new(device: &'a mut D) -> Result<Self, ()> {     

        let mut sector = [0u8; 512];
        device.read_sector(0, &mut sector)?;

        let bpb = BiosParameterBlock::from_bytes(&sector[0..36])?;

        let ebr = ExtendedBootRecord32::from_bytes(&sector[36..90])?;


        let fat_start = bpb.reserved_sectors as u32;
        let cluster_heap_start = fat_start + (ebr.fat_size_32 * bpb.fat_table_count as u32);
        let root_dir_cluster = ebr.root_clusters;

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
        let mut disk = RamDisk::new(16);
        disk.init_fat32_boot_sector(16);
        Mutex::new(disk)
    };
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
    pub root_clusters: u32,
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

impl RamDisk {
    pub fn init_fat32_boot_sector(&mut self, size_in_sectors: usize) {
        self.data.resize(size_in_sectors * 512, 0);

        let data = &mut self.data;

        data[0] = 0xEB; // JMP short
        data[1] = 0x58; // JMP offset
        data[2] = 0x90; // NOP

        // OEM Name (8 bytes)
        let oem = b"MSWIN4.1";
        data[3..11].copy_from_slice(oem);

        // Bytes per sector (2 bytes) = 512
        data[11..13].copy_from_slice(&512u16.to_le_bytes());

        // Sectors per cluster (1 byte) = 8
        data[13] = 8;

        // Reserved sectors (2 bytes) = 32
        data[14..16].copy_from_slice(&32u16.to_le_bytes());

        // Number of FATs (1 byte) = 2
        data[16] = 2;

        // Root entries (2 bytes) = 0
        data[17..19].copy_from_slice(&0u16.to_le_bytes());

        // Total sectors 16 (2 bytes) = 0 (use 32-bit total sectors)
        data[19..21].copy_from_slice(&0u16.to_le_bytes());

        // Media descriptor (1 byte)
        data[21] = 0xF8;

        // FAT size 16 (2 bytes) = 0 (use 32-bit fat size)
        data[22..24].copy_from_slice(&0u16.to_le_bytes());

        // Sectors per track (2 bytes) = 63
        data[24..26].copy_from_slice(&63u16.to_le_bytes());

        // Number of heads (2 bytes) = 255
        data[26..28].copy_from_slice(&255u16.to_le_bytes());

        // Hidden sectors (4 bytes) = 0
        data[28..32].copy_from_slice(&0u32.to_le_bytes());

        // Total sectors 32 (4 bytes)
        data[32..36].copy_from_slice(&(size_in_sectors as u32).to_le_bytes());

        // FAT size 32 (4 bytes) = 100 sectors per FAT (example)
        data[36..40].copy_from_slice(&100u32.to_le_bytes());

        // Extended Boot Record starts at offset 0x24 (36 decimal), so continue filling:
        // Ext flags (2 bytes) = 0
        data[40..42].copy_from_slice(&0u16.to_le_bytes());

        // FAT version (2 bytes) = 0
        data[42..44].copy_from_slice(&0u16.to_le_bytes());

        // Root cluster (4 bytes) = 2
        data[44..48].copy_from_slice(&2u32.to_le_bytes());

        // FS info sector (2 bytes) = 1
        data[48..50].copy_from_slice(&1u16.to_le_bytes());

        // Backup boot sector (2 bytes) = 6
        data[50..52].copy_from_slice(&6u16.to_le_bytes());

        // Drive number (1 byte)
        data[64] = 0x80;

        // Signature (1 byte)
        data[66] = 0x29;

        // Volume ID (4 bytes)
        data[67..71].copy_from_slice(&0x12345678u32.to_le_bytes());

        // Volume Label (11 bytes)
        let label = b"NO NAME    ";
        data[71..82].copy_from_slice(label);

        // File system type (8 bytes)
        let fs_type = b"FAT32   ";
        data[82..90].copy_from_slice(fs_type);

        // Boot signature (2 bytes) at offset 510-511
        data[510] = 0x55;
        data[511] = 0xAA;
    }
    pub fn new(size_in_sectors: usize) -> Self {
        RamDisk {
            data: vec![0; size_in_sectors * 512],
        }
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
            root_clusters: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
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
}

impl fmt::Display for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Convert OEM name bytes to string safely
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
            self.root_clusters,
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
