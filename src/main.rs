use anyhow::{anyhow, Result};
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Seek, SeekFrom},
    path::Path,
};

#[derive(Debug)]
struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

trait Image {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn pixel(&self, x: u32, y: u32) -> &Pixel;
    fn pixel_mut(&mut self, x: u32, y: u32) -> &mut Pixel;
}

#[derive(Debug)]
struct BMP {
    magic: u16,
    size: u32,
    offset: u32,
    hdr_size: u32,
    width: u32,
    height: u32,
    num_planes: u16,
    bpp: u16,
    compression: u32,
    image_size: u32,
    h_ppm: i32,
    v_ppm: i32,
    num_colors: u32,
    used_colors: u32,

    pixels: Vec<Pixel>,
}

impl BMP {
    fn read(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().read(true).open(path)?;
        let mut rd = BufReader::new(file);

        let magic = rd.read_u16::<LE>()?;
        let size = rd.read_u32::<LE>()?;
        let _ = rd.read_u32::<LE>()?;
        let offset = rd.read_u32::<LE>()?;
        let hdr_size = rd.read_u32::<LE>()?;
        let width = rd.read_u32::<LE>()?;
        let height = rd.read_u32::<LE>()?;
        let num_planes = rd.read_u16::<LE>()?;
        let bpp = rd.read_u16::<LE>()?;
        let compression = rd.read_u32::<LE>()?;
        let image_size = rd.read_u32::<LE>()?;
        let h_ppm = rd.read_i32::<LE>()?;
        let v_ppm = rd.read_i32::<LE>()?;
        let num_colors = rd.read_u32::<LE>()?;
        let used_colors = rd.read_u32::<LE>()?;

        let row_size = {
            let row_bytes = (bpp as u32 / 8) * width;
            4 * ((row_bytes / 4) + if row_bytes % 4 != 0 { 1 } else { 0 })
        };

        let mut pixels: Vec<Pixel> = Vec::new();
        for y in (0..height as u32).rev() {
            rd.seek(SeekFrom::Start((offset + y as u32 * row_size) as u64))?;
            for _ in 0..width {
                let b = rd.read_u8()?;
                let g = rd.read_u8()?;
                let r = rd.read_u8()?;
                pixels.push(Pixel { r, g, b });
            }
        }

        Ok(BMP {
            magic,
            size,
            offset,
            hdr_size,
            width,
            height,
            num_planes,
            num_colors,
            bpp,
            compression,
            image_size,
            h_ppm,
            v_ppm,
            used_colors,
            pixels,
        })
    }

    fn write(&self, path: &Path) -> Result<()> {
        let file = OpenOptions::new().write(true).create(true).open(path)?;
        let mut wd = BufWriter::new(file);

        wd.write_u16::<LE>(self.magic)?;
        wd.write_u32::<LE>(self.size)?;
        wd.write_u32::<LE>(0)?;
        wd.write_u32::<LE>(self.offset)?;
        wd.write_u32::<LE>(self.hdr_size)?;
        wd.write_u32::<LE>(self.width)?;
        wd.write_u32::<LE>(self.height)?;
        wd.write_u16::<LE>(self.num_planes)?;
        wd.write_u16::<LE>(self.bpp)?;
        wd.write_u32::<LE>(self.compression)?;
        wd.write_u32::<LE>(self.image_size)?;
        wd.write_i32::<LE>(self.h_ppm)?;
        wd.write_i32::<LE>(self.v_ppm)?;
        wd.write_u32::<LE>(self.num_colors)?;
        wd.write_u32::<LE>(self.used_colors)?;

        let pad = {
            let row_bytes = (self.bpp as u32 / 8) * self.width;
            let row_size = 4 * ((row_bytes / 4) + if row_bytes % 4 != 0 { 1 } else { 0 });

            row_size - row_bytes
        };

        for y in (0..self.height).rev() {
            for x in 0..self.width {
                let pixel = self.pixel(x, y);
                wd.write_u8(pixel.b)?;
                wd.write_u8(pixel.g)?;
                wd.write_u8(pixel.r)?;
            }

            for _ in 0..pad {
                wd.write_u8(0)?;
            }
        }

        Ok(())
    }
}

impl Image for BMP {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn pixel(&self, x: u32, y: u32) -> &Pixel {
        &self.pixels[(x + y * self.width) as usize]
    }

    fn pixel_mut(&mut self, x: u32, y: u32) -> &mut Pixel {
        &mut self.pixels[(x + y * self.width) as usize]
    }
}

struct FileBitReader {
    pub size: u64,
    rd: BufReader<File>,
    bit_position: u64,
}

impl FileBitReader {
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().read(true).open(path)?;
        let size = file.metadata()?.len();

        let rd = BufReader::new(file);
        let bit_position: u64 = 0;

        Ok(Self {
            rd,
            size,
            bit_position,
        })
    }

    pub fn read_bit(&mut self) -> Result<u8> {
        self.rd.seek(SeekFrom::Start(self.bit_position / 8))?;
        let bit = {
            let byte = self.rd.read_u8()?;

            (byte >> (self.bit_position % 8)) & 1
        };

        self.bit_position += 1;
        Ok(bit)
    }

    pub fn read_bits(&mut self, len: u8) -> Result<u8> {
        assert!(len > 0 && len <= 8);

        let mut out: u8 = 0;
        for i in 0..len {
            out |= self.read_bit()? << i;
        }
        Ok(out)
    }
}

struct FileBitWriter {
    wd: BufWriter<File>,
    bit_position: u64,
    byte: Option<u8>,
}

