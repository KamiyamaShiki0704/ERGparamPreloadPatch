"""Read-only WW 2.7.1.0 checks; requires pefile and capstone. Pass EXE path."""
import hashlib
import struct
import sys

import capstone
import pefile

pe = pefile.PE(sys.argv[1])
info = pe.VS_FIXEDFILEINFO[0]
version = (info.FileVersionMS >> 16, info.FileVersionMS & 65535,
           info.FileVersionLS >> 16, info.FileVersionLS & 65535)
assert version == (2, 7, 1, 0), version
print('Version:', version)
print('SHA256:', hashlib.sha256(pe.__data__).hexdigest())


def expect(rva, hex_bytes):
    expected = bytes.fromhex(hex_bytes)
    assert pe.get_data(rva, len(expected)) == expected, hex(rva)


def relative(rva, length, target):
    displacement = struct.unpack('<i', pe.get_data(rva + length - 4, 4))[0]
    assert rva + length + displacement == target, hex(rva)


# CommonEvent type-8 loop, resource-manager load and request call.
expect(0x613D07, '41 b8 08 00 00 00')
expect(0x613D2A, '48 8b 0d')
relative(0x613D2A, 7, 0x3D5F158)
expect(0x613D62, 'e8')
relative(0x613D62, 5, 0x1F2420)
expect(0x613D91, '83 fb 05')
expect(0x1F2420, '40 53 55 56 57 41 56')
# Prime function formats type 8 and is called by the 0..4 prime loop.
expect(0xAB9CE8, '41 b8 08 00 00 00')
expect(0xAB96CB, 'e8')
relative(0xAB96CB, 5, 0xAB9CB0)
expect(0xAB96D2, '83 ff 05')
# CSWindow getter/setter both reference the HINSTANCE slot.
expect(0xDEC0C0, '48 8b 05')
relative(0xDEC0C0, 7, 0x3D89708)
expect(0xDEC6B0, '48 89 0d')
relative(0xDEC6B0, 7, 0x3D89708)
# Native task caller passes task in r8 and group in edx to the new entry.
expect(0xEB7466, '4c 8b c7 8b d6 e8')
relative(0xEB746B, 5, 0xEB3E50)
expect(0xEB3E50, '48 89 5c 24 08 57 48 83 ec 40')
expect(0xEB3DE0, '83 fa 06')

cs = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
for rva, size in [(0x613D00, 0x9A), (0xAB9CB0, 0x50),
                  (0xDEC0C0, 8), (0xDEC6B0, 8),
                  (0xEB7466, 10), (0xEB3DE0, 16), (0xEB3E50, 0x65)]:
    print('\nRVA', hex(rva))
    for ins in cs.disasm(pe.get_data(rva, size), rva):
        print(f'{ins.address:08X} {ins.mnemonic} {ins.op_str}')
print('\nPASS: five targets and supporting call/global relationships verified.')
