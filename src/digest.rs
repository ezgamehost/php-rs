// Altered Rust adaptation of PHP 8.5.0 ext/standard/{md5,sha1,crc32}.c.
// Copyright (c) The PHP Group. The MD5 compression core follows Solar
// Designer's 2001 public-domain implementation. See THIRD_PARTY_NOTICES.md.

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = crc32_table();
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc = (crc >> 8) ^ TABLE[((crc ^ u32::from(byte)) & 0xff) as usize];
    }
    !crc
}

const fn crc32_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ if value & 1 != 0 { 0xedb88320 } else { 0 };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Algorithm {
    Md5,
    Sha1,
}

pub(crate) struct Digest {
    algorithm: Algorithm,
    state: [u32; 5],
    length: u64,
    buffer: [u8; 64],
    used: usize,
}

impl Digest {
    pub(crate) fn new(algorithm: Algorithm) -> Self {
        Self {
            algorithm,
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            length: 0,
            buffer: [0; 64],
            used: 0,
        }
    }

    pub(crate) fn update(&mut self, mut bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        if self.used != 0 {
            let take = bytes.len().min(64 - self.used);
            self.buffer[self.used..self.used + take].copy_from_slice(&bytes[..take]);
            self.used += take;
            bytes = &bytes[take..];
            if self.used != 64 {
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.used = 0;
        }
        while bytes.len() >= 64 {
            self.compress(bytes[..64].try_into().unwrap());
            bytes = &bytes[64..];
        }
        self.buffer[..bytes.len()].copy_from_slice(bytes);
        self.used = bytes.len();
    }

    pub(crate) fn finish(mut self) -> Vec<u8> {
        let bits = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        let padding = (56 + 64 - self.used) % 64;
        self.update(&[0; 64][..padding]);
        let length = match self.algorithm {
            Algorithm::Md5 => bits.to_le_bytes(),
            Algorithm::Sha1 => bits.to_be_bytes(),
        };
        self.update(&length);
        let words = if self.algorithm == Algorithm::Md5 {
            4
        } else {
            5
        };
        self.state[..words]
            .iter()
            .flat_map(|word| match self.algorithm {
                Algorithm::Md5 => word.to_le_bytes(),
                Algorithm::Sha1 => word.to_be_bytes(),
            })
            .collect()
    }

    fn compress(&mut self, block: &[u8; 64]) {
        match self.algorithm {
            Algorithm::Md5 => md5(&mut self.state, block),
            Algorithm::Sha1 => sha1(&mut self.state, block),
        }
    }
}

fn md5(state: &mut [u32; 5], block: &[u8; 64]) {
    const SHIFTS: [[u32; 4]; 4] = [
        [7, 12, 17, 22],
        [5, 9, 14, 20],
        [4, 11, 16, 23],
        [6, 10, 15, 21],
    ];
    let words: [u32; 16] = std::array::from_fn(|index| {
        u32::from_le_bytes(block[index * 4..index * 4 + 4].try_into().unwrap())
    });
    let [mut a, mut b, mut c, mut d, _] = *state;
    for (index, constant) in MD5_CONSTANTS.into_iter().enumerate() {
        let (function, word) = match index / 16 {
            0 => ((b & c) | (!b & d), index),
            1 => ((d & b) | (!d & c), (5 * index + 1) % 16),
            2 => (b ^ c ^ d, (3 * index + 5) % 16),
            _ => (c ^ (b | !d), (7 * index) % 16),
        };
        let next = b.wrapping_add(
            a.wrapping_add(function)
                .wrapping_add(constant)
                .wrapping_add(words[word])
                .rotate_left(SHIFTS[index / 16][index % 4]),
        );
        (a, b, c, d) = (d, next, b, c);
    }
    for (state, value) in state.iter_mut().zip([a, b, c, d]) {
        *state = state.wrapping_add(value);
    }
}

fn sha1(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut words = [0_u32; 80];
    for (word, bytes) in words.iter_mut().zip(block.as_chunks::<4>().0) {
        *word = u32::from_be_bytes(*bytes);
    }
    for index in 16..80 {
        words[index] =
            (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                .rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (index, word) in words.into_iter().enumerate() {
        let (function, constant) = match index / 20 {
            0 => ((b & c) | (!b & d), 0x5a827999),
            1 => (b ^ c ^ d, 0x6ed9eba1),
            2 => ((b & c) | (d & (b | c)), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let next = a
            .rotate_left(5)
            .wrapping_add(function)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(word);
        (a, b, c, d, e) = (next, a, b.rotate_left(30), c, d);
    }
    for (state, value) in state.iter_mut().zip([a, b, c, d, e]) {
        *state = state.wrapping_add(value);
    }
}

const MD5_CONSTANTS: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];
