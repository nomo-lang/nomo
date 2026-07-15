use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Architecture {
    X86_64,
    Aarch64,
}

impl Architecture {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Vendor {
    Unknown,
    Apple,
    Pc,
}

impl Vendor {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Apple => "apple",
            Self::Pc => "pc",
        }
    }
}

impl fmt::Display for Vendor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperatingSystem {
    Linux,
    Darwin,
    Windows,
}

impl OperatingSystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Darwin => "darwin",
            Self::Windows => "windows",
        }
    }

    pub const fn platform_name(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Darwin => "macos",
            Self::Windows => "windows",
        }
    }
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Environment {
    Gnu,
    Msvc,
    None,
}

impl Environment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gnu => "gnu",
            Self::Msvc => "msvc",
            Self::None => "none",
        }
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiFacts {
    pub pointer_width: u8,
    pub c_int_width: u8,
    pub c_long_width: u8,
    pub endianness: Endianness,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetTriple {
    arch: Architecture,
    vendor: Vendor,
    os: OperatingSystem,
    env: Environment,
}

impl TargetTriple {
    pub const fn new(
        arch: Architecture,
        vendor: Vendor,
        os: OperatingSystem,
        env: Environment,
    ) -> Self {
        Self {
            arch,
            vendor,
            os,
            env,
        }
    }

    pub fn host() -> Result<Self, String> {
        let arch = if cfg!(target_arch = "x86_64") {
            Architecture::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Architecture::Aarch64
        } else {
            return Err(format!(
                "unsupported host architecture `{}`; supported architectures are x86_64 and aarch64",
                std::env::consts::ARCH
            ));
        };

        if cfg!(target_os = "macos") {
            Ok(Self::new(
                arch,
                Vendor::Apple,
                OperatingSystem::Darwin,
                Environment::None,
            ))
        } else if cfg!(target_os = "linux") {
            Ok(Self::new(
                arch,
                Vendor::Unknown,
                OperatingSystem::Linux,
                Environment::Gnu,
            ))
        } else if cfg!(target_os = "windows") {
            Ok(Self::new(
                arch,
                Vendor::Pc,
                OperatingSystem::Windows,
                Environment::Msvc,
            ))
        } else {
            Err(format!(
                "unsupported host operating system `{}`; supported systems are linux, macos, and windows",
                std::env::consts::OS
            ))
        }
    }

    pub const fn architecture(&self) -> Architecture {
        self.arch
    }

    pub const fn vendor(&self) -> Vendor {
        self.vendor
    }

    pub const fn operating_system(&self) -> OperatingSystem {
        self.os
    }

    pub const fn environment(&self) -> Environment {
        self.env
    }

    pub const fn abi(&self) -> AbiFacts {
        let c_long_width = if matches!(self.os, OperatingSystem::Windows) {
            32
        } else {
            64
        };
        AbiFacts {
            pointer_width: 64,
            c_int_width: 32,
            c_long_width,
            endianness: Endianness::Little,
        }
    }

    pub fn llvm_triple(&self) -> String {
        if self.env == Environment::None {
            format!("{}-{}-{}", self.arch, self.vendor, self.os)
        } else {
            self.to_string()
        }
    }

    pub fn c_toolchain_from(&self, host: &Self) -> Result<CToolchain, String> {
        if self == host {
            return Ok(CToolchain {
                program: "cc".to_string(),
                args: Vec::new(),
            });
        }
        if host.os == OperatingSystem::Darwin
            && self.os == OperatingSystem::Darwin
            && host.vendor == Vendor::Apple
            && self.vendor == Vendor::Apple
            && host.arch != self.arch
        {
            return Ok(CToolchain {
                program: "cc".to_string(),
                args: vec!["-target".to_string(), self.llvm_triple()],
            });
        }
        Err(format!(
            "cross compilation from `{host}` to `{self}` is not configured; the first supported cross path is x86_64-apple-darwin-none <-> aarch64-apple-darwin-none"
        ))
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}-{}-{}-{}",
            self.arch, self.vendor, self.os, self.env
        )
    }
}