impl FileBitWriter {
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let wd = BufWriter::new(file);
        let bit_position: u64 = 0;
        let byte = None;

        Ok(Self {
            wd,
            bit_position,
            byte,
        })
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(byte) = self.byte {
            self.wd.write_u8(byte)?;
            self.byte = None;
        }
        Ok(())
    }

    pub fn write_bit(&mut self, bit: u8) -> Result<()> {
        self.byte = if let Some(byte) = self.byte {
            let value = byte | (bit << (self.bit_position % 8));
            Some(value)
        } else {
            Some(bit)
        };

        self.bit_position += 1;
        if self.bit_position != 0 && self.bit_position % 8 == 0 {
            self.flush()?;
        }

        Ok(())
    }

    pub fn write_bits(&mut self, mut bits: u8, len: u8) -> Result<()> {
        assert!(len > 0 && len <= 8);

        for i in 0..len {
            self.write_bit(bits & 1)?;
            bits >>= 1;
        }
        Ok(())
    }
}

impl Drop for FileBitWriter {
    fn drop(&mut self) {
        self.flush().expect("flush before drop");
    }
}

struct ImageDataStream<T: Image> {
    image: T,
}

impl<T: Image> ImageDataStream<T> {
    pub fn new(image: T) -> Self {
        Self { image }
    }

    fn pixel(&self, addr: u32) -> &Pixel {
        self.image
            .pixel(addr % self.image.width(), addr / self.image.height())
    }

    fn pixel_mut(&mut self, addr: u32) -> &mut Pixel {
        self.image
            .pixel_mut(addr % self.image.width(), addr / self.image.height())
    }

    const R_BITS: u8 = 3;
    const G_BITS: u8 = 2;
    const B_BITS: u8 = 2;

    const R_MASK: u8 = (1 << Self::R_BITS) - 1;
    const G_MASK: u8 = (1 << Self::G_BITS) - 1;
    const B_MASK: u8 = (1 << Self::B_BITS) - 1;

    const B_POS: u8 = 0;
    const G_POS: u8 = Self::B_BITS;
    const R_POS: u8 = Self::G_BITS + Self::B_BITS;

    const WORD_SIZE: u8 = Self::R_BITS + Self::G_BITS + Self::B_BITS;
    const WORD_MASK: u8 = (1 << Self::WORD_SIZE) - 1;

    pub fn read_word(&self, addr: u32) -> u8 {
        let pixel = self.pixel(addr);

        (pixel.r & Self::R_MASK) << Self::R_POS
            | (pixel.g & Self::G_MASK) << Self::G_POS
            | (pixel.b & Self::B_MASK) << Self::B_POS
    }

    pub fn write_word(&mut self, addr: u32, value: u8) {
        let pixel = self.pixel_mut(addr);

        pixel.r = (pixel.r & !Self::R_MASK) | ((value >> Self::R_POS) & Self::R_MASK);
        pixel.g = (pixel.g & !Self::G_MASK) | ((value >> Self::G_POS) & Self::G_MASK);
        pixel.b = (pixel.b & !Self::B_MASK) | (value & Self::B_MASK);
    }

    const HEADER_SIZE: u8 = 63;
    const HEADER_WORDS: u8 = Self::HEADER_SIZE / Self::WORD_SIZE;

    fn read_header(&self) -> u64 {
        let mut header: u64 = 0;
        for i in 0..Self::HEADER_WORDS {
            header |= (self.read_word(i as u32) as u64) << (i * Self::WORD_SIZE);
        }

        header
    }

    fn write_header(&mut self, header: u64) {
        for i in 0..Self::HEADER_WORDS {
            let value = (header >> (i * Self::WORD_SIZE)) as u8 & Self::WORD_MASK;
            self.write_word(i as u32, value);
        }
    }

    const DATA_START: u64 = Self::HEADER_WORDS as u64;
    pub fn read_stream(&self, output: &mut FileBitWriter) -> Result<()> {
        let bytes = self.read_header();
        let bits = bytes * 8;
        let count = bits / Self::WORD_SIZE as u64;
        let rem = bits % Self::WORD_SIZE as u64;

        for i in Self::DATA_START..Self::DATA_START + count {
            output.write_bits(self.read_word(i as u32), Self::WORD_SIZE)?;
        }
        if rem != 0 {
            let value = self.read_word((Self::DATA_START + count) as u32);
            output.write_bits(value, rem as u8)?; // & !((1<<rem)-1);
        }

        Ok(())
    }

    pub fn write_stream(&mut self, input: &mut FileBitReader) -> Result<()> {
        let bytes = input.size;
        let bits = bytes * 8;
        let count = bits / Self::WORD_SIZE as u64;
        let rem = bits % Self::WORD_SIZE as u64;
        
        self.write_header(bytes);

        for i in Self::DATA_START..Self::DATA_START + count {
            self.write_word(i as u32, input.read_bits(Self::WORD_SIZE)?);
        }
        if rem != 0 {
            let value = input.read_bits(rem as u8)?;
            self.write_word((Self::DATA_START + count) as u32, value);
        }

        Ok(())
    }

    pub fn into_inner(self) -> T {
        self.image
    }
}

fn main() {
    let bmp = BMP::read(Path::new("blank.bmp")).expect("read");
    let mut test = ImageDataStream::new(bmp);

    let mut input = FileBitReader::open(Path::new("input.jpg")).expect("open");
    test.write_stream(&mut input).expect("write_stream");

    let mut output = FileBitWriter::open(Path::new("output.jpg")).expect("open");
    test.read_stream(&mut output).expect("read_stream");

    let out = test.into_inner();
    out.write(Path::new("test2.bmp")).expect("write");
}
