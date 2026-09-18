macro_rules! linker_section {
    ($struct_name:ident, $__section_start:ident, $__section_end:ident) => {
        pub struct $struct_name;

        impl $struct_name {
            /// The starting address of this section.
            pub fn start() -> usize {
                unsafe extern "C" {
                    unsafe static $__section_start: u64;
                }

                // `addr_of!` avoids creating a reference to the linker symbol,
                // which would be unsound if the symbol is misaligned or zero-sized.
                core::ptr::addr_of!($__section_start) as usize
            }

            /// The ending address of this section (exclusive).
            pub fn end() -> usize {
                unsafe extern "C" {
                    unsafe static $__section_end: u64;
                }

                core::ptr::addr_of!($__section_end) as usize
            }

            /// Grab a slice to this section.
            pub fn as_ref() -> &'static [u8] {
                unsafe {
                    core::slice::from_raw_parts(Self::start() as *const u8, Self::size())
                }
            }

            /// Grab a mutable slice to this section.
            pub fn as_mut() -> &'static mut [u8] {
                unsafe {
                    core::slice::from_raw_parts_mut(Self::start() as *mut u8, Self::size())
                }
            }

            /// The size of this section in bytes.
            pub fn size() -> usize {
                Self::end().saturating_sub(Self::start())
            }

            /// Returns `true` if `addr` falls within `[start, end)`.
            pub fn contains(addr: usize) -> bool {
                addr >= Self::start() && addr < Self::end()
            }
        }

        impl core::fmt::Debug for $struct_name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "[\x1b[92m0x{:08x}\x1b[0m..\x1b[95m0x{:08x}\x1b[0m]: \x1b[93m{:<10}\x1b[0m (\x1b[94m{:>6}\x1b[0m bytes)", $struct_name::start(), $struct_name::end(), stringify!($struct_name), $struct_name::size())
            }
        }
    };
}

linker_section!(Text, __text_start, __text_end);
linker_section!(Rodata, __rodata_start, __rodata_end);
linker_section!(Data, __data_start, __data_end);
linker_section!(Bss, __bss_start, __bss_end);
linker_section!(Trampoline, __trampoline_start, __trampoline_end);
linker_section!(Kernel, __kernel_start, __kernel_end);

/// For debugging purposes, print out the memory regions.
pub fn print_elf_sections() {
    crate::debugln!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        Kernel,
        Text,
        Trampoline,
        Rodata,
        Data,
        Bss
    );
}