impl FromStr for TargetTriple {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.trim() != input || input.is_empty() || input.chars().any(char::is_uppercase) {
            return Err(format!(
                "invalid target triple `{input}`; target triples must be lowercase with no surrounding whitespace"
            ));
        }
        let mut parts = input.split('-').collect::<Vec<_>>();
        if parts.len() == 3 && matches!(parts[2], "darwin" | "macos") {
            parts.push("none");
        }
        let [arch, vendor, os, env] = parts.as_slice() else {
            return Err(format!(
                "invalid target triple `{input}`; expected arch-vendor-os-env (the standard three-part Apple Darwin spelling is also accepted)"
            ));
        };
        let arch = match *arch {
            "x86_64" | "amd64" => Architecture::X86_64,
            "aarch64" | "arm64" => Architecture::Aarch64,
            other => return Err(format!("unsupported target architecture `{other}`")),
        };
        let vendor = match *vendor {
            "unknown" => Vendor::Unknown,
            "apple" => Vendor::Apple,
            "pc" => Vendor::Pc,
            other => return Err(format!("unsupported target vendor `{other}`")),
        };
        let os = match *os {
            "linux" => OperatingSystem::Linux,
            "darwin" | "macos" => OperatingSystem::Darwin,
            "windows" => OperatingSystem::Windows,
            other => return Err(format!("unsupported target operating system `{other}`")),
        };
        let env = match *env {
            "gnu" => Environment::Gnu,
            "msvc" => Environment::Msvc,
            "none" => Environment::None,
            other => return Err(format!("unsupported target environment `{other}`")),
        };
        let target = Self::new(arch, vendor, os, env);
        match (target.vendor, target.os, target.env) {
            (Vendor::Unknown, OperatingSystem::Linux, Environment::Gnu)
            | (Vendor::Apple, OperatingSystem::Darwin, Environment::None)
            | (Vendor::Pc, OperatingSystem::Windows, Environment::Msvc) => Ok(target),
            _ => Err(format!(
                "unsupported target combination `{target}`; supported families are unknown-linux-gnu, apple-darwin-none, and pc-windows-msvc"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CToolchain {
    pub program: String,
    pub args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_supported_aliases() {
        let darwin = "arm64-apple-macos".parse::<TargetTriple>().unwrap();
        assert_eq!(darwin.to_string(), "aarch64-apple-darwin-none");
        assert_eq!(darwin.llvm_triple(), "aarch64-apple-darwin");

        let linux = "amd64-unknown-linux-gnu".parse::<TargetTriple>().unwrap();
        assert_eq!(linux.to_string(), "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn rejects_unknown_or_incoherent_targets() {
        assert!("riscv64-unknown-linux-gnu".parse::<TargetTriple>().is_err());
        assert!("x86_64-apple-linux-gnu".parse::<TargetTriple>().is_err());
        assert!("X86_64-unknown-linux-gnu".parse::<TargetTriple>().is_err());
    }

    #[test]
    fn exposes_stable_abi_facts() {
        let linux = "x86_64-unknown-linux-gnu".parse::<TargetTriple>().unwrap();
        assert_eq!(linux.abi().pointer_width, 64);
        assert_eq!(linux.abi().c_long_width, 64);

        let windows = "x86_64-pc-windows-msvc".parse::<TargetTriple>().unwrap();
        assert_eq!(windows.abi().c_long_width, 32);
    }

    #[test]
    fn host_target_is_supported_and_canonical() {
        let host = TargetTriple::host().unwrap();
        assert_eq!(host.to_string().split('-').count(), 4);
        assert_eq!(
            host.c_toolchain_from(&host).unwrap().args,
            Vec::<String>::new()
        );
    }

    #[test]
    fn apple_cross_arch_uses_explicit_clang_target() {
        let host = "aarch64-apple-darwin".parse::<TargetTriple>().unwrap();
        let target = "x86_64-apple-darwin".parse::<TargetTriple>().unwrap();
        assert_eq!(
            target.c_toolchain_from(&host).unwrap().args,
            ["-target", "x86_64-apple-darwin"]
        );
    }
}
