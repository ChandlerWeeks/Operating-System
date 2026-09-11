#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct Source {
        raw: String,
        tokens: Vec<String>,
        compact: String,
    }

    impl Source {
        fn read(relative: &str) -> Self {
            let path = repo_root().join(relative);
            let raw = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let tokens = lex(&raw);
            let compact = tokens.concat();
            Self {
                raw,
                tokens,
                compact,
            }
        }
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("lab2-tests must be inside the MyOS repository")
            .to_path_buf()
    }

    fn lex(input: &str) -> Vec<String> {
        let bytes = input.as_bytes();
        let mut tokens = Vec::new();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }

            if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }

            if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                i += 2;
                let mut depth = 1usize;
                while i < bytes.len() && depth != 0 {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                continue;
            }

            if bytes[i] == b'"' {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i = (i + 2).min(bytes.len());
                    } else if bytes[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                tokens.push(input[start..i].to_string());
                continue;
            }

            let is_character = bytes[i] == b'\''
                && (bytes.get(i + 2) == Some(&b'\'')
                    || (bytes.get(i + 1) == Some(&b'\\') && bytes.get(i + 3) == Some(&b'\'')));
            if is_character {
                let width = if bytes.get(i + 1) == Some(&b'\\') {
                    4
                } else {
                    3
                };
                tokens.push(input[i..i + width].to_string());
                i += width;
                continue;
            }

            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(input[start..i].to_string());
                continue;
            }

            if bytes[i].is_ascii_digit() {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(input[start..i].to_string());
                continue;
            }

            tokens.push((bytes[i] as char).to_string());
            i += 1;
        }

        tokens
    }

    fn parse_integer(token: &str) -> Option<u64> {
        let token = token.replace('_', "");
        if let Some(hex) = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
        {
            let digits: String = hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            return (!digits.is_empty())
                .then(|| u64::from_str_radix(&digits, 16).expect("valid hexadecimal literal"));
        }
        let digits: String = token.chars().take_while(|c| c.is_ascii_digit()).collect();
        (!digits.is_empty()).then(|| digits.parse().expect("valid decimal literal"))
    }

    fn initializer<'a>(tokens: &'a [String], kind: &str, name: &str) -> &'a [String] {
        let start = tokens
            .windows(2)
            .position(|pair| pair[0] == kind && pair[1] == name)
            .unwrap_or_else(|| panic!("missing `{kind} {name}`"));
        let equals = tokens[start..]
            .iter()
            .position(|token| token == "=")
            .map(|offset| start + offset)
            .unwrap_or_else(|| panic!("`{kind} {name}` has no initializer"));

        let mut square = 0usize;
        let mut round = 0usize;
        let mut curly = 0usize;
        for end in equals + 1..tokens.len() {
            match tokens[end].as_str() {
                "[" => square += 1,
                "]" => square = square.saturating_sub(1),
                "(" => round += 1,
                ")" => round = round.saturating_sub(1),
                "{" => curly += 1,
                "}" => curly = curly.saturating_sub(1),
                ";" if square == 0 && round == 0 && curly == 0 => {
                    return &tokens[equals + 1..end];
                }
                _ => {}
            }
        }
        panic!("unterminated initializer for `{kind} {name}`")
    }

    fn assert_public_usize_const(source: &Source, name: &str, expected: u64) {
        let position = source
            .tokens
            .windows(3)
            .position(|window| window == ["pub", "const", name])
            .unwrap_or_else(|| panic!("{name} must be a public constant"));
        let equals = source.tokens[position..]
            .iter()
            .position(|token| token == "=")
            .map(|offset| position + offset)
            .expect("constant must have an initializer");
        let colon = source.tokens[position..equals]
            .iter()
            .position(|token| token == ":")
            .map(|offset| position + offset)
            .expect("constant must have an explicit type");
        assert_eq!(
            source.tokens[colon + 1..equals].concat(),
            "usize",
            "{name} must have type usize"
        );
        let value = initializer(&source.tokens, "const", name)
            .iter()
            .find_map(|token| parse_integer(token))
            .unwrap_or_else(|| panic!("{name} must have a numeric initializer"));
        assert_eq!(value, expected, "{name} has the wrong value");
    }

    fn assert_const_array(source: &Source, name: &str, expected: &[u64]) {
        let actual: Vec<u64> = initializer(&source.tokens, "const", name)
            .iter()
            .filter_map(|token| parse_integer(token))
            .collect();
        assert_eq!(actual, expected, "{name} has the wrong Limine magic values");
    }

    fn block_after<'a>(tokens: &'a [String], item: usize) -> &'a [String] {
        let open = tokens[item..]
            .iter()
            .position(|token| token == "{")
            .map(|offset| item + offset)
            .expect("item must have a body");
        let mut depth = 0usize;
        for end in open..tokens.len() {
            match tokens[end].as_str() {
                "{" => depth += 1,
                "}" => {
                    depth -= 1;
                    if depth == 0 {
                        return &tokens[open + 1..end];
                    }
                }
                _ => {}
            }
        }
        panic!("item body is not closed")
    }

    fn named_blocks<'a>(source: &'a Source, kind: &str, name: &str) -> Vec<&'a [String]> {
        source
            .tokens
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| pair[0] == kind && pair[1] == name)
            .map(|(position, _)| block_after(&source.tokens, position))
            .collect()
    }

    fn function_body<'a>(source: &'a Source, name: &str) -> &'a [String] {
        named_blocks(source, "fn", name)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("missing function `{name}`"))
    }

    fn function_body_with<'a>(source: &'a Source, name: &str, marker: &str) -> &'a [String] {
        named_blocks(source, "fn", name)
            .into_iter()
            .find(|body| body.concat().contains(marker))
            .unwrap_or_else(|| panic!("missing `{name}` implementation for {marker}"))
    }

    fn assert_body_number(body: &[String], expected: u64, context: &str) {
        assert!(
            body.iter()
                .filter_map(|token| parse_integer(token))
                .any(|value| value == expected),
            "{context} must use {expected:#x}"
        );
    }

    fn assert_order(body: &[String], names: &[&str], context: &str) {
        let mut cursor = 0usize;
        for name in names {
            let next = body[cursor..]
                .iter()
                .position(|token| token == name)
                .map(|offset| cursor + offset)
                .unwrap_or_else(|| panic!("{context} must use `{name}`"));
            cursor = next + 1;
        }
    }

    fn struct_fields(source: &Source, name: &str) -> Vec<(String, String)> {
        let body = named_blocks(source, "struct", name)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("missing struct `{name}`"));
        let mut fields = Vec::new();
        let mut start = 0usize;
        let mut square = 0usize;
        let mut round = 0usize;
        let mut angle = 0usize;

        for end in 0..=body.len() {
            let token = body.get(end).map(String::as_str).unwrap_or(",");
            match token {
                "[" => square += 1,
                "]" => square = square.saturating_sub(1),
                "(" => round += 1,
                ")" => round = round.saturating_sub(1),
                "<" => angle += 1,
                ">" => angle = angle.saturating_sub(1),
                "," if square == 0 && round == 0 && angle == 0 => {
                    let field = &body[start..end];
                    if let Some(colon) = field.iter().position(|part| part == ":") {
                        let field_name = field[..colon]
                            .iter()
                            .rev()
                            .find(|part| part.as_str() != "pub")
                            .expect("field must have a name")
                            .clone();
                        fields.push((field_name, field[colon + 1..].concat()));
                    }
                    start = end + 1;
                }
                _ => {}
            }
        }
        fields
    }

    fn assert_fields(source: &Source, name: &str, expected: &[(&str, &str)]) {
        let actual = struct_fields(source, name);
        let expected: Vec<(String, String)> = expected
            .iter()
            .map(|(field, ty)| ((*field).to_string(), (*ty).to_string()))
            .collect();
        assert_eq!(actual, expected, "{name} has the wrong field layout");
    }

    fn assert_repr(source: &Source, name: &str, packed: bool) {
        let position = source
            .tokens
            .windows(2)
            .position(|pair| pair[0] == "struct" && pair[1] == name)
            .unwrap_or_else(|| panic!("missing struct `{name}`"));
        let start = position.saturating_sub(30);
        let attributes = source.tokens[start..position].concat();
        assert!(attributes.contains("repr(C"), "{name} must use #[repr(C)]");
        if packed {
            assert!(
                attributes
                    .rsplit("repr(")
                    .next()
                    .unwrap_or("")
                    .contains("packed"),
                "{name} must use a packed C representation"
            );
        }
    }

    fn assert_request(source: &Source, static_name: &str, request_id: &str) {
        let position = source
            .tokens
            .windows(2)
            .position(|pair| pair[0] == "static" && pair[1] == static_name)
            .unwrap_or_else(|| panic!("missing Limine request `{static_name}`"));
        let start = position.saturating_sub(30);
        let attributes = source.tokens[start..position].concat();
        assert!(
            attributes.contains("link_section=\".limine_requests\""),
            "{static_name} must be in .limine_requests"
        );
        assert!(
            attributes.contains("used"),
            "{static_name} must use #[used] so the linker retains it"
        );
        let init = initializer(&source.tokens, "static", static_name).concat();
        assert!(
            init.contains(&format!("id:make_id({request_id})")),
            "{static_name} must use {request_id}"
        );
        assert!(
            init.contains("response:AtomicPtr::new(core::ptr::null_mut())"),
            "{static_name} must start with a null atomic response pointer"
        );
    }

    fn debug_output_text(source: &Source) -> String {
        let mut output = String::new();
        let mut i = 0usize;
        while i + 2 < source.tokens.len() {
            if source.tokens[i] == "debugln" && source.tokens[i + 1] == "!" {
                let open = &source.tokens[i + 2];
                let close = match open.as_str() {
                    "(" => ")",
                    "[" => "]",
                    "{" => "}",
                    _ => {
                        i += 1;
                        continue;
                    }
                };
                let mut depth = 1usize;
                i += 3;
                while i < source.tokens.len() && depth != 0 {
                    if source.tokens[i] == *open {
                        depth += 1;
                    } else if source.tokens[i] == close {
                        depth -= 1;
                    }
                    if depth != 0 {
                        output.push_str(&source.tokens[i]);
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        output.to_ascii_lowercase()
    }

    #[test]
    fn required_modules_exist_and_are_enabled() {
        for file in [
            "src/config.rs",
            "src/limine.rs",
            "src/acpi.rs",
            "src/main.rs",
        ] {
            assert!(
                repo_root().join(file).is_file(),
                "missing required file `{file}`"
            );
        }
        let main = Source::read("src/main.rs");
        for module in ["config", "limine", "acpi"] {
            assert!(
                main.compact.contains(&format!("pubmod{module};")),
                "main.rs must declare `pub mod {module};`"
            );
        }
    }

    #[test]
    fn configuration_constants_match_the_lab() {
        let config = Source::read("src/config.rs");
        assert_public_usize_const(&config, "MAX_HARTS", 16);
        assert_public_usize_const(&config, "KERNEL_IO_ADDR", 0xffff_beef_0000_0000);
        assert_public_usize_const(&config, "KERNEL_HEAP_ADDR", 0xffff_cafe_0000_0000);
        assert_public_usize_const(&config, "NUM_HEAP_PAGES", 1024);
    }

    #[test]
    fn qemu_limine_and_linker_configuration_support_lab_2() {
        let options = Source::read("OPTIONS.sh");
        let lower = options.raw.to_ascii_lowercase();
        for required in ["virt", "aia=aplic-imsic", "acpi=on", "sv48=on"] {
            assert!(
                lower.contains(required),
                "OPTIONS.sh must contain `{required}`"
            );
        }

        let limine_config = Source::read("uefi/esp/boot/limine/limine.conf");
        assert!(
            limine_config.raw.contains("kernel_cmdline:"),
            "limine.conf must provide a kernel command line for CMD_LINE_ID"
        );

        let linker = Source::read("riscv64gc-virt.ld");
        for section in [
            ".limine_requests_start",
            ".limine_requests",
            ".limine_requests_end",
        ] {
            assert!(
                linker.raw.contains(&format!("KEEP(*({section}))")),
                "linker script must retain {section}"
            );
        }
        assert!(
            linker
                .raw
                .to_ascii_uppercase()
                .contains("ORIGIN = 0XFFFFFFFFA0000000"),
            "the kernel link address must be 0xFFFF_FFFF_A000_0000"
        );
    }

    #[test]
    fn limine_protocol_magic_values_match_the_specification() {
        let limine = Source::read("src/limine.rs");
        let arrays: [(&str, &[u64]); 13] = [
            ("COMMON_MAGIC", &[0xc7b1dd30df4c8b88, 0x0a82e883a194f07b]),
            (
                "BASE_REVISION_MAGIC",
                &[0xf9562b2d5c95a6c8, 0x6a7b384944536bdc],
            ),
            (
                "REQUESTS_START_MAGIC",
                &[
                    0xf6b8f4b39de7d1ae,
                    0xfab91a6940fcb9cf,
                    0x785c6ed015d3e316,
                    0x181e920a7852b9d9,
                ],
            ),
            (
                "REQUESTS_END_MAGIC",
                &[0xadc0e0531bb10d03, 0x9572709f31764c62],
            ),
            ("HHDM_ID", &[0x48dcf1cb8ad2b852, 0x63984e959a98244b]),
            ("MEMMAP_ID", &[0x67cf3d9d378a806f, 0xe304acdfc50c3c62]),
            ("RSDP_ID", &[0xc5e77b6b397e7b43, 0x27637845accdcf3c]),
            ("EXE_ADDR_ID", &[0x71ba76863cc55f63, 0xb2644a48c516a487]),
            ("STACK_SIZE_ID", &[0x224ef0460a8e8926, 0xe1cb0fc25f46ea3d]),
            ("BSP_HARTID_ID", &[0x1369359f025525f9, 0x2ff2a56178391bb6]),
            ("PAGING_MODE_ID", &[0x95c1a0edab0944cb, 0xa4e5cb3842f7488a]),
            ("DATE_ID", &[0x502746e184c088aa, 0xfbc5ec83e6327893]),
            ("CMD_LINE_ID", &[0x4b161536e598651e, 0xb390ad4a2f1f303a]),
        ];
        for (name, expected) in arrays {
            assert_const_array(&limine, name, expected);
        }
        let make_id = function_body(&limine, "make_id").concat();
        for part in [
            "COMMON_MAGIC[0]",
            "COMMON_MAGIC[1]",
            "specific[0]",
            "specific[1]",
        ] {
            assert!(make_id.contains(part), "make_id must include `{part}`");
        }
    }

    #[test]
    fn limine_markers_and_all_required_requests_are_retained() {
        let limine = Source::read("src/limine.rs");
        for (name, section) in [
            ("REQUESTS_START", ".limine_requests_start"),
            ("REQUESTS_END", ".limine_requests_end"),
        ] {
            let position = limine
                .tokens
                .windows(2)
                .position(|pair| pair[0] == "static" && pair[1] == name)
                .unwrap_or_else(|| panic!("missing Limine marker `{name}`"));
            let attributes = limine.tokens[position.saturating_sub(30)..position].concat();
            assert!(attributes.contains("used"), "{name} must use #[used]");
            assert!(
                attributes.contains(&format!("link_section=\"{section}\"")),
                "{name} must be in {section}"
            );
        }

        for (request, id) in [
            ("HHDM_REQUEST", "HHDM_ID"),
            ("EXE_ADDRESS_REQUEST", "EXE_ADDR_ID"),
            ("MEMORY_MAP_REQUEST", "MEMMAP_ID"),
            ("RSDP_REQUEST", "RSDP_ID"),
            ("STACK_SIZE_REQUEST", "STACK_SIZE_ID"),
            ("PAGING_MODE_REQUEST", "PAGING_MODE_ID"),
            ("BSP_HARTID_REQUEST", "BSP_HARTID_ID"),
            ("CMD_LINE_REQUEST", "CMD_LINE_ID"),
            ("DATE_AT_BOOT_REQUEST", "DATE_ID"),
        ] {
            assert_request(&limine, request, id);
        }
    }

    #[test]
    fn limine_request_and_response_layouts_match_the_protocol() {
        let limine = Source::read("src/limine.rs");
        let layouts: [(&str, &[(&str, &str)]); 16] = [
            (
                "HhdmRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<HhdmResponse>"),
                ],
            ),
            ("HhdmResponse", &[("revision", "u64"), ("offset", "u64")]),
            (
                "ExecutableAddressRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<ExecutableAddressResponse>"),
                ],
            ),
            (
                "ExecutableAddressResponse",
                &[
                    ("revision", "u64"),
                    ("physical_base", "u64"),
                    ("virtual_base", "u64"),
                ],
            ),
            (
                "PagingModeRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<u8>"),
                    ("mode", "u64"),
                    ("max_mode", "u64"),
                    ("min_mode", "u64"),
                ],
            ),
            (
                "BspHartidRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<BspHartidResponse>"),
                ],
            ),
            (
                "BspHartidResponse",
                &[("revision", "u64"), ("bsp_hartid", "u64")],
            ),
            (
                "StackSizeRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<u8>"),
                    ("stack_size", "u64"),
                ],
            ),
            (
                "CmdlineRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<CmdlineResponse>"),
                ],
            ),
            (
                "CmdlineResponse",
                &[("revision", "u64"), ("cmdline", "*constu8")],
            ),
            (
                "MemoryMapRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<MemoryMapResponse>"),
                ],
            ),
            (
                "MemoryMapResponse",
                &[
                    ("revision", "u64"),
                    ("entry_count", "u64"),
                    ("entries", "*const*constMemoryMapEntry"),
                ],
            ),
            (
                "DateAtBootRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<DateAtBootResponse>"),
                ],
            ),
            (
                "DateAtBootResponse",
                &[("revision", "u64"), ("timestamp", "u64")],
            ),
            (
                "RsdpRequest",
                &[
                    ("id", "[u64;4]"),
                    ("revision", "u64"),
                    ("response", "AtomicPtr<RsdpResponse>"),
                ],
            ),
            ("RsdpResponse", &[("revision", "u64"), ("address", "u64")]),
        ];

        for (name, fields) in layouts {
            assert_repr(&limine, name, false);
            assert_fields(&limine, name, fields);
        }
    }

    #[test]
    fn stack_and_paging_requests_have_required_values() {
        let limine = Source::read("src/limine.rs");
        let stack = initializer(&limine.tokens, "const", "LIMINE_STACK_SIZE")
            .iter()
            .find_map(|token| parse_integer(token))
            .expect("LIMINE_STACK_SIZE must be numeric");
        assert_eq!(stack, 8192, "LIMINE_STACK_SIZE must be 8 KiB");
        let stack_request = initializer(&limine.tokens, "static", "STACK_SIZE_REQUEST").concat();
        assert!(
            stack_request.contains("stack_size:LIMINE_STACK_SIZE"),
            "STACK_SIZE_REQUEST must use LIMINE_STACK_SIZE"
        );

        let paging = initializer(&limine.tokens, "static", "PAGING_MODE_REQUEST").concat();
        for required in [
            "revision:1",
            "mode:paging_modes::PREFERRED",
            "max_mode:paging_modes::MAX",
            "min_mode:paging_modes::MIN",
        ] {
            assert!(
                paging.contains(required),
                "PAGING_MODE_REQUEST must contain `{required}`"
            );
        }
        let sv48 = initializer(&limine.tokens, "const", "SV48")
            .iter()
            .find_map(|token| parse_integer(token))
            .expect("paging_modes::SV48 must be numeric");
        assert_eq!(sv48, 1, "Limine's RISC-V SV48 mode value is 1");
        for name in ["PREFERRED", "MAX", "MIN"] {
            assert_eq!(
                initializer(&limine.tokens, "const", name).concat(),
                "SV48",
                "paging_modes::{name} must require exactly SV48"
            );
        }
    }

    #[test]
    fn address_conversion_contract_covers_all_address_regions() {
        let limine = Source::read("src/limine.rs");
        assert!(
            limine.compact.contains("pubfnva_to_pa(addr:usize)->usize"),
            "va_to_pa must be public and accept and return usize"
        );
        assert!(
            limine.compact.contains("pubfnpa_to_va(addr:usize)->usize"),
            "pa_to_va must be public and accept and return usize"
        );

        let to_phys = function_body(&limine, "va_to_pa");
        assert_body_number(
            to_phys,
            0xffff_ffff_0000_0000,
            "va_to_pa kernel classification",
        );
        assert_body_number(
            to_phys,
            0xffff_8000_0000_0000,
            "va_to_pa HHDM classification",
        );
        assert_order(
            to_phys,
            &[
                "kernel_virt_base",
                "kernel_phys_base",
                "KERNEL_IO_ADDR",
                "KERNEL_HEAP_ADDR",
                "hhdm_virt_to_phys",
            ],
            "va_to_pa address-region order",
        );

        let to_virt = function_body(&limine, "pa_to_va");
        assert_body_number(
            to_virt,
            0xffff_8000_0000_0000,
            "pa_to_va virtual-address guard",
        );
        assert_body_number(to_virt, 0x8000_0000, "pa_to_va MMIO boundary");
        assert_order(
            to_virt,
            &["KERNEL_IO_ADDR", "hhdm_phys_to_virt"],
            "pa_to_va physical-address classification",
        );

        for helper in ["hhdm_phys_to_virt", "hhdm_virt_to_phys"] {
            let body = function_body(&limine, helper).concat();
            assert!(
                body.contains("saturating_add") || body.contains("saturating_sub"),
                "{helper} must use saturating arithmetic"
            );
        }
    }

    #[test]
    fn limine_response_readers_handle_required_data() {
        let limine = Source::read("src/limine.rs");
        for function in [
            "kernel_phys_base",
            "kernel_virt_base",
            "bsp_hartid",
            "cmd_line",
            "usable_memory_regions",
            "all_memory_regions",
            "date_at_boot",
            "rsdp_virt",
        ] {
            assert!(
                limine.compact.contains(&format!("pubfn{function}")),
                "missing public Limine response reader `{function}`"
            );
        }

        let cmdline = function_body(&limine, "cmd_line").concat();
        for required in ["is_null", "CStr", "to_str", "None"] {
            assert!(cmdline.contains(required), "cmd_line must use `{required}`");
        }

        let date = function_body(&limine, "date_at_boot").concat();
        assert!(date.contains("is_null") && date.contains("None") && date.contains("timestamp"));
        let rsdp = function_body(&limine, "rsdp_virt").concat();
        assert!(
            rsdp.contains("is_null") && rsdp.contains("None") && rsdp.contains("address"),
            "rsdp_virt must reject a missing response"
        );
        assert!(
            rsdp.contains("addr==0") || rsdp.contains("address==0"),
            "rsdp_virt must reject a zero RSDP address"
        );

        assert_fields(
            &limine,
            "MemoryMapEntry",
            &[("base", "u64"), ("length", "u64"), ("entry_type", "u64")],
        );
        let memmap_values: BTreeMap<&str, u64> = [
            ("USABLE", 0),
            ("RESERVED", 1),
            ("ACPI_RECLAIMABLE", 2),
            ("ACPI_NVS", 3),
            ("BAD_MEMORY", 4),
            ("BOOTLOADER_RECLAIMABLE", 5),
            ("KERNEL_AND_MODULES", 6),
            ("FRAMEBUFFER", 7),
            ("RESERVED_MAPPED", 8),
        ]
        .into_iter()
        .collect();
        for (name, expected) in memmap_values {
            let value = initializer(&limine.tokens, "const", name)
                .iter()
                .find_map(|token| parse_integer(token))
                .unwrap_or_else(|| panic!("memmap::{name} must be numeric"));
            assert_eq!(value, expected, "memmap::{name} has the wrong value");
        }
        let usable = function_body(&limine, "usable_memory_regions").concat();
        for required in [
            "entry_count",
            "entries",
            "entry_type",
            "memmap::USABLE",
            "base",
            "length",
        ] {
            assert!(
                usable.contains(required),
                "usable_memory_regions must use `{required}`"
            );
        }
        let all = function_body(&limine, "all_memory_regions").concat();
        assert!(all.contains("entry_count") && all.contains("entries"));
    }

    #[test]
    fn acpi_rsdp_and_description_header_layouts_are_correct() {
        let acpi = Source::read("src/acpi.rs");
        assert_repr(&acpi, "DescriptionHeader", false);
        assert_fields(
            &acpi,
            "DescriptionHeader",
            &[
                ("signature", "[u8;4]"),
                ("length", "u32"),
                ("revision", "u8"),
                ("checksum", "u8"),
                ("oem_id", "[u8;6]"),
                ("oem_table_id", "u64"),
                ("oem_revision", "u32"),
                ("creator_id", "u32"),
                ("creator_revision", "u32"),
            ],
        );

        assert_repr(&acpi, "RsdtRawV1", true);
        let v1 = struct_fields(&acpi, "RsdtRawV1");
        assert_eq!(
            &v1[..4],
            &[
                ("signature".to_string(), "[u8;8]".to_string()),
                ("checksum".to_string(), "u8".to_string()),
                ("oem_id".to_string(), "[u8;6]".to_string()),
                ("revision".to_string(), "u8".to_string()),
            ],
            "RsdtRawV1 must start with the ACPI 1.0 RSDP fields"
        );
        assert!(
            v1.len() == 5 && v1[4].0.contains("rsdt") && v1[4].1 == "u32",
            "RsdtRawV1 must end with the 32-bit physical RSDT address"
        );

        assert_repr(&acpi, "RsdtRawV2", true);
        let v2 = struct_fields(&acpi, "RsdtRawV2");
        let v2_compact: String = v2
            .iter()
            .map(|(field, ty)| format!("{field}:{ty},"))
            .collect();
        let extension = "length:u32,xsdt_address:u64,extended_checksum:u8,reserved:[u8;3],";
        assert!(
            v2_compact.ends_with(extension),
            "RsdtRawV2 must end with the ACPI 2.0 RSDP extension fields"
        );
        let has_nested_v1 = v2_compact.starts_with("v1:RsdtRawV1,");
        let flat_prefix = &v2[..v2.len().saturating_sub(4)];
        let has_flat_v1 = flat_prefix.len() == 5
            && flat_prefix[0] == ("signature".to_string(), "[u8;8]".to_string())
            && flat_prefix[1] == ("checksum".to_string(), "u8".to_string())
            && flat_prefix[2] == ("oem_id".to_string(), "[u8;6]".to_string())
            && flat_prefix[3] == ("revision".to_string(), "u8".to_string())
            && flat_prefix[4].0.contains("rsdt")
            && flat_prefix[4].1 == "u32";
        assert!(
            has_nested_v1 || has_flat_v1,
            "RsdtRawV2 must contain the RSDP v1 prefix"
        );
        assert!(
            acpi.raw.contains("RSD PTR "),
            "RSDP validation must use the eight-byte `RSD PTR ` signature"
        );
    }

    #[test]
    fn xsdt_uses_64_bit_physical_entries_and_routes_required_tables() {
        let acpi = Source::read("src/acpi.rs");
        assert!(
            acpi.raw.contains("XSDT"),
            "ACPI code must identify the XSDT signature"
        );
        let count = function_body_with(&acpi, "num_entries", "saturating_sub");
        assert_body_number(count, 36, "XSDT entry count");
        assert_body_number(count, 8, "XSDT entry width");

        let entry = function_body_with(&acpi, "get_entry", "SystemTableType");
        let entry_text = entry.concat();
        assert!(
            entry_text.contains("read_byte_offset::<u64>")
                || entry_text.contains("read_unaligned::<u64>"),
            "each XSDT entry must be read as one u64 physical address"
        );
        assert_body_number(entry, 36, "first XSDT entry offset");
        assert_body_number(entry, 8, "XSDT entry stride");
        assert!(
            entry_text.contains("None"),
            "XSDT get_entry must reject an out-of-range index"
        );
        assert!(
            entry_text.contains("new_from_phys") || entry_text.contains("pa_to_va"),
            "XSDT physical table addresses must be converted before dereference"
        );
        for (signature, table) in [
            ("RHCT", "Rhct"),
            ("APIC", "Madt"),
            ("SPCR", "Spcr"),
            ("MCFG", "Mcfg"),
        ] {
            assert!(
                entry_text.contains(signature),
                "XSDT must recognize `{signature}`"
            );
            assert!(
                entry_text.contains(table),
                "XSDT must return the {table} table type"
            );
        }
        assert!(
            entry_text.contains("Unknown"),
            "unknown XSDT signatures must remain enumerable"
        );
    }

    #[test]
    fn madt_layout_and_variable_length_traversal_match_the_lab() {
        let acpi = Source::read("src/acpi.rs");
        assert_repr(&acpi, "MadtRaw", false);
        assert_fields(
            &acpi,
            "MadtRaw",
            &[
                ("header", "DescriptionHeader"),
                ("local_controller_addr", "u32"),
                ("flags", "u32"),
            ],
        );
        let entry = function_body_with(&acpi, "get_entry", "MadtStructure");
        let body = entry.concat();
        assert_body_number(entry, 44, "first MADT structure offset");
        assert!(
            body.contains("structure.length") || body.contains("header.length"),
            "MADT traversal must advance by each structure's length"
        );
        assert!(
            body.contains(">=") && body.contains("None"),
            "MADT traversal must check table bounds"
        );
        for required in [
            "MADT_TYPE_RINTC",
            "MadtStructure::Rintc",
            "MADT_TYPE_IMSIC",
            "MadtStructure::Imsic",
            "MADT_TYPE_APLIC",
            "MadtStructure::Aplic",
        ] {
            assert!(
                body.contains(required),
                "MADT get_entry must use `{required}`"
            );
        }
        assert!(
            acpi.raw.to_ascii_lowercase().contains("imsic")
                && acpi.raw.to_ascii_lowercase().contains("rintc"),
            "the implementation must retain RINTC data for per-hart IMSIC addresses"
        );
    }

    #[test]
    fn mcfg_layout_and_entry_stride_match_the_lab() {
        let acpi = Source::read("src/acpi.rs");
        assert_repr(&acpi, "McfgRaw", false);
        assert_fields(
            &acpi,
            "McfgRaw",
            &[("header", "DescriptionHeader"), ("reserved", "u64")],
        );
        assert_repr(&acpi, "McfgEntry", false);
        assert_fields(
            &acpi,
            "McfgEntry",
            &[
                ("base", "u64"),
                ("pci_seg_group", "u16"),
                ("start_bus", "u8"),
                ("end_bus", "u8"),
                ("reserved", "u32"),
            ],
        );
        let entry = function_body_with(&acpi, "get_entry", "McfgEntry");
        let body = entry.concat();
        assert_body_number(entry, 44, "first MCFG allocation offset");
        assert_body_number(entry, 16, "MCFG allocation stride");
        assert!(
            body.contains(">=") && body.contains("None"),
            "MCFG get_entry must check bounds"
        );
    }

    #[test]
    fn spcr_and_rhct_packed_layouts_match_the_lab() {
        let acpi = Source::read("src/acpi.rs");
        assert_repr(&acpi, "SpcrRaw", true);
        assert_fields(
            &acpi,
            "SpcrRaw",
            &[
                ("header", "DescriptionHeader"),
                ("interface_type", "u8"),
                ("reserved1", "[u8;3]"),
                ("base_address", "[u8;12]"),
                ("interrupt_type", "u8"),
                ("irq", "u8"),
                ("global_system_interrupt", "u32"),
                ("configured_baud_rate", "u8"),
                ("parity", "u8"),
                ("stop_bits", "u8"),
                ("flow_control", "u8"),
                ("terminal_type", "u8"),
                ("language", "u8"),
                ("pci_dev_id", "u16"),
                ("pci_ven_id", "u16"),
                ("pci_bus_num", "u8"),
                ("pci_dev_num", "u8"),
                ("pci_func_num", "u8"),
                ("pci_flags", "u32"),
                ("pci_seg", "u8"),
                ("uart_freq", "u32"),
                ("precise_baud", "u32"),
                ("namespace_str_len", "u16"),
                ("namespace_str_off", "u16"),
            ],
        );

        assert_repr(&acpi, "RhctRaw", true);
        assert_fields(
            &acpi,
            "RhctRaw",
            &[
                ("header", "DescriptionHeader"),
                ("flags", "u32"),
                ("time_base_freq", "u64"),
                ("num_nodes", "u32"),
                ("offset_to_nodes", "u32"),
            ],
        );
    }

    #[test]
    fn main_initializes_lab_modules_and_prints_required_results() {
        let main = Source::read("src/main.rs");
        assert!(
            main.compact.contains("limine::"),
            "main.rs must call Limine readers"
        );
        assert!(
            main.compact.contains("acpi::"),
            "main.rs must call ACPI readers"
        );
        let output = debug_output_text(&main);
        for term in ["aplic", "imsic", "ecam", "uart"] {
            assert!(
                output.contains(term),
                "main.rs debugln! output must identify {term} data"
            );
        }
        assert!(
            [
                "timer",
                "time_base",
                "timebase",
                "frequency",
                "freq",
                "rhct"
            ]
            .iter()
            .any(|term| output.contains(term)),
            "main.rs debugln! output must identify the system timer frequency"
        );
    }

    #[test]
    fn lab_sources_do_not_suppress_all_warnings() {
        for file in [
            "src/main.rs",
            "src/config.rs",
            "src/limine.rs",
            "src/acpi.rs",
        ] {
            let source = Source::read(file);
            let compact = source.compact.to_ascii_lowercase();
            assert!(
                !compact.contains("allow(warnings)") && !compact.contains("allow(unused)"),
                "{file} must not suppress all warnings or all unused-code warnings"
            );
        }
    }

    #[test]
    fn kernel_passes_cargo_check_with_warnings_denied() {
        let target_dir =
            std::env::temp_dir().join(format!("myos-lab2-cargo-check-{}", std::process::id()));
        let output = Command::new("cargo")
            .args(["check", "--quiet"])
            .current_dir(repo_root())
            .env("RUSTFLAGS", "-Dwarnings")
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .expect("failed to start cargo check");
        let _ = fs::remove_dir_all(&target_dir);
        assert!(
            output.status.success(),
            "kernel must pass `cargo check` with warnings denied\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn bare_metal_rv64_guest_runs_in_qemu() {
        let guest = repo_root().join("lab2-tests/riscv-guest");
        let target_dir =
            std::env::temp_dir().join(format!("myos-lab2-riscv-guest-{}", std::process::id()));
        let output = Command::new("cargo")
            .arg("test")
            .current_dir(&guest)
            .env("CARGO_TARGET_DIR", &target_dir)
            .env(
                "CARGO_TARGET_RISCV64GC_UNKNOWN_NONE_ELF_RUNNER",
                "sh run-qemu.sh",
            )
            .env("CARGO_ENCODED_RUSTFLAGS", "-Clink-arg=-Ttests/riscv.ld")
            .output()
            .expect("failed to start the RISC-V guest test");
        let _ = fs::remove_dir_all(&target_dir);
        assert!(
            output.status.success(),
            "the bare-metal RV64 guest must pass in QEMU\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
